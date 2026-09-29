//! A small forward proxy that only connects to public addresses, for a browser that runs a
//! stranger's page.
//!
//! Headless Chromium runs the JavaScript of whatever page it is sent to. A page that answers
//! the server's fetch with a bot check (which is what sends us to Chromium) can be a hostile
//! one, and its script could ask for `http://169.254.169.254/` or the auth service on the
//! private network and read the answer. Chromium is launched with `--proxy-server` pointing
//! here, and `--proxy-bypass-list=<-loopback>` so loopback goes through as well. Every
//! request it makes, redirects and sub-resources and script-made ones alike, arrives as a
//! `CONNECT host:port` (https) or an absolute-URI request (http).
//!
//! The name is resolved here, by the same checked resolver as [`fetch_public`](crate::fetch_public)
//! ([`PublicResolver`](crate::guard)), and the connection is made to the address that was
//! checked, so there is no second lookup a rebinding DNS could answer differently. Anything
//! that isn't a public address gets a `403`.
//!
//! Not a general proxy: HTTP/1.1 with `Connection: close` for plain http, tunnels for https,
//! no authentication (it listens on loopback for one browser), no caching. It lives as long
//! as the [`Proxy`] value.

use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use crate::guard::{TargetError, public_targets};

/// Longest a request's head (the request line and headers) may be.
const MAX_HEAD: usize = 16 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Longest one connection lives; a page load is far shorter.
const CONNECTION_LIFETIME: Duration = Duration::from_secs(120);

/// A running proxy. Dropping it stops it.
pub struct Proxy {
    addr: SocketAddr,
    task: JoinHandle<()>,
}

impl Proxy {
    /// Starts a proxy on a free loopback port.
    pub async fn start() -> io::Result<Self> {
        Self::bind(false).await
    }

    /// `allow_private` switches the check off: for the tests, which serve from loopback.
    async fn bind(allow_private: bool) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let _ = tokio::time::timeout(CONNECTION_LIFETIME, serve(socket, allow_private))
                        .await;
                });
            }
        });
        Ok(Self { addr, task })
    }

    /// The address to give the browser: `--proxy-server=http://{addr}`.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Answers with `status` and nothing else.
async fn refuse(client: &mut TcpStream, status: &str) {
    let response = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    let _ = client.write_all(response.as_bytes()).await;
    let _ = client.shutdown().await;
}

/// What a request asked for.
#[derive(Debug, PartialEq, Eq)]
enum Wanted {
    /// `CONNECT host:port`: a tunnel.
    Tunnel { host: String, port: u16 },
    /// `GET http://host:port/path`: a plain request, with the head to send on.
    Plain {
        host: String,
        port: u16,
        head: Vec<u8>,
    },
}

/// `host:port` or `[v6]:port`.
fn host_and_port(authority: &str, default_port: Option<u16>) -> Option<(String, u16)> {
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.ends_with(':') && port.bytes().all(|b| b.is_ascii_digit()) => {
            (host, port.parse().ok()?)
        }
        _ => (authority, default_port?),
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    (!host.is_empty() && port != 0).then(|| (host.to_string(), port))
}

/// What the request head (through the blank line) asks for. `None` if it isn't a request the
/// proxy serves.
fn parse_head(head: &[u8]) -> Option<Wanted> {
    let text = std::str::from_utf8(head).ok()?;
    let (line, headers) = text.split_once("\r\n")?;
    let mut parts = line.split_whitespace();
    let (method, target, version) = (parts.next()?, parts.next()?, parts.next()?);
    if !version.starts_with("HTTP/1.") {
        return None;
    }
    if method.eq_ignore_ascii_case("CONNECT") {
        let (host, port) = host_and_port(target, None)?;
        return Some(Wanted::Tunnel { host, port });
    }
    let url = url::Url::parse(target).ok()?;
    if url.scheme() != "http" || !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let (host, port) = (
        url.host_str()?.to_string(),
        url.port_or_known_default().unwrap_or(80),
    );
    // Origin-form on to the server; one request per connection
    let mut path = url.path().to_string();
    if let Some(query) = url.query() {
        path.push('?');
        path.push_str(query);
    }
    let mut out = format!("{method} {path} {version}\r\n");
    for header in headers.split("\r\n").filter(|h| !h.is_empty()) {
        let name = header
            .split(':')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !matches!(
            name.as_str(),
            "proxy-connection" | "connection" | "proxy-authorization"
        ) {
            out.push_str(header);
            out.push_str("\r\n");
        }
    }
    out.push_str("Connection: close\r\n\r\n");
    Some(Wanted::Plain {
        host,
        port,
        head: out.into_bytes(),
    })
}

/// The first connection to any of `targets` that opens.
async fn connect_any(targets: &[SocketAddr]) -> Option<TcpStream> {
    for addr in targets {
        if let Ok(Ok(stream)) =
            tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await
        {
            return Some(stream);
        }
    }
    None
}

async fn serve(mut client: TcpStream, allow_private: bool) {
    // The head, and whatever followed it in the same reads
    let mut buf = Vec::new();
    let end = loop {
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        if buf.len() > MAX_HEAD {
            return refuse(&mut client, "431 Request Header Fields Too Large").await;
        }
        let mut chunk = [0u8; 4096];
        match client.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let (head, rest) = buf.split_at(end);
    let Some(wanted) = parse_head(head) else {
        return refuse(&mut client, "400 Bad Request").await;
    };
    let (host, port) = match &wanted {
        Wanted::Tunnel { host, port } | Wanted::Plain { host, port, .. } => (host.as_str(), *port),
    };
    let targets = match public_targets(host, port, allow_private).await {
        Ok(targets) => targets,
        Err(TargetError::Forbidden) => return refuse(&mut client, "403 Forbidden").await,
        Err(TargetError::Failed(why)) => {
            tracing::debug!("[proxy] couldn't resolve {host}: {why}");
            return refuse(&mut client, "502 Bad Gateway").await;
        }
    };
    let Some(mut upstream) = connect_any(&targets).await else {
        return refuse(&mut client, "502 Bad Gateway").await;
    };
    match wanted {
        Wanted::Tunnel { .. } => {
            let established = b"HTTP/1.1 200 Connection Established\r\n\r\n";
            if client.write_all(established).await.is_err() {
                return;
            }
        }
        Wanted::Plain { head, .. } => {
            if upstream.write_all(&head).await.is_err() {
                return;
            }
        }
    }
    if !rest.is_empty() && upstream.write_all(rest).await.is_err() {
        return;
    }
    let _ = tokio::io::copy_bidirectional(&mut client, &mut upstream).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_a_browser_asks_for() {
        assert_eq!(
            parse_head(b"CONNECT example.com:443 HTTP/1.1\r\nHost: example.com:443\r\n\r\n"),
            Some(Wanted::Tunnel {
                host: "example.com".into(),
                port: 443
            })
        );
        assert_eq!(
            parse_head(b"CONNECT [2001:db8::1]:8443 HTTP/1.1\r\n\r\n"),
            Some(Wanted::Tunnel {
                host: "2001:db8::1".into(),
                port: 8443
            })
        );
        let Some(Wanted::Plain { host, port, head }) = parse_head(
            b"GET http://example.com/a/b?c=d HTTP/1.1\r\nHost: example.com\r\nProxy-Connection: keep-alive\r\nAccept: */*\r\n\r\n",
        ) else {
            panic!("a plain request");
        };
        assert_eq!((host.as_str(), port), ("example.com", 80));
        assert_eq!(
            String::from_utf8(head).unwrap(),
            "GET /a/b?c=d HTTP/1.1\r\nHost: example.com\r\nAccept: */*\r\nConnection: close\r\n\r\n"
        );
        // Not served: no port for a tunnel, https in the clear, a request that isn't absolute
        assert_eq!(parse_head(b"CONNECT example.com HTTP/1.1\r\n\r\n"), None);
        assert_eq!(
            parse_head(b"GET https://example.com/ HTTP/1.1\r\n\r\n"),
            None
        );
        assert_eq!(parse_head(b"GET /path HTTP/1.1\r\n\r\n"), None);
        assert_eq!(
            parse_head(b"GET http://u:p@example.com/ HTTP/1.1\r\n\r\n"),
            None
        );
        assert_eq!(parse_head(b"nonsense\r\n\r\n"), None);
    }

    /// Sends one request to the proxy and returns the first line of its answer.
    async fn ask(proxy: &Proxy, request: &str) -> String {
        let mut socket = TcpStream::connect(proxy.addr()).await.unwrap();
        socket.write_all(request.as_bytes()).await.unwrap();
        let mut answer = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(5), socket.read_to_end(&mut answer)).await;
        String::from_utf8_lossy(&answer)
            .lines()
            .next()
            .unwrap_or("")
            .to_string()
    }

    /// A server that says "hello" to whoever connects, and counts them.
    async fn hello() -> (SocketAddr, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = seen.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = socket.write_all(b"hello").await;
                let _ = socket.shutdown().await;
            }
        });
        (addr, seen)
    }

    #[tokio::test]
    async fn private_targets_get_a_403_and_are_never_connected_to() {
        let proxy = Proxy::start().await.unwrap();
        let (local, seen) = hello().await;
        for request in [
            format!("CONNECT {local} HTTP/1.1\r\n\r\n"),
            format!("GET http://{local}/ HTTP/1.1\r\nHost: {local}\r\n\r\n"),
            format!("GET http://localhost:{}/ HTTP/1.1\r\n\r\n", local.port()),
            "CONNECT 169.254.169.254:80 HTTP/1.1\r\n\r\n".to_string(),
            "CONNECT 10.0.0.5:443 HTTP/1.1\r\n\r\n".to_string(),
            "CONNECT [::1]:443 HTTP/1.1\r\n\r\n".to_string(),
            "GET http://192.168.1.1/admin HTTP/1.1\r\n\r\n".to_string(),
            "CONNECT 100.100.100.100:80 HTTP/1.1\r\n\r\n".to_string(),
        ] {
            let answer = ask(&proxy, &request).await;
            assert_eq!(answer, "HTTP/1.1 403 Forbidden", "{request}");
        }
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 0);
        // Requests it doesn't serve are turned away too
        assert_eq!(
            ask(&proxy, "GET /local HTTP/1.1\r\n\r\n").await,
            "HTTP/1.1 400 Bad Request"
        );
    }

    #[tokio::test]
    async fn a_permitted_target_is_tunnelled_and_proxied() {
        // With the check off (the test server is on loopback) the plumbing works
        let proxy = Proxy::bind(true).await.unwrap();
        let (local, seen) = hello().await;
        let mut socket = TcpStream::connect(proxy.addr()).await.unwrap();
        socket
            .write_all(format!("CONNECT {local} HTTP/1.1\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut answer = Vec::new();
        socket.read_to_end(&mut answer).await.unwrap();
        let answer = String::from_utf8_lossy(&answer);
        assert!(
            answer.starts_with("HTTP/1.1 200 Connection Established\r\n\r\nhello"),
            "{answer}"
        );

        let mut socket = TcpStream::connect(proxy.addr()).await.unwrap();
        socket
            .write_all(format!("GET http://{local}/x HTTP/1.1\r\nHost: {local}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut answer = Vec::new();
        socket.read_to_end(&mut answer).await.unwrap();
        assert_eq!(String::from_utf8_lossy(&answer), "hello");
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 2);
    }
}
