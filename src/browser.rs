//! Headless Chromium fallback for sites that block plain HTTP fetches.
//!
//! Chromium is installed in the Docker image; locally set CHROMIUM_PATH or it's skipped.
//! Only one page loads at a time to keep memory in check on a small instance. The browser
//! is driven over the DevTools protocol directly (a handful of commands), which keeps the
//! binary small compared with a full CDP client.

use futures_util::{SinkExt, StreamExt};
use regex::Regex;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
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

pub struct Browser {
    executable: Option<PathBuf>,
    lock: Mutex<()>,
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
        }
    }

    pub fn disabled() -> Self {
        Self {
            executable: None,
            lock: Mutex::new(()),
        }
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
        let _guard = self.lock.lock().await;
        let timeout = Duration::from_secs(40);
        tokio::time::timeout(timeout + Duration::from_secs(5), run(exe, url, timeout))
            .await
            .map_err(|_| "Timed out loading the page".to_string())?
    }
}

async fn run(exe: &Path, url: &str, timeout: Duration) -> Result<String, String> {
    let profile = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut command = Command::new(exe);
    if let Some(ua) = user_agent(exe).await {
        command.arg(format!("--user-agent={ua}"));
    }
    let mut child = command
        .args([
            "--headless=new",
            "--no-sandbox",
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
    result
}

async fn devtools_url(child: &mut Child) -> Result<String, String> {
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let mut lines = BufReader::new(stderr).lines();
    let find = async {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(rest) = line.split("DevTools listening on ").nth(1) {
                return Some(rest.trim().to_string());
            }
        }
        None
    };
    let url = tokio::time::timeout(Duration::from_secs(20), find)
        .await
        .map_err(|_| "Chromium didn't start in time".to_string())?
        .ok_or("Chromium exited before it was ready")?;
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
        if !crate::scraper::is_challenge_title(&cdp.title().await) {
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

    if crate::scraper::is_challenge_title(&cdp.title().await) {
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
