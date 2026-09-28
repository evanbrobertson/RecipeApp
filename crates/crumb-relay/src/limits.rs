//! How much the relay will fetch. A relay's address is someone's home connection, and a home
//! address that hammers a site gets blocked like the datacenter's did, so it is deliberately
//! stingy: one request per site every few seconds, a couple at once, a few a minute. It keeps
//! no records beyond the last few minutes, in memory.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Past this many sites remembered, the ones whose wait is over are forgotten.
const HOSTS_KEPT: usize = 1024;

/// Why a fetch wasn't started.
#[derive(Debug, PartialEq, Eq)]
pub enum Denied {
    /// The most fetches at once are running.
    Busy,
    /// The per-minute cap is used up; try again after this long.
    Minute(Duration),
    /// This site was fetched too recently; try again after this long.
    Host(Duration),
}

impl Denied {
    /// How long to tell the caller to wait, at least a second.
    pub fn retry_after(&self) -> Duration {
        match self {
            Denied::Busy => Duration::from_secs(1),
            Denied::Minute(d) | Denied::Host(d) => (*d).max(Duration::from_secs(1)),
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Denied::Busy => "The relay is busy.",
            Denied::Minute(_) => "The relay is over its per-minute limit.",
            Denied::Host(_) => "That site was fetched a moment ago.",
        }
    }
}

#[derive(Default)]
struct Recent {
    /// When each site was last fetched.
    hosts: HashMap<String, Instant>,
    /// When the fetches of the last minute started.
    minute: VecDeque<Instant>,
}

pub struct Limits {
    host_interval: Duration,
    per_minute: usize,
    running: Arc<Semaphore>,
    recent: Mutex<Recent>,
}

impl Limits {
    pub fn new(host_interval: Duration, concurrency: usize, per_minute: usize) -> Self {
        Self {
            host_interval,
            per_minute,
            running: Arc::new(Semaphore::new(concurrency)),
            recent: Mutex::default(),
        }
    }

    /// Lets a fetch of `host` start, or says why not. The permit is held while it runs. A
    /// fetch that is turned away is not counted against any limit.
    pub fn admit(&self, host: &str, now: Instant) -> Result<OwnedSemaphorePermit, Denied> {
        let permit = self
            .running
            .clone()
            .try_acquire_owned()
            .map_err(|_| Denied::Busy)?;
        let mut recent = self.recent.lock().unwrap();

        while recent
            .minute
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60))
        {
            recent.minute.pop_front();
        }
        if recent.minute.len() >= self.per_minute
            && let Some(oldest) = recent.minute.front()
        {
            return Err(Denied::Minute(
                (*oldest + Duration::from_secs(60)).saturating_duration_since(now),
            ));
        }
        if let Some(last) = recent.hosts.get(host) {
            let ready = *last + self.host_interval;
            if ready > now {
                return Err(Denied::Host(ready - now));
            }
        }

        if recent.hosts.len() >= HOSTS_KEPT {
            let interval = self.host_interval;
            recent.hosts.retain(|_, last| *last + interval > now);
        }
        recent.hosts.insert(host.to_string(), now);
        recent.minute.push_back(now);
        Ok(permit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEC: Duration = Duration::from_secs(1);

    #[test]
    fn one_fetch_per_site_per_interval() {
        let limits = Limits::new(5 * SEC, 10, 100);
        let t0 = Instant::now();
        drop(limits.admit("a.test", t0).unwrap());
        assert_eq!(
            limits.admit("a.test", t0 + 2 * SEC).unwrap_err(),
            Denied::Host(3 * SEC)
        );
        // Another site is unaffected, and the first is fine again after the interval
        drop(limits.admit("b.test", t0 + 2 * SEC).unwrap());
        drop(limits.admit("a.test", t0 + 5 * SEC).unwrap());
    }

    #[test]
    fn a_turned_away_fetch_does_not_push_the_wait_out() {
        let limits = Limits::new(5 * SEC, 10, 100);
        let t0 = Instant::now();
        drop(limits.admit("a.test", t0).unwrap());
        assert!(limits.admit("a.test", t0 + 4 * SEC).is_err());
        drop(limits.admit("a.test", t0 + 5 * SEC).unwrap());
    }

    #[test]
    fn a_few_at_once() {
        let limits = Limits::new(Duration::ZERO, 2, 100);
        let t0 = Instant::now();
        let a = limits.admit("a.test", t0).unwrap();
        let _b = limits.admit("b.test", t0).unwrap();
        assert_eq!(limits.admit("c.test", t0).unwrap_err(), Denied::Busy);
        drop(a);
        assert!(limits.admit("c.test", t0).is_ok());
    }

    #[test]
    fn a_few_a_minute() {
        let limits = Limits::new(Duration::ZERO, 10, 3);
        let t0 = Instant::now();
        for (i, host) in ["a", "b", "c"].iter().enumerate() {
            drop(limits.admit(host, t0 + i as u32 * SEC).unwrap());
        }
        // The first of the three started 10 seconds ago: 50 to wait
        assert_eq!(
            limits.admit("d", t0 + 10 * SEC).unwrap_err(),
            Denied::Minute(50 * SEC)
        );
        // A minute after the first, one place is free again
        drop(limits.admit("d", t0 + 60 * SEC).unwrap());
        assert!(matches!(
            limits.admit("e", t0 + 60 * SEC).unwrap_err(),
            Denied::Minute(_)
        ));
    }

    #[test]
    fn retry_after_is_at_least_a_second() {
        assert_eq!(Denied::Host(Duration::from_millis(10)).retry_after(), SEC);
        assert_eq!(Denied::Minute(30 * SEC).retry_after(), 30 * SEC);
        assert_eq!(Denied::Busy.retry_after(), SEC);
    }

    #[test]
    fn forgets_sites_whose_wait_is_over() {
        let limits = Limits::new(SEC, 10_000, 100_000);
        let t0 = Instant::now();
        for i in 0..HOSTS_KEPT {
            drop(limits.admit(&format!("{i}.test"), t0).unwrap());
        }
        drop(limits.admit("new.test", t0 + 10 * SEC).unwrap());
        assert!(limits.recent.lock().unwrap().hosts.len() < HOSTS_KEPT);
    }
}
