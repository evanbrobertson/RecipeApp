//! Fetching on someone else's behalf without becoming a way into the network the fetcher sits
//! on. `crumb-relay` runs on a Raspberry Pi or a home-lab box, next to a router, a NAS and a
//! tailnet: a request for `http://192.168.1.1/` or `http://100.100.100.100/` must not be
//! answered.
//!
//! What is refused, before any connection:
//! - anything but `http` and `https`, on any port but 80 and 443, or with a user name in it
//! - a host that is an IP address outside the public internet (see [`is_public_ip`])
//! - a name that resolves to *any* such address (one bad answer among good ones is refused)
//!
//! Redirects are followed by hand, at most [`MAX_REDIRECTS`], and every hop goes through the
//! same checks.
//!
//! DNS rebinding: names are not resolved once and then trusted. The client's own resolver
//! ([`PublicResolver`]) is the checked one, so the addresses `wreq` connects to are exactly the
//! ones that were checked; there is no second lookup an attacker's DNS could answer
//! differently. (The lookup made first, to answer a bad name with a clear error, is only for
//! the message.) Proxies are off for the same reason: a proxy would resolve the name itself.
//!
//! Not covered: this checks where a connection goes, not what a public site says.
//!
//! The server's own fetches of links its cooks paste use a lighter form of the same guard:
//! [`check_resolved`] on the first link (any port: recipe sites do run on 8080), and on
//! every redirect [`server_redirects`], with [`PublicResolver`] as the client's resolver.
//! Unlike the relay's it lets a proxy configured in the environment stand, since the
//! operator chose it.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use url::{Host, Url};
use wreq::dns::{Addrs, Name, Resolve, Resolving};

use crate::{Fetched, Profile, page_of, public_client};

/// Redirects followed, each checked like the first request.
pub const MAX_REDIRECTS: usize = 10;
/// The whole chain of redirects, whatever each hop's own timeout.
const TOTAL_TIMEOUT: Duration = Duration::from_secs(30);

/// Why an address was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forbidden(pub String);

impl fmt::Display for Forbidden {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Forbidden {}

fn forbidden<T>(why: &str) -> Result<T, Forbidden> {
    Err(Forbidden(why.into()))
}

/// Whether `ip` is an address on the public internet. Everything else (loopback, private
/// ranges, link-local, CGNAT and so Tailscale, multicast, documentation, reserved) is not.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => is_public_v6(ip),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(a == 0 // 0.0.0.0/8: "this network", including 0.0.0.0
        || a == 10 // 10.0.0.0/8
        || (a == 100 && b & 0xc0 == 64) // 100.64.0.0/10: carrier-grade NAT, and Tailscale
        || a == 127 // loopback
        || (a == 169 && b == 254) // link-local, including cloud metadata (169.254.169.254)
        || (a == 172 && b & 0xf0 == 16) // 172.16.0.0/12
        || (a == 192 && b == 0 && c == 0) // 192.0.0.0/24: IETF protocol assignments
        || (a == 192 && b == 0 && c == 2) // 192.0.2.0/24: documentation
        || (a == 192 && b == 88 && c == 99) // 192.88.99.0/24: 6to4 relay anycast
        || (a == 192 && b == 168) // 192.168.0.0/16
        || (a == 198 && b & 0xfe == 18) // 198.18.0.0/15: benchmarking
        || (a == 198 && b == 51 && c == 100) // documentation
        || (a == 203 && b == 0 && c == 113) // documentation
        || a >= 224) // 224.0.0.0/4 multicast, 240.0.0.0/4 reserved, 255.255.255.255 broadcast
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    // ::ffff:a.b.c.d is that IPv4 address
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_public_v4(v4);
    }
    let s = ip.segments();
    let embedded = |hi: u16, lo: u16| {
        let [a, b] = hi.to_be_bytes();
        let [c, d] = lo.to_be_bytes();
        Ipv4Addr::new(a, b, c, d)
    };
    // 64:ff9b::/96 (NAT64) carries an IPv4 address in its last 32 bits
    if s[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        return is_public_v4(embedded(s[6], s[7]));
    }
    // 2002::/16 (6to4) carries one right after the prefix
    if s[0] == 0x2002 {
        return is_public_v4(embedded(s[1], s[2]));
    }
    // Only global unicast, 2000::/3, is let through: that leaves out ::, ::1, IPv4-compatible
    // ::a.b.c.d, unique-local fc00::/7, link-local fe80::/10, site-local fec0::/10 and
    // multicast ff00::/8 without listing them
    if s[0] & 0xe000 != 0x2000 {
        return false;
    }
    match (s[0], s[1]) {
        (0x2001, 0..=0x01ff) => false, // 2001::/23: protocol assignments, Teredo, ORCHID
        (0x2001, 0x0db8) => false,     // 2001:db8::/32: documentation
        (0x3fff, 0..=0x0fff) => false, // 3fff::/20: documentation
        _ => true,
    }
}

/// Checks everything about `url` that needs no lookup: the scheme, the port, no credentials,
/// and, when the host is an address, that it is a public one. A host name is checked when it
/// is resolved.
pub fn check_url(url: &Url) -> Result<(), Forbidden> {
    check(url, true)
}

/// [`check_url`] for the server's own fetches: the same, but any port.
fn check_any_port(url: &Url) -> Result<(), Forbidden> {
    check(url, false)
}

fn check(url: &Url, ports: bool) -> Result<(), Forbidden> {
    if !matches!(url.scheme(), "http" | "https") {
        return forbidden("Only http and https links can be fetched.");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return forbidden("Links with a user name or password can't be fetched.");
    }
    // `port()` is None for the scheme's own default
    if ports && url.port().is_some_and(|p| p != 80 && p != 443) {
        return forbidden("Only ports 80 and 443 can be fetched.");
    }
    match url.host() {
        None => forbidden("The link has no host."),
        Some(Host::Ipv4(ip)) if !is_public_v4(ip) => forbidden("That address isn't public."),
        Some(Host::Ipv6(ip)) if !is_public_v6(ip) => forbidden("That address isn't public."),
        Some(Host::Domain(name)) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            if name.is_empty() || name == "localhost" || name.ends_with(".localhost") {
                return forbidden("That host isn't public.");
            }
            Ok(())
        }
        Some(_) => Ok(()),
    }
}

/// Why a lookup gave no usable addresses.
#[derive(Debug)]
enum LookupError {
    /// It answered with an address that isn't public.
    Forbidden,
    /// It didn't answer.
    Failed(String),
}

/// Resolves `host` and returns its addresses only if every one is public.
async fn lookup_public(host: &str) -> Result<Vec<SocketAddr>, LookupError> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, 0))
        .await
        .map_err(|e| LookupError::Failed(e.to_string()))?
        .collect();
    if addrs.is_empty() {
        return Err(LookupError::Failed("no addresses".into()));
    }
    if addrs.iter().any(|a| !is_public_ip(a.ip())) {
        return Err(LookupError::Forbidden);
    }
    Ok(addrs)
}

/// Why a connection through [`crate::proxy`] may not be made.
#[derive(Debug)]
pub(crate) enum TargetError {
    /// The target is not a public address.
    Forbidden,
    /// It couldn't be resolved.
    Failed(String),
}

/// The addresses to connect to for `host:port`, all public: the address itself if the host
/// is one, else what the name resolves to (every answer must be public). `allow_private`
/// skips the check, for the proxy's tests.
pub(crate) async fn public_targets(
    host: &str,
    port: u16,
    allow_private: bool,
) -> Result<Vec<SocketAddr>, TargetError> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return if allow_private || is_public_ip(ip) {
            Ok(vec![SocketAddr::new(ip, port)])
        } else {
            Err(TargetError::Forbidden)
        };
    }
    let name = host.trim_end_matches('.').to_ascii_lowercase();
    if !allow_private && (name == "localhost" || name.ends_with(".localhost")) {
        return Err(TargetError::Forbidden);
    }
    let resolved = if allow_private {
        tokio::net::lookup_host((name.as_str(), 0))
            .await
            .map(|found| found.collect::<Vec<_>>())
            .map_err(|e| LookupError::Failed(e.to_string()))
    } else {
        lookup_public(&name).await
    };
    match resolved {
        Ok(addrs) => Ok(addrs
            .into_iter()
            .map(|a| SocketAddr::new(a.ip(), port))
            .collect()),
        Err(LookupError::Forbidden) => Err(TargetError::Forbidden),
        Err(LookupError::Failed(why)) => Err(TargetError::Failed(why)),
    }
}

/// The resolver of the client behind [`fetch_public`]: the system's, minus any answer that
/// isn't public. Because `wreq` connects to what this returns, a name can't be checked as one
/// address and then connected to as another.
pub(crate) struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            match lookup_public(name.as_str()).await {
                Ok(addrs) => Ok(Box::new(addrs.into_iter()) as Addrs),
                Err(LookupError::Forbidden) => Err("host resolves to a non-public address".into()),
                Err(LookupError::Failed(e)) => Err(e.into()),
            }
        })
    }
}

/// Whether the server may fetch `url` at all: what [`check_url`] refuses (bar the port), and a
/// name that resolves to a private address. A lookup that fails is not a refusal (the fetch
/// will say what went wrong). For the first link of a server-side fetch, so the cook can be
/// told the link points somewhere private.
pub async fn check_resolved(url: &Url) -> Result<(), Forbidden> {
    check_any_port(url)?;
    if let Some(Host::Domain(name)) = url.host()
        && let Err(LookupError::Forbidden) = lookup_public(name).await
    {
        return forbidden("That host isn't public.");
    }
    Ok(())
}

/// [`check_url`] without the port rule, for a client that follows redirects by itself (each
/// hop is checked with this; the name is caught by its resolver).
pub fn check_target(url: &Url) -> Result<(), Forbidden> {
    check_any_port(url)
}

/// The addresses `host` resolves to, only if every one is public: the resolver for a client
/// that isn't wreq (the image fallback's `reqwest`).
pub async fn resolve_public(host: &str) -> Result<Vec<SocketAddr>, String> {
    lookup_public(host).await.map_err(|e| match e {
        LookupError::Forbidden => "host resolves to a non-public address".to_string(),
        LookupError::Failed(why) => why,
    })
}

/// The redirect policy of the server's clients: at most [`MAX_REDIRECTS`], none to an address
/// that isn't public or to `localhost`. (A name is caught by [`PublicResolver`] when the client
/// connects.)
pub fn server_redirects() -> wreq::redirect::Policy {
    wreq::redirect::Policy::custom(|attempt| {
        if attempt.previous.len() >= MAX_REDIRECTS {
            return attempt.error("too many redirects");
        }
        match Url::parse(&attempt.uri.to_string()).map(|u| check_any_port(&u)) {
            Ok(Ok(())) => attempt.follow(),
            Ok(Err(Forbidden(why))) => attempt.error(why),
            Err(_) => attempt.error("the site redirected somewhere that isn't a valid link"),
        }
    })
}

/// Fetches `url` with `profile`, only from public web addresses: the address, every redirect
/// and every address a name resolves to are checked (see the module docs).
///
/// `Err` means the link (or where it redirected to) is refused; an `Ok(Unreachable)` means
/// a public site that didn't answer.
pub async fn fetch_public(profile: Profile, url: &str) -> Result<Fetched, Forbidden> {
    let Ok(first) = Url::parse(url) else {
        return forbidden("That isn't a valid link.");
    };
    match tokio::time::timeout(TOTAL_TIMEOUT, follow(profile, first)).await {
        Ok(result) => result,
        Err(_) => Ok(Fetched::Unreachable("timed out".into())),
    }
}

async fn follow(profile: Profile, first: Url) -> Result<Fetched, Forbidden> {
    let Some(client) = public_client(profile) else {
        return Ok(Fetched::Unreachable("client unavailable".into()));
    };
    let mut url = first;
    for _ in 0..=MAX_REDIRECTS {
        check_url(&url)?;
        if let Some(Host::Domain(name)) = url.host() {
            match lookup_public(name).await {
                Ok(_) => {}
                Err(LookupError::Forbidden) => return forbidden("That host isn't public."),
                Err(LookupError::Failed(e)) => return Ok(Fetched::Unreachable(e)),
            }
        }
        let res = match client.get(url.as_str()).send().await {
            Ok(res) => res,
            Err(e) => return Ok(Fetched::Unreachable(e.to_string())),
        };
        let status = res.status().as_u16();
        if !matches!(status, 301 | 302 | 303 | 307 | 308) {
            return Ok(page_of(res).await);
        }
        let Some(target) = res
            .headers()
            .get(wreq::header::LOCATION)
            .and_then(|v| v.to_str().ok())
        else {
            // A redirect that says nowhere: the status is all there is
            return Ok(page_of(res).await);
        };
        let Ok(next) = url.join(target.trim()) else {
            return forbidden("The site redirected somewhere that isn't a valid link.");
        };
        url = next;
    }
    Ok(Fetched::Unreachable("too many redirects".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn public(ip: &str) -> bool {
        is_public_ip(ip.parse().unwrap())
    }

    #[test]
    fn public_v4_addresses_pass() {
        for ip in [
            "1.1.1.1",
            "8.8.8.8",
            "93.184.216.34",
            "172.15.255.255",
            "172.32.0.0",
            "100.63.255.255",
            "100.128.0.0",
            "169.253.1.1",
            "198.17.255.255",
            "198.20.0.0",
            "192.0.1.1",
            "192.169.0.1",
            "223.255.255.255",
        ] {
            assert!(public(ip), "{ip} should be public");
        }
    }

    #[test]
    fn private_and_special_v4_addresses_are_refused() {
        for ip in [
            "0.0.0.0",
            "0.1.2.3",
            "10.0.0.1",
            "10.255.255.255",
            "100.64.0.1",
            "100.100.100.100", // Tailscale's MagicDNS
            "100.101.102.103",
            "100.127.255.255",
            "127.0.0.1",
            "127.255.255.254",
            "169.254.0.1",
            "169.254.169.254", // cloud metadata
            "172.16.0.1",
            "172.31.255.255",
            "192.0.0.1",
            "192.0.2.1",
            "192.88.99.1",
            "192.168.0.1",
            "192.168.255.255",
            "198.18.0.1",
            "198.19.255.255",
            "198.51.100.7",
            "203.0.113.7",
            "224.0.0.1",
            "239.255.255.250",
            "240.0.0.1",
            "255.255.255.254",
            "255.255.255.255",
        ] {
            assert!(!public(ip), "{ip} should be refused");
        }
    }

    #[test]
    fn public_v6_addresses_pass() {
        for ip in [
            "2606:4700:4700::1111",
            "2a00:1450:4001:81b::200e",
            "2001:4860:4860::8888",
            "2001:200::1",
            "2620:fe::fe",
            "::ffff:8.8.8.8",
            "64:ff9b::808:808",  // NAT64 to 8.8.8.8
            "2002:0808:0808::1", // 6to4 of 8.8.8.8
        ] {
            assert!(public(ip), "{ip} should be public");
        }
    }

    #[test]
    fn private_and_special_v6_addresses_are_refused() {
        for ip in [
            "::",
            "::1",
            "::2",
            "::127.0.0.1", // IPv4-compatible
            "fc00::1",
            "fd7a:115c:a1e0::1", // Tailscale's own v6 range
            "fdff::1",
            "fe80::1",
            "febf::1",
            "fec0::1",
            "ff02::1",
            "ff00::",
            "100::1",  // discard-only
            "2001::1", // Teredo
            "2001:1::1",
            "2001:db8::1",
            "3fff::1",
            "3fff:fff::1",
            "5f00::1",
            "1234::1", // outside 2000::/3
            // IPv4-mapped, embedded and tunnelled private addresses
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "::ffff:169.254.169.254",
            "::ffff:100.100.100.100",
            "::ffff:192.168.1.1",
            "::ffff:0.0.0.0",
            "::ffff:255.255.255.255",
            "64:ff9b::7f00:1",
            "64:ff9b::a00:1",
            "64:ff9b:1::1",
            "2002:7f00:0001::1",
            "2002:0a00:0001::1",
            "2002:c0a8:0101::1",
        ] {
            assert!(!public(ip), "{ip} should be refused");
        }
    }

    fn check(url: &str) -> Result<(), Forbidden> {
        check_url(&Url::parse(url).unwrap())
    }

    #[test]
    fn urls_to_the_public_web_pass() {
        for url in [
            "http://example.com/recipe",
            "https://example.com/a?b=c#d",
            "https://example.com:443/",
            "http://example.com:80/",
            "https://example.com:80/",
            "http://8.8.8.8/",
            "https://[2606:4700:4700::1111]/",
            "https://EXAMPLE.com./",
        ] {
            assert!(check(url).is_ok(), "{url} should pass");
        }
    }

    #[test]
    fn urls_that_reach_into_the_network_are_refused() {
        for url in [
            "http://127.0.0.1/",
            "http://127.0.0.1:80/",
            "http://localhost/",
            "http://LOCALHOST./",
            "http://app.localhost/",
            "http://169.254.169.254/latest/meta-data/",
            "http://100.100.100.100/",
            "http://100.64.0.1/",
            "http://10.0.0.5/",
            "http://192.168.1.1/",
            "http://172.16.0.1/",
            "http://0.0.0.0/",
            "http://[::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:7f00:1]/",
            "http://[fd7a:115c:a1e0::1]/",
            "http://[fe80::1]/",
            // Ways of writing 127.0.0.1 that a URL parser turns into it
            "http://2130706433/",
            "http://0x7f.1/",
            "http://0177.0.0.1/",
            "http://127.1/",
            // Other schemes, ports and credentials
            "ftp://example.com/",
            "file:///etc/passwd",
            "gopher://example.com/",
            "http://example.com:8080/",
            "https://example.com:22/",
            "http://user@example.com/",
            "http://user:pass@example.com/",
            "http://example.com@127.0.0.1/",
        ] {
            assert!(check(url).is_err(), "{url} should be refused");
        }
    }

    #[test]
    fn a_backslash_does_not_hide_the_host() {
        // The parser reads the backslash as a slash, so the host is the address before it
        assert!(check("http://127.0.0.1\\@example.com/").is_err());
    }

    #[tokio::test]
    async fn fetch_public_refuses_before_connecting() {
        for url in [
            "http://127.0.0.1/",
            "http://169.254.169.254/",
            "http://100.100.100.100/",
            "http://[::1]/",
            "http://localhost/",
            "http://example.com:8080/",
            "ftp://example.com/",
            "not a url",
        ] {
            assert!(
                fetch_public(Profile::Firefox, url).await.is_err(),
                "{url} should be refused"
            );
        }
    }

    #[tokio::test]
    async fn a_name_that_resolves_to_loopback_is_refused() {
        // check_url turns `localhost` away by name; a name only DNS knows is caught when it
        // is resolved, both by the lookup made first and by the client's own resolver
        let refused = lookup_public("localhost").await;
        assert!(
            matches!(refused, Err(LookupError::Forbidden)),
            "{refused:?}"
        );
        assert!(
            PublicResolver
                .resolve(Name::from("localhost"))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn the_servers_own_check_refuses_private_targets_on_any_port() {
        for url in [
            "http://127.0.0.1:3000/api",
            "http://localhost:8080/",
            "http://169.254.169.254/latest/meta-data/",
            "http://10.1.2.3/",
            "http://192.168.0.1:8443/",
            "http://100.100.100.100/",
            "http://[::1]:3000/",
            "http://[fd00::1]/",
            "http://user:pw@example.com/",
            "file:///etc/passwd",
        ] {
            let url = Url::parse(url).unwrap();
            assert!(
                check_resolved(&url).await.is_err(),
                "{url} should be refused"
            );
        }
        // Public addresses pass, on the ports recipe sites really use too
        for url in ["http://93.184.216.34:8080/r", "https://1.1.1.1/"] {
            let url = Url::parse(url).unwrap();
            assert_eq!(check_resolved(&url).await, Ok(()), "{url}");
        }
    }
}
