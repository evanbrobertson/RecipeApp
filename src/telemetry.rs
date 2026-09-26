//! Error reporting and request tracing with Sentry, for the server and (through a
//! `Server-Timing` hint on page loads) the browser.
//!
//! Everything is off unless `SENTRY_DSN` is set, so self-hosted copies report nothing. Recipes
//! stay private: no cookies, auth headers, query strings, bodies or user data are sent, log lines
//! below WARN never leave the process, and WARN lines only travel as breadcrumbs on an error.
//! Every message that does leave (breadcrumbs, errors, panics) has its URLs cut to the host and
//! is capped in length ([`scrub_text`]); log lines name hosts, not recipe URLs.

use std::sync::{Arc, OnceLock};

use axum::extract::{MatchedPath, Request};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use sentry::integrations::tracing::EventFilter;
use sentry::protocol::{self, SpanStatus};
use sentry::{Hub, SentryFutureExt, TransactionContext};
use tracing::Level;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::registry::LookupSpan;

/// Request headers worth keeping on an event; everything else (cookies, auth, IPs) is dropped.
const KEPT_HEADERS: &[&str] = &[
    "accept",
    "content-length",
    "content-type",
    "sec-fetch-dest",
    "sec-fetch-mode",
    "user-agent",
];

/// Longest message (log line, panic, error) that leaves the process, in characters.
const MAX_MESSAGE: usize = 200;

/// A log line or panic message made safe to send: every URL cut down to its scheme and host
/// (recipe URLs and their query strings are private), then capped at [`MAX_MESSAGE`].
pub fn scrub_text(s: &str) -> String {
    static URL: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(
            r#"(?i)\b([a-z][a-z0-9+.-]*)://(?:[^\s/?#@"'<>]*@)?([^\s/?#:"'<>()\[\]]+)[^\s"'<>()\[\]]*"#,
        )
        .unwrap()
    });
    let hosts = URL.replace_all(s, |c: &regex::Captures| {
        // Sentence punctuation right after a URL isn't part of it
        let whole = &c[0];
        let tail = &whole[whole.trim_end_matches(['.', ',', ':', ';', '!', '?']).len()..];
        let host = c[2].trim_end_matches(['.', ',', ':', ';', '!', '?']);
        format!("{}://{host}{tail}", &c[1])
    });
    let hosts = redact_path(&hosts).into_owned();
    match hosts.char_indices().nth(MAX_MESSAGE) {
        Some((at, _)) => format!("{}…", &hosts[..at]),
        None => hosts,
    }
}

/// A path with any share token hidden: `/s/{token}/og.jpg` → `/s/[token]/og.jpg`. The
/// token is the share, so it never goes into a span, an event or a log line. `/api/shares/…`
/// is hidden too, though nothing routes there now (the API names shares by recipe id).
pub fn redact_path(path: &str) -> std::borrow::Cow<'_, str> {
    static SHARE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(^|/)(s|shares)/[^/?#\s]+").unwrap());
    if !path.contains("s/") {
        return path.into();
    }
    SHARE.replace_all(path, "${1}${2}/[token]")
}

/// A URL's host for a log line (`example.com`), never its path or query.
pub fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| "an unparseable URL".into())
}

/// Longest API error body kept in a log line or a stored error, in characters.
const MAX_API_ERROR: usize = 300;

/// An API error body cut down for a log line: the provider's error message when the body is
/// JSON (not any echoed request, which may hold recipe text), capped at 300 characters.
pub fn api_error_text(body: &str) -> String {
    let message = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            let text = |v: &serde_json::Value| v.as_str().map(str::to_owned);
            text(&v["error"]["message"])
                .or_else(|| text(&v["message"]))
                .or_else(|| text(&v["error"]))
                .or_else(|| text(&v["detail"]))
                .or_else(|| {
                    // A list of validation problems: their messages only
                    let list = v["detail"].as_array()?;
                    let msgs: Vec<String> = list.iter().filter_map(|d| text(&d["msg"])).collect();
                    (!msgs.is_empty()).then(|| msgs.join("; "))
                })
                .or_else(|| Some("(no error message)".into()))
        })
        .unwrap_or_else(|| body.trim().to_owned());
    match message.char_indices().nth(MAX_API_ERROR) {
        Some((at, _)) => format!("{}…", &message[..at]),
        None => message,
    }
}

fn scrub_value(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::String(s) => *s = scrub_text(s),
        serde_json::Value::Array(items) => items.iter_mut().for_each(scrub_value),
        serde_json::Value::Object(map) => map.values_mut().for_each(scrub_value),
        _ => {}
    }
}

fn scrub_breadcrumb(crumb: &mut protocol::Breadcrumb) {
    if let Some(m) = crumb.message.as_mut() {
        *m = scrub_text(m);
    }
    crumb.data.values_mut().for_each(scrub_value);
}

/// Messages, panic and error values, log fields and breadcrumbs on an outgoing event.
fn scrub_event(event: &mut protocol::Event<'_>) {
    if let Some(m) = event.message.as_mut() {
        *m = scrub_text(m);
    }
    if let Some(entry) = event.logentry.as_mut() {
        entry.message = scrub_text(&entry.message);
        entry.params.iter_mut().for_each(scrub_value);
    }
    for e in &mut event.exception.values {
        if let Some(v) = e.value.as_mut() {
            *v = scrub_text(v);
        }
    }
    if let Some(culprit) = event.culprit.as_mut() {
        *culprit = scrub_text(culprit);
    }
    event.extra.values_mut().for_each(scrub_value);
    for context in event.contexts.values_mut() {
        if let protocol::Context::Other(map) = context {
            map.values_mut().for_each(scrub_value);
        }
    }
    event
        .breadcrumbs
        .values
        .iter_mut()
        .for_each(scrub_breadcrumb);
}

/// Routes not worth a transaction: health probes and resized photos (a share's photos
/// and export share the `/s/{token}/{*rest}` route).
const UNTRACED_PREFIXES: &[&str] = &["/api/health", "/img/", "/s/{token}/"];

/// What the environment asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub dsn: Option<String>,
    pub environment: String,
    pub release: String,
    pub traces_sample_rate: f32,
    /// Tell the browser SDK where to report (off with `SENTRY_BROWSER=off`).
    pub browser: bool,
}

impl Settings {
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Self {
        let get = |key: &str| {
            get(key)
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        };
        Self {
            dsn: get("SENTRY_DSN"),
            environment: get("SENTRY_ENVIRONMENT").unwrap_or_else(|| "production".into()),
            release: get("SENTRY_RELEASE")
                .unwrap_or_else(|| concat!("crumb@", env!("CARGO_PKG_VERSION")).into()),
            traces_sample_rate: get("SENTRY_TRACES_SAMPLE_RATE")
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .map_or(0.1, |v| v.clamp(0.0, 1.0)),
            browser: !matches!(
                get("SENTRY_BROWSER").as_deref(),
                Some("off" | "false" | "0" | "no")
            ),
        }
    }

    /// The `Server-Timing` value page loads carry so the browser SDK can start without a
    /// build-time DSN: one image serves every environment, and a copy without `SENTRY_DSN`
    /// sends nothing from the browser either.
    fn browser_hint(&self) -> Option<HeaderValue> {
        let dsn = self.dsn.as_deref().filter(|_| self.browser)?;
        let quoted = |v: &str| v.replace(['"', '\\'], "");
        HeaderValue::from_str(&format!(
            "sentry;desc=\"{}\", sentry-env;desc=\"{}\", sentry-release;desc=\"{}\"",
            quoted(dsn),
            quoted(&self.environment),
            quoted(&self.release)
        ))
        .ok()
    }
}

#[derive(Default)]
struct State {
    enabled: bool,
    traces: bool,
    browser_hint: Option<HeaderValue>,
}

static STATE: OnceLock<State> = OnceLock::new();

/// Starts the Sentry client. Call before the async runtime starts and keep the guard alive for
/// the life of the process (dropping it flushes queued events). `None` when reporting is off.
pub fn init(settings: &Settings) -> Option<sentry::ClientInitGuard> {
    let dsn = settings
        .dsn
        .as_deref()
        .filter(|dsn| dsn.parse::<sentry::types::Dsn>().is_ok());
    let Some(dsn) = dsn else {
        let _ = STATE.set(State::default());
        return None;
    };
    let guard = sentry::init(
        sentry::ClientOptions::new()
            .dsn(dsn)
            .release(settings.release.clone())
            .environment(settings.environment.clone())
            .traces_sample_rate(settings.traces_sample_rate)
            .send_default_pii(false)
            .max_breadcrumbs(30)
            .before_send(|mut event| {
                event.user = None;
                if let Some(request) = event.request.as_mut() {
                    scrub(request);
                }
                scrub_event(&mut event);
                Some(event)
            })
            .before_breadcrumb(|mut crumb| {
                scrub_breadcrumb(&mut crumb);
                Some(crumb)
            }),
    );
    let _ = STATE.set(State {
        enabled: true,
        traces: settings.traces_sample_rate > 0.0,
        browser_hint: settings.browser_hint(),
    });
    Some(guard)
}

/// One startup log line saying whether (and how) reporting is on.
pub fn describe(settings: &Settings, enabled: bool) -> String {
    match (&settings.dsn, enabled) {
        (None, _) => "Sentry off (SENTRY_DSN not set)".into(),
        (Some(_), false) => "Sentry off: SENTRY_DSN is not a valid DSN".into(),
        (Some(_), true) => format!(
            "Sentry on: environment {}, release {}, traces {}{}",
            settings.environment,
            settings.release,
            settings.traces_sample_rate,
            if settings.browser {
                ", browser too"
            } else {
                ""
            }
        ),
    }
}

/// Errors become Sentry events and warnings their breadcrumbs; nothing quieter is looked at.
pub fn tracing_layer<S>() -> impl Layer<S>
where
    S: tracing::Subscriber + for<'a> LookupSpan<'a>,
{
    sentry::integrations::tracing::layer()
        .event_filter(|meta| match *meta.level() {
            Level::ERROR => EventFilter::Event,
            Level::WARN => EventFilter::Breadcrumb,
            _ => EventFilter::Ignore,
        })
        .span_filter(|_| false)
        .with_filter(LevelFilter::WARN)
}

/// Router middleware: a transaction per routed request (continuing the browser's trace), and
/// the browser hint on HTML page loads. A no-op when reporting is off.
pub async fn middleware(req: Request, next: Next) -> Response {
    let Some(state) = STATE.get().filter(|s| s.enabled) else {
        return next.run(req).await;
    };
    let document = req
        .headers()
        .get("sec-fetch-dest")
        .is_some_and(|v| v == "document");
    // Share pages run no Sentry: whoever opens a link isn't the cook
    let share = req.uri().path().starts_with("/s/");
    let name = state.traces.then(|| transaction_name(&req)).flatten();
    let mut res = match name {
        Some(name) => traced(req, next, name).await,
        None => next.run(req).await,
    };
    if let Some(hint) = &state.browser_hint {
        let html = res
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/html"));
        if (document || html) && !share {
            res.headers_mut()
                .append(HeaderName::from_static("server-timing"), hint.clone());
        }
    }
    res
}

/// `GET /api/recipes/{id}` style names (the route, never the raw path), or `None` for static
/// files and the routes in [`UNTRACED_PREFIXES`].
fn transaction_name(req: &Request) -> Option<String> {
    let route = req.extensions().get::<MatchedPath>()?.as_str();
    if UNTRACED_PREFIXES.iter().any(|p| route.starts_with(p)) {
        return None;
    }
    Some(format!("{} {route}", req.method()))
}

async fn traced(req: Request, next: Next, name: String) -> Response {
    let ctx = TransactionContext::continue_from_headers(
        &name,
        "http.server",
        req.headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str(), v.to_str().ok()?))),
    );
    let request = event_request(req.method().as_str(), req.uri().path(), req.headers());
    let hub = Arc::new(Hub::new_from_top(Hub::current()));
    async move {
        let transaction = sentry::start_transaction(ctx);
        transaction.set_request(request.clone());
        sentry::configure_scope(|scope| {
            scope.set_span(Some(transaction.clone().into()));
            scope.add_event_processor(move |mut event| {
                event.request.get_or_insert_with(|| request.clone());
                Some(event)
            });
        });
        let res = next.run(req).await;
        transaction.set_data("http.response.status_code", res.status().as_u16().into());
        transaction.set_status(span_status(res.status()));
        transaction.finish();
        res
    }
    .bind_hub(hub)
    .await
}

/// The request as Sentry sees it: method, path (no query) and a few harmless headers.
fn event_request(method: &str, path: &str, headers: &HeaderMap) -> protocol::Request {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let mut request = protocol::Request {
        method: Some(method.to_owned()),
        url: format!("https://{host}{}", redact_path(path)).parse().ok(),
        headers: headers
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_owned(), v.to_str().ok()?.to_owned())))
            .collect(),
        ..Default::default()
    };
    scrub(&mut request);
    request
}

fn scrub(request: &mut protocol::Request) {
    request.query_string = None;
    request.cookies = None;
    request.data = None;
    request.env.clear();
    if let Some(url) = request.url.as_mut() {
        url.set_query(None);
        url.set_fragment(None);
        let _ = url.set_username("");
        let _ = url.set_password(None);
    }
    request
        .headers
        .retain(|k, _| KEPT_HEADERS.contains(&k.to_ascii_lowercase().as_str()));
}

fn span_status(status: StatusCode) -> SpanStatus {
    match status {
        s if s.is_success() || s.is_redirection() => SpanStatus::Ok,
        StatusCode::UNAUTHORIZED => SpanStatus::Unauthenticated,
        StatusCode::FORBIDDEN => SpanStatus::PermissionDenied,
        StatusCode::NOT_FOUND => SpanStatus::NotFound,
        StatusCode::TOO_MANY_REQUESTS => SpanStatus::ResourceExhausted,
        s if s.is_client_error() => SpanStatus::InvalidArgument,
        StatusCode::SERVICE_UNAVAILABLE => SpanStatus::Unavailable,
        _ => SpanStatus::InternalError,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(pairs: &[(&str, &str)]) -> Settings {
        let map: std::collections::HashMap<_, _> = pairs.iter().copied().collect();
        Settings::from_lookup(|k| map.get(k).map(|v| v.to_string()))
    }

    #[test]
    fn off_without_a_dsn() {
        let s = settings(&[("SENTRY_DSN", "  ")]);
        assert_eq!(s.dsn, None);
        assert_eq!(s.environment, "production");
        assert_eq!(s.traces_sample_rate, 0.1);
        assert!(s.release.starts_with("crumb@"));
        assert_eq!(s.browser_hint(), None);
    }

    #[test]
    fn reads_rates_and_switches() {
        let s = settings(&[
            ("SENTRY_DSN", "https://key@o1.ingest.sentry.io/2"),
            ("SENTRY_TRACES_SAMPLE_RATE", "7"),
            ("SENTRY_ENVIRONMENT", "dev"),
            ("SENTRY_RELEASE", "1.2.0-main.3"),
        ]);
        assert_eq!(s.traces_sample_rate, 1.0);
        let hint = s.browser_hint().unwrap();
        assert_eq!(
            hint.to_str().unwrap(),
            "sentry;desc=\"https://key@o1.ingest.sentry.io/2\", sentry-env;desc=\"dev\", \
             sentry-release;desc=\"1.2.0-main.3\""
        );
        let off = settings(&[("SENTRY_DSN", "https://k@h/1"), ("SENTRY_BROWSER", "off")]);
        assert_eq!(off.browser_hint(), None);
        let bad = settings(&[("SENTRY_TRACES_SAMPLE_RATE", "lots")]);
        assert_eq!(bad.traces_sample_rate, 0.1);
    }

    #[test]
    fn log_lines_lose_urls_and_length() {
        assert_eq!(
            scrub_text(
                "[scrape] failed https://user:pw@Example.com:8080/p/secret-pie?x=1#top: 403"
            ),
            "[scrape] failed https://Example.com: 403"
        );
        assert_eq!(
            scrub_text("fetch (http://a.test/b?q) and <https://c.test/d>"),
            "fetch (http://a.test) and <https://c.test>"
        );
        assert_eq!(scrub_text("no links here"), "no links here");
        let long = "é".repeat(500);
        let out = scrub_text(&long);
        assert_eq!(out.chars().count(), MAX_MESSAGE + 1);
        assert!(out.ends_with('…'));

        let mut crumb = protocol::Breadcrumb {
            message: Some("GET https://site.test/r/1?token=abc".into()),
            ..Default::default()
        };
        crumb
            .data
            .insert("url".into(), "https://site.test/r/2#frag".into());
        scrub_breadcrumb(&mut crumb);
        assert_eq!(crumb.message.as_deref(), Some("GET https://site.test"));
        assert_eq!(crumb.data["url"], "https://site.test");

        let mut event = protocol::Event {
            message: Some("x".repeat(1000)),
            ..Default::default()
        };
        event.exception.values.push(protocol::Exception {
            ty: "panic".into(),
            value: Some(format!("bad recipe https://s.test/a?b {}", "y".repeat(400))),
            ..Default::default()
        });
        scrub_event(&mut event);
        assert!(event.message.unwrap().chars().count() <= MAX_MESSAGE + 1);
        let value = event.exception.values[0].value.clone().unwrap();
        assert!(value.starts_with("bad recipe https://s.test y"));
        assert!(value.chars().count() <= MAX_MESSAGE + 1);
    }

    #[test]
    fn api_errors_keep_only_the_message() {
        assert_eq!(
            api_error_text(
                r#"{"error":{"type":"invalid","message":"max_tokens too big"},"echo":"secret pie"}"#
            ),
            "max_tokens too big"
        );
        assert_eq!(
            api_error_text(
                r#"{"detail":[{"loc":["state"],"msg":"field required","input":"secret pie"}]}"#
            ),
            "field required"
        );
        assert_eq!(
            api_error_text(r#"{"state":{"title":"secret pie"}}"#),
            "(no error message)"
        );
        assert_eq!(api_error_text(" bad request \n"), "bad request");
        assert_eq!(api_error_text(&"z".repeat(1000)).chars().count(), 301);
        assert_eq!(host_of("https://site.test/p/pie?x=1"), "site.test");
        assert_eq!(host_of("nope"), "an unparseable URL");
    }

    #[test]
    fn requests_lose_private_parts() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "crumb.example".parse().unwrap());
        headers.insert(header::COOKIE, "crumb_session=secret".parse().unwrap());
        headers.insert(header::AUTHORIZATION, "Bearer secret".parse().unwrap());
        headers.insert("x-forwarded-for", "203.0.113.9".parse().unwrap());
        headers.insert(header::USER_AGENT, "test".parse().unwrap());
        let shared = event_request("GET", "/s/AbC-123_xyzAbC-123_xyz/og.jpg", &headers);
        assert_eq!(
            shared.url.unwrap().as_str(),
            "https://crumb.example/s/[token]/og.jpg"
        );
        assert_eq!(redact_path("/s/tok"), "/s/[token]");
        assert_eq!(redact_path("/api/shares/tok"), "/api/shares/[token]");
        assert_eq!(redact_path("/api/recipes/1/share"), "/api/recipes/1/share");
        assert_eq!(redact_path("/sx/tok"), "/sx/tok");
        assert_eq!(redact_path("/recipes/1"), "/recipes/1");
        assert_eq!(
            scrub_text("share page /s/secretToken123/crumb.json failed"),
            "share page /s/[token]/crumb.json failed"
        );
        let r = event_request("GET", "/api/recipes", &headers);
        assert_eq!(r.url.unwrap().as_str(), "https://crumb.example/api/recipes");
        assert_eq!(r.headers.len(), 1);
        assert_eq!(
            r.headers.get("user-agent").map(String::as_str),
            Some("test")
        );

        let mut raw = protocol::Request {
            url: "https://u:p@crumb.example/oauth/token?code=abc#x"
                .parse()
                .ok(),
            query_string: Some("code=abc".into()),
            cookies: Some("a=b".into()),
            data: Some("{\"title\":\"Nan's pie\"}".into()),
            ..Default::default()
        };
        scrub(&mut raw);
        assert_eq!(
            raw.url.unwrap().as_str(),
            "https://crumb.example/oauth/token"
        );
        assert!(raw.query_string.is_none() && raw.cookies.is_none() && raw.data.is_none());
    }
}
