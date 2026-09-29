//! Headless Chromium fallback for sites that block plain HTTP fetches, on the server and on
//! a `crumb-relay` that works for it.
//!
//! Chromium is installed in the Docker image; locally set CHROMIUM_PATH or it's skipped.
//! Only one page loads at a time to keep memory in check on a small instance, and it takes
//! a permit of the heavy-work budget video imports use (the server's `video_jobs`, a relay's
//! own), so the two never stack beyond it. The browser
//! is driven over the DevTools protocol directly (a handful of commands), which keeps the
//! binary small compared with a full CDP client.
//!
//! Chromium runs the JavaScript of whatever page it is sent to, and a page that answered the
//! server's own fetch with a bot check is not to be trusted. Its network goes through a
//! proxy that only connects to public addresses (`crumb_fetch::proxy`), loopback included, so
//! the page's script can't reach cloud metadata, the auth service or anything else on the
//! server's private network, whether it navigates, loads a sub-resource or calls `fetch`.

use futures_util::{SinkExt, StreamExt};
use regex::Regex;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, Semaphore};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

const CANDIDATES: [&str; 4] = [
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/usr/bin/google-chrome",
    "/opt/pw-browsers/chromium",
];

// Runs in the page
const RECIPE_READY: &str = r#"[...document.querySelectorAll('script[type="application/ld+json"]')]
  .some((s) => /Recipe/.test(s.textContent || ""))
  || !!document.querySelector('[itemprop="recipeIngredient"], [class*="ingredient"]')"#;

static CHALLENGE_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)just a moment|attention required|access denied|access to this page has been denied|verify you are human|are you a robot|not a robot|pardon our interruption|security check",
    )
    .unwrap()
});

/// Whether a page's title is a bot check's ("Just a moment…" and the like).
pub fn is_challenge_title(title: &str) -> bool {
    CHALLENGE_TITLE.is_match(title)
}

static VERSION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(\d+)\.\d+\.\d+\.\d+\b").unwrap());

#[cfg(target_os = "macos")]
const PLATFORM: &str = "Macintosh; Intel Mac OS X 10_15_7";
#[cfg(target_os = "windows")]
const PLATFORM: &str = "Windows NT 10.0; Win64; x64";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const PLATFORM: &str = "X11; Linux x86_64";

/// Chrome's reduced User-Agent for the major version in `chromium --version` output. Headless
/// mode's own says "HeadlessChrome", which bot checks refuse; this one keeps the real
/// version and platform, so it agrees with the client hints Chromium sends.
pub fn user_agent_for(version_output: &str) -> Option<String> {
    let major = VERSION.captures(version_output)?.get(1)?.as_str();
    Some(format!(
        "Mozilla/5.0 ({PLATFORM}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{major}.0.0.0 Safari/537.36"
    ))
}

/// The User-Agent for this Chromium, asked of the binary once. `None` leaves Chromium's own.
async fn user_agent(exe: &Path) -> Option<&'static str> {
    static UA: tokio::sync::OnceCell<Option<String>> = tokio::sync::OnceCell::const_new();
    UA.get_or_init(|| async {
        let out = Command::new(exe)
            .arg("--version")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output();
        let out = tokio::time::timeout(Duration::from_secs(10), out).await;
        let ua = out
            .ok()
            .and_then(Result::ok)
            .and_then(|o| user_agent_for(&String::from_utf8_lossy(&o.stdout)));
        if ua.is_none() {
            tracing::warn!("[browser] couldn't read Chromium's version; using its own User-Agent");
        }
        ua
    })
    .await
    .as_deref()
}

// Images, media and fonts are never needed for the DOM
const BLOCKED: [&str; 18] = [
    "*.png", "*.jpg", "*.jpeg", "*.gif", "*.webp", "*.avif", "*.svg", "*.ico", "*.bmp", "*.woff",
    "*.woff2", "*.ttf", "*.otf", "*.mp4", "*.webm", "*.mp3", "*.m3u8", "*.mov",
];

/// Longest a page load waits for the heavy-work budget before giving up.
const BUDGET_WAIT: Duration = Duration::from_secs(90);

pub struct Browser {
    executable: Option<PathBuf>,
    lock: Mutex<()>,
    /// The heavy-work budget shared with video imports (None: only `lock` limits it).
    budget: Option<Arc<Semaphore>>,
}

impl Browser {
    pub fn from_env() -> Self {
        let executable = if std::env::var("BROWSER_SCRAPING").is_ok_and(|v| v == "off") {
            None
        } else {
            std::env::var("CHROMIUM_PATH")
                .ok()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .or_else(|| CANDIDATES.iter().map(PathBuf::from).find(|p| p.exists()))
        };
        Self {
            executable,
            lock: Mutex::new(()),
            budget: None,
        }
    }

    pub fn disabled() -> Self {
        Self {
            executable: None,
            lock: Mutex::new(()),
            budget: None,
        }
    }

    /// Each page load takes a permit of `budget` too.
    pub fn share_budget(&mut self, budget: Arc<Semaphore>) {
        self.budget = Some(budget);
    }

    pub fn available(&self) -> bool {
        self.executable.is_some()
    }

    /// Loads a page in real Chromium and returns the rendered HTML (after scripts run).
    pub async fn fetch(&self, url: &str) -> Result<String, String> {
        let exe = self
            .executable
            .as_deref()
            .ok_or("Chromium is not installed")?;
        // The proxy refuses private addresses for everything Chromium does; this refuses the
        // link itself before a browser is started for it
        let target = url::Url::parse(url).map_err(|e| e.to_string())?;
        crumb_fetch::check_resolved(&target)
            .await
            .map_err(|why| why.to_string())?;
        let _permit = match &self.budget {
            Some(budget) => Some(
                tokio::time::timeout(BUDGET_WAIT, budget.acquire())
                    .await
                    .map_err(|_| "The server is busy watching a video. Try again in a minute.")?
                    .map_err(|e| e.to_string())?,
            ),
            None => None,
        };
        let _guard = self.lock.lock().await;
        let timeout = Duration::from_secs(40);
        tokio::time::timeout(timeout + Duration::from_secs(5), run(exe, url, timeout))
            .await
            .map_err(|_| "Timed out loading the page".to_string())?
    }
}

async fn run(exe: &Path, url: &str, timeout: Duration) -> Result<String, String> {
    let profile = tempfile::tempdir().map_err(|e| e.to_string())?;
    let proxy = crumb_fetch::proxy::Proxy::start()
        .await
        .map_err(|e| format!("Couldn't start the browser's proxy: {e}"))?;
    let mut command = Command::new(exe);
    if let Some(ua) = user_agent(exe).await {
        command.arg(format!("--user-agent={ua}"));
    }
    if no_sandbox() {
        command.arg("--no-sandbox");
    }
    let mut child = command
        .args([
            "--headless=new",
            "--disable-dev-shm-usage",
            "--disable-gpu",
            "--disable-blink-features=AutomationControlled",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-extensions",
            "--mute-audio",
            "--window-size=1366,900",
            "--lang=en-US",
            "--remote-debugging-port=0",
            &format!("--proxy-server=http://{}", proxy.addr()),
            // Loopback goes through the proxy too (and is refused there)
            "--proxy-bypass-list=<-loopback>",
            // Neither of these goes through an HTTP proxy
            "--disable-quic",
            "--force-webrtc-ip-handling-policy=disable_non_proxied_udp",
            &format!("--user-data-dir={}", profile.path().display()),
            "about:blank",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Couldn't start Chromium: {e}"))?;

    let result = drive(&mut child, url, timeout).await;
    let _ = child.kill().await;
    drop(proxy);
    result
}

/// Chromium's sandbox stays on unless `BROWSER_NO_SANDBOX=1`. It needs unprivileged user
/// namespaces (or a setuid helper), which some container platforms refuse; an operator who has
/// seen the launch error below opts out knowingly. (Root can't use the sandbox either, which is
/// one more reason the Docker image runs as an unprivileged user.)
fn no_sandbox() -> bool {
    static CHOICE: LazyLock<bool> = LazyLock::new(|| {
        let asked = std::env::var("BROWSER_NO_SANDBOX")
            .is_ok_and(|v| matches!(v.trim(), "1" | "true" | "on" | "yes"));
        if asked {
            tracing::warn!(
                "[browser] BROWSER_NO_SANDBOX is set: Chromium runs without its sandbox while it opens untrusted pages"
            );
        }
        asked
    });
    *CHOICE
}

/// What to tell the operator when Chromium never became ready.
fn launch_hint(stderr: &str) -> Option<&'static str> {
    let lower = stderr.to_lowercase();
    let sandbox_trouble =
        lower.contains("sandbox") || lower.contains("namespace") || lower.contains("zygote");
    (!no_sandbox() && sandbox_trouble).then_some(
        "Chromium couldn't start with its sandbox (this host may not allow unprivileged user namespaces). Fix the host, or set BROWSER_NO_SANDBOX=1 to run it unsandboxed knowingly",
    )
}

async fn devtools_url(child: &mut Child) -> Result<String, String> {
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let mut lines = BufReader::new(stderr).lines();
    let mut seen = String::new();
    let find = async {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(rest) = line.split("DevTools listening on ").nth(1) {
                return Some(rest.trim().to_string());
            }
            if seen.len() < 4000 {
                seen.push_str(&line);
                seen.push('\n');
            }
        }
        None
    };
    let found = tokio::time::timeout(Duration::from_secs(20), find).await;
    let url = match found {
        Ok(Some(url)) => url,
        other => {
            if let Some(hint) = launch_hint(&seen) {
                tracing::error!("[browser] {hint}");
            }
            return Err(match other {
                Err(_) => "Chromium didn't start in time".to_string(),
                _ => "Chromium exited before it was ready".to_string(),
            });
        }
    };
    // Keep draining stderr so Chromium never blocks on a full pipe
    tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
    Ok(url)
}

struct Cdp {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
    next_id: u64,
    session: Option<String>,
}

impl Cdp {
    async fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let mut msg = json!({"id": id, "method": method, "params": params});
        if let Some(s) = &self.session {
            msg["sessionId"] = json!(s);
        }
        self.ws
            .send(Message::Text(msg.to_string()))
            .await
            .map_err(|e| e.to_string())?;
        while let Some(frame) = self.ws.next().await {
            let Message::Text(text) = frame.map_err(|e| e.to_string())? else {
                continue;
            };
            let v: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if v.get("id").and_then(Value::as_u64) != Some(id) {
                continue; // an event or another reply
            }
            if let Some(err) = v.get("error") {
                return Err(format!(
                    "{method}: {}",
                    err["message"].as_str().unwrap_or("failed")
                ));
            }
            return Ok(v.get("result").cloned().unwrap_or(Value::Null));
        }
        Err("DevTools connection closed".into())
    }

    async fn eval(&mut self, expression: &str) -> Result<Value, String> {
        let r = self
            .call(
                "Runtime.evaluate",
                json!({"expression": expression, "returnByValue": true}),
            )
            .await?;
        Ok(r.pointer("/result/value").cloned().unwrap_or(Value::Null))
    }

    async fn title(&mut self) -> String {
        self.eval("document.title")
            .await
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    }
}

async fn drive(child: &mut Child, url: &str, timeout: Duration) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    let ws_url = devtools_url(child).await?;
    let (ws, _) = tokio_tungstenite::connect_async(ws_url.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let mut cdp = Cdp {
        ws,
        next_id: 0,
        session: None,
    };

    let target = cdp
        .call("Target.createTarget", json!({"url": "about:blank"}))
        .await?;
    let target_id = target["targetId"].as_str().ok_or("no target")?.to_string();
    let attached = cdp
        .call(
            "Target.attachToTarget",
            json!({"targetId": target_id, "flatten": true}),
        )
        .await?;
    cdp.session = attached["sessionId"].as_str().map(String::from);

    cdp.call("Network.enable", json!({})).await?;
    cdp.call("Network.setBlockedURLs", json!({"urls": BLOCKED}))
        .await?;
    cdp.call(
        "Network.setExtraHTTPHeaders",
        json!({"headers": {"Accept-Language": "en-US,en;q=0.9"}}),
    )
    .await?;
    cdp.call("Page.enable", json!({})).await?;
    // Look less like automation
    cdp.call(
        "Page.addScriptToEvaluateOnNewDocument",
        json!({"source": "Object.defineProperty(navigator, 'webdriver', { get: () => undefined })"}),
    )
    .await?;

    let nav = cdp.call("Page.navigate", json!({"url": url})).await?;
    if let Some(err) = nav
        .get("errorText")
        .and_then(Value::as_str)
        .filter(|e| !e.is_empty())
    {
        return Err(format!("Couldn't load the page ({err})"));
    }

    // Wait for the DOM
    while Instant::now() < deadline {
        let state = cdp.eval("document.readyState").await.unwrap_or(Value::Null);
        if matches!(state.as_str(), Some("interactive" | "complete")) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // Bot challenges usually redirect or reload once solved; give them a moment
    for _ in 0..8 {
        if !is_challenge_title(&cdp.title().await) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
    }

    // Wait briefly for client-rendered recipe data (JSON-LD or ingredient lists)
    let remaining = deadline.saturating_duration_since(Instant::now());
    let wait_until =
        Instant::now() + remaining.clamp(Duration::from_secs(1), Duration::from_secs(8));
    while Instant::now() < wait_until {
        if cdp.eval(RECIPE_READY).await.ok().and_then(|v| v.as_bool()) == Some(true) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    if is_challenge_title(&cdp.title().await) {
        return Err("Blocked by the site".into());
    }
    let html = cdp.eval("document.documentElement.outerHTML").await?;
    cdp.session = None;
    let _ = cdp.call("Browser.close", json!({})).await;
    html.as_str()
        .map(String::from)
        .ok_or_else(|| "Empty page".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With a Chromium installed (skipped without one): pages on the server's own network
    /// are never reached, not even the one Chromium is sent to.
    #[tokio::test]
    async fn chromium_cannot_reach_loopback() {
        let browser = Browser::from_env();
        let Some(exe) = browser.executable.as_deref().filter(|p| p.is_file()) else {
            return;
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = hits.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let body = "<html><body>secret</body></html>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes()).await;
            }
        });
        let result = run(exe, &format!("http://{addr}/"), Duration::from_secs(15)).await;
        assert_eq!(
            hits.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "{result:?}"
        );
        assert!(result.is_err(), "the proxy turns the page away");
    }

    #[test]
    fn user_agent_follows_the_installed_version() {
        let ua = user_agent_for("Chromium 140.0.7339.185 built on Debian GNU/Linux 12 (bookworm)")
            .unwrap();
        assert!(ua.contains("Chrome/140.0.0.0 Safari/537.36"), "{ua}");
        assert!(!ua.contains("Headless"));
        assert!(user_agent_for("Google Chrome 131.0.6778.85 ").is_some());
        assert_eq!(user_agent_for("chromium: not found"), None);
    }
}
