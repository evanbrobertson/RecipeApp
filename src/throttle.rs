//! Attempt limits for sign-in and other guessable or abusable endpoints.
//!
//! [`Throttle`] backs off: after a few free tries against a key (a client address, an account),
//! each further try has to wait twice as long as the one before, up to 15 minutes. Tries are
//! counted when they start, not when they fail, so a burst of parallel guesses can't all slip
//! past a counter that only moves once each has been checked; a success clears the keys.
//! [`Rate`] is a plain per-window count for things like client registrations.
//! Both are bounded in memory: a flood of new keys never grows them without limit.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::AppError;

/// The connection's client address (see [`crate::share::client_ip_from`]), put on each request
/// by [`client_ip_layer`] so handlers can key limits on it.
#[derive(Clone, Debug)]
pub struct ClientIp(pub String);

/// Sets [`ClientIp`] on the request.
pub async fn client_ip_layer(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let ip = crate::share::client_ip(&req, state.config.trust_proxy_headers);
    req.extensions_mut().insert(ClientIp(ip));
    next.run(req).await
}

const MAX_TRACKED: usize = 8192;
const FORGET_AFTER: Duration = Duration::from_secs(60 * 60);
const LONGEST_WAIT: Duration = Duration::from_secs(15 * 60);

struct Entry {
    attempts: u32,
    last: Instant,
}

pub struct Throttle {
    seen: Mutex<HashMap<String, Entry>>,
    base: Duration,
}

impl Default for Throttle {
    fn default() -> Self {
        Self::with_base(Duration::from_secs(2))
    }
}

impl Throttle {
    pub fn with_base(base: Duration) -> Self {
        Self {
            seen: Mutex::default(),
            base,
        }
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.seen.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// How long a key with `attempts` tries (`free` of them free) makes the next one wait.
    fn wait(&self, attempts: u32, free: u32) -> Duration {
        if attempts < free {
            return Duration::ZERO;
        }
        let doublings = (attempts - free).min(20);
        self.base
            .checked_mul(1u32 << doublings)
            .unwrap_or(LONGEST_WAIT)
            .min(LONGEST_WAIT)
    }

    /// Starts an attempt against every `(key, free tries)` pair, or says how long to wait.
    /// A refused attempt is not counted, so waiting it out always works.
    pub fn attempt(&self, keys: &[(&str, u32)]) -> Result<(), AppError> {
        let mut seen = self.locked();
        let mut longest = Duration::ZERO;
        for (key, free) in keys {
            if let Some(e) = seen.get(*key)
                && e.last.elapsed() < FORGET_AFTER
            {
                let wait = self
                    .wait(e.attempts, *free)
                    .saturating_sub(e.last.elapsed());
                longest = longest.max(wait);
            }
        }
        if !longest.is_zero() {
            let secs = longest.as_secs().max(1);
            let unit = if secs == 1 { "second" } else { "seconds" };
            return Err(AppError::new(
                429,
                format!("Too many attempts. Try again in {secs} {unit}."),
            ));
        }
        if seen.len() >= MAX_TRACKED {
            seen.retain(|_, e| e.last.elapsed() < FORGET_AFTER);
            while seen.len() >= MAX_TRACKED {
                let Some(oldest) = seen
                    .iter()
                    .min_by_key(|(_, e)| e.last)
                    .map(|(k, _)| k.clone())
                else {
                    break;
                };
                seen.remove(&oldest);
            }
        }
        for (key, _) in keys {
            let e = seen.entry((*key).to_string()).or_insert(Entry {
                attempts: 0,
                last: Instant::now(),
            });
            if e.last.elapsed() >= FORGET_AFTER {
                e.attempts = 0;
            }
            e.attempts = e.attempts.saturating_add(1);
            e.last = Instant::now();
        }
        Ok(())
    }

    /// A successful sign-in forgets its keys.
    pub fn clear(&self, keys: &[&str]) {
        let mut seen = self.locked();
        for key in keys {
            seen.remove(*key);
        }
    }
}

/// A count per key in fixed windows.
#[derive(Default)]
pub struct Rate {
    seen: Mutex<HashMap<String, (Instant, u32)>>,
}

impl Rate {
    /// Counts one hit on `key`; false once it is past `limit` within `window`.
    pub fn hit(&self, key: &str, limit: u32, window: Duration) -> bool {
        let mut seen = self.seen.lock().unwrap_or_else(|e| e.into_inner());
        if seen.len() >= MAX_TRACKED && !seen.contains_key(key) {
            seen.retain(|_, (at, _)| at.elapsed() < window);
            if seen.len() >= MAX_TRACKED
                && let Some(oldest) = seen
                    .iter()
                    .min_by_key(|(_, (at, _))| *at)
                    .map(|(k, _)| k.clone())
            {
                seen.remove(&oldest);
            }
        }
        let entry = seen.entry(key.to_string()).or_insert((Instant::now(), 0));
        if entry.0.elapsed() >= window {
            *entry = (Instant::now(), 0);
        }
        if entry.1 >= limit {
            return false;
        }
        entry.1 += 1;
        true
    }
}

/// Tries allowed from one client address before backing off (a household may share one).
pub const IP_FREE: u32 = 10;
/// Tries allowed against one account before backing off.
pub const ACCOUNT_FREE: u32 = 5;

impl crate::AppState {
    /// Starts a sign-in attempt from `ip` against `account` (an email, or "password" for the
    /// single app password). Refuses with 429 while either is backing off.
    pub fn login_attempt(&self, ip: &ClientIp, account: &str) -> Result<LoginKeys, AppError> {
        let keys = LoginKeys {
            ip: format!("ip:{}", ip.0),
            account: format!("acct:{}", account.trim().to_lowercase()),
        };
        self.logins
            .attempt(&[(&keys.ip, IP_FREE), (&keys.account, ACCOUNT_FREE)])?;
        Ok(keys)
    }

    /// The attempt worked: forget it.
    pub fn login_succeeded(&self, keys: &LoginKeys) {
        self.logins.clear(&[&keys.ip, &keys.account]);
    }
}

pub struct LoginKeys {
    ip: String,
    account: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backs_off_after_the_free_tries_and_doubles() {
        let t = Throttle::with_base(Duration::from_millis(40));
        for _ in 0..3 {
            assert!(t.attempt(&[("a", 3)]).is_ok());
        }
        // Counted at the start, so an immediate fourth is refused
        let err = t.attempt(&[("a", 3)]).unwrap_err();
        assert_eq!(err.status.as_u16(), 429);
        assert!(t.attempt(&[("b", 3)]).is_ok(), "other keys are unaffected");
        std::thread::sleep(Duration::from_millis(50));
        assert!(t.attempt(&[("a", 3)]).is_ok());
        // The next wait is twice as long
        std::thread::sleep(Duration::from_millis(50));
        assert!(t.attempt(&[("a", 3)]).is_err());
        std::thread::sleep(Duration::from_millis(50));
        assert!(t.attempt(&[("a", 3)]).is_ok());
    }

    #[test]
    fn a_success_clears_the_keys() {
        let t = Throttle::with_base(Duration::from_secs(60));
        for _ in 0..2 {
            t.attempt(&[("a", 2)]).unwrap();
        }
        assert!(t.attempt(&[("a", 2)]).is_err());
        t.clear(&["a"]);
        assert!(t.attempt(&[("a", 2)]).is_ok());
    }

    #[test]
    fn either_key_can_hold_an_attempt_back() {
        let t = Throttle::with_base(Duration::from_secs(60));
        for _ in 0..2 {
            t.attempt(&[("ip:1", 10), ("acct:x", 2)]).unwrap();
        }
        // The account is backing off even from a fresh address
        assert!(t.attempt(&[("ip:2", 10), ("acct:x", 2)]).is_err());
        // and a refused try isn't counted against the fresh address
        assert!(t.attempt(&[("ip:2", 10), ("acct:y", 2)]).is_ok());
    }

    #[test]
    fn memory_stays_bounded() {
        let t = Throttle::default();
        for i in 0..MAX_TRACKED + 100 {
            t.attempt(&[(&format!("k{i}"), 5)]).unwrap();
        }
        assert!(t.locked().len() <= MAX_TRACKED);
    }

    #[test]
    fn rate_counts_per_window() {
        let r = Rate::default();
        let w = Duration::from_millis(60);
        assert!(r.hit("a", 2, w));
        assert!(r.hit("a", 2, w));
        assert!(!r.hit("a", 2, w));
        assert!(r.hit("b", 2, w));
        std::thread::sleep(Duration::from_millis(70));
        assert!(r.hit("a", 2, w));
    }
}
