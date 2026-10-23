//! Transport layers (spec 006, design « Pile de couches », layers 3 to 5): the delay
//! (REQ-006-20), the client address and TLS, direct or from the `Forwarded` of a trusted
//! proxy only (RFC 7239, REQ-006-27), the bounded body (413, 415, REQ-006-31) and the URL
//! of 8 KiB at most (414, REQ-006-10).

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use http_body_util::Limited;

use crate::problem::{Problem, ProblemKind};

/// Longest URL (request target), in octets.
pub const MAX_URL: usize = 8 * 1024;

/// A block of addresses whose proxies are believed (`10.0.0.0/8`, `2001:db8::/32`, or one
/// address).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct TrustedProxy {
    network: IpAddr,
    len: u8,
}

/// A block that does not parse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a trusted proxy is an address or a CIDR block: {0}")]
pub struct BadProxy(String);

fn bits(addr: IpAddr) -> (u128, u8) {
    match addr {
        IpAddr::V4(v4) => (u128::from(u32::from(v4)), 32),
        IpAddr::V6(v6) => (u128::from(v6), 128),
    }
}

fn mask(len: u8, width: u8) -> u128 {
    let all = if width == 128 {
        u128::MAX
    } else {
        (1 << width) - 1
    };
    if len == 0 {
        0
    } else {
        all & !((1u128 << (width - len)) - 1)
    }
}

impl TrustedProxy {
    /// Whether `addr` is in the block; an IPv4 address mapped in IPv6 is the IPv4 one.
    #[must_use]
    pub fn contains(self, addr: IpAddr) -> bool {
        let addr = addr.to_canonical();
        if addr.is_ipv4() != self.network.is_ipv4() {
            return false;
        }
        let (network, width) = bits(self.network);
        bits(addr).0 & mask(self.len, width) == network
    }
}

impl FromStr for TrustedProxy {
    type Err = BadProxy;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let bad = || BadProxy(text.to_owned());
        let (addr, len) = match text.split_once('/') {
            Some((addr, len)) => (addr, Some(len)),
            None => (text, None),
        };
        let network: IpAddr = addr.parse().map_err(|_| bad())?;
        let width = bits(network).1;
        let len = match len {
            None => width,
            Some(len) => len
                .parse::<u8>()
                .ok()
                .filter(|l| *l <= width)
                .ok_or_else(bad)?,
        };
        // Host bits set: a typing error more often than an intent.
        if bits(network).0 & !mask(len, width) & mask(width, width) != 0 {
            return Err(bad());
        }
        Ok(Self { network, len })
    }
}

impl TryFrom<String> for TrustedProxy {
    type Error = BadProxy;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl fmt::Display for TrustedProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.len)
    }
}

/// The settings of the transport layers (`[http]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Proxies whose `Forwarded` and `traceparent` are believed.
    pub trusted_proxies: Vec<TrustedProxy>,
    /// Largest request body.
    pub max_body_bytes: usize,
    /// Longest request, until the response headers.
    pub request_timeout: Duration,
    /// Most requests in flight (REQ-006-30).
    pub max_in_flight: usize,
    /// The origin through which the browser reaches kartenn, for the CSRF check of an
    /// `Origin` header (REQ-006-28).
    pub public_origin: Option<String>,
    /// Longest write transaction (`transaction_timeout` of the pool): an `as_of` older
    /// than it is a frozen past (REQ-006-6).
    pub write_transaction_timeout: Duration,
}

impl Default for Settings {
    /// The defaults of the design: no trusted proxy, 16 MiB, 30 s, twice the 10
    /// connections of a default pool in flight, writes of at most 60 s.
    fn default() -> Self {
        Self {
            trusted_proxies: Vec::new(),
            max_body_bytes: 16 * 1024 * 1024,
            request_timeout: Duration::from_secs(30),
            max_in_flight: 20,
            public_origin: None,
            write_transaction_timeout: Duration::from_secs(60),
        }
    }
}

impl Settings {
    /// Whether the peer of the connection is a trusted proxy.
    #[must_use]
    pub fn trusts(&self, peer: IpAddr) -> bool {
        self.trusted_proxies.iter().any(|p| p.contains(peer))
    }
}

/// Set by the server on each request: closes its connection, whatever its requests in
/// flight (an event stream whose client no longer reads, spec 022, REQ-022-21).
#[derive(Debug, Clone, Default)]
pub struct ConnectionKill(pub std::sync::Arc<tokio::sync::Notify>);

impl ConnectionKill {
    /// Closes the connection.
    pub fn kill(&self) {
        self.0.notify_one();
    }
}

/// Set by the server on each request: whether its connection is TLS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionTls(pub bool);

/// The client of a request, after the trusted proxies: its address and whether it
/// reached the first of them over TLS. Without a connection address (a test without
/// server), the unspecified address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client {
    /// The client address.
    pub addr: IpAddr,
    /// Whether the client spoke TLS.
    pub tls: bool,
}

/// One element of `Forwarded`: its `for` (when an address) and its `proto`.
#[derive(Debug, Default, PartialEq, Eq)]
struct Element {
    for_: Option<IpAddr>,
    proto: Option<String>,
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .unwrap_or(value)
}

/// A node of RFC 7239 §6: `192.0.2.1`, `"192.0.2.1:80"`, `"[2001:db8::1]:80"`; an
/// obfuscated or `unknown` node is no address.
fn node(value: &str) -> Option<IpAddr> {
    let value = unquote(value);
    if let Some(rest) = value.strip_prefix('[') {
        return rest.split_once(']')?.0.parse().ok();
    }
    value
        .parse::<IpAddr>()
        .ok()
        .or_else(|| value.parse::<SocketAddr>().ok().map(|s| s.ip()))
}

/// The elements of every `Forwarded` field, first proxy first (RFC 7239 §4).
fn elements(headers: &HeaderMap) -> Vec<Element> {
    headers
        .get_all(header::FORWARDED)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(|element| {
            let mut parsed = Element::default();
            for pair in element.split(';') {
                if let Some((name, value)) = pair.trim().split_once('=') {
                    if name.eq_ignore_ascii_case("for") {
                        parsed.for_ = node(value);
                    } else if name.eq_ignore_ascii_case("proto") {
                        parsed.proto = Some(unquote(value).to_ascii_lowercase());
                    }
                }
            }
            parsed
        })
        .collect()
}

/// The client: from the peer, walk back the `Forwarded` elements while the hop is a
/// trusted proxy. Each element describes the connection its proxy received; a missing
/// `proto` counts as clear, a `for` that is no address stops at the proxy.
#[must_use]
pub fn client(settings: &Settings, peer: IpAddr, tls: bool, headers: &HeaderMap) -> Client {
    let mut client = Client { addr: peer, tls };
    if !settings.trusts(peer) {
        return client;
    }
    for element in elements(headers).into_iter().rev() {
        client.tls = element.proto.as_deref() == Some("https");
        match element.for_ {
            Some(addr) => client.addr = addr,
            None => break,
        }
        if !settings.trusts(client.addr) {
            break;
        }
    }
    client
}

/// The peer of the connection, as set by the server.
pub(crate) fn peer(request: &Request) -> Option<IpAddr> {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(peer)| peer.ip())
}

/// Layer 3: no response headers within the delay → 503 `unavailable`.
pub async fn delay(
    State(settings): State<Arc<Settings>>,
    request: Request,
    next: Next,
) -> Response {
    match tokio::time::timeout(settings.request_timeout, next.run(request)).await {
        Ok(response) => response,
        Err(_) => Problem::new(ProblemKind::Unavailable)
            .detail("the request took too long")
            .into_response(),
    }
}

/// Layer 4: the [`Client`] in the request extensions.
pub async fn forwarded(
    State(settings): State<Arc<Settings>>,
    mut request: Request,
    next: Next,
) -> Response {
    let tls = request
        .extensions()
        .get::<ConnectionTls>()
        .is_some_and(|t| t.0);
    let client = client(
        &settings,
        peer(&request).unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
        tls,
        request.headers(),
    );
    request.extensions_mut().insert(client);
    next.run(request).await
}

/// Layer 5: URL over [`MAX_URL`] → 414 without body; a `Content-Encoding` → 415; a body
/// over the limit → 413, announced by `Content-Length` before reading it, or while
/// reading it.
pub async fn limits(
    State(settings): State<Arc<Settings>>,
    request: Request,
    next: Next,
) -> Response {
    let url = request
        .uri()
        .path_and_query()
        .map_or(0, |p| p.as_str().len());
    if url > MAX_URL {
        return StatusCode::URI_TOO_LONG.into_response();
    }
    if request.headers().contains_key(header::CONTENT_ENCODING) {
        // RFC 9110 §15.5.16: say which codings would have been accepted.
        return (
            [(
                header::ACCEPT_ENCODING,
                HeaderValue::from_static("identity"),
            )],
            Problem::new(ProblemKind::UnsupportedMediaType)
                .detail("a request body is not content-coded"),
        )
            .into_response();
    }
    let max = settings.max_body_bytes;
    let announced = request
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());
    if announced.is_some_and(|n| n > max as u64) {
        return too_large(max);
    }
    let response = next
        .run(request.map(|body| Body::new(Limited::new(body, max))))
        .await;
    // The rejection of the extractors, as a problem.
    if response.status() == StatusCode::PAYLOAD_TOO_LARGE
        && response.extensions().get::<Problem>().is_none()
    {
        return too_large(max);
    }
    response
}

fn too_large(max: usize) -> Response {
    Problem::new(ProblemKind::PayloadTooLarge)
        .detail(format!("a request body has {max} octets at most"))
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(proxies: &[&str]) -> Settings {
        Settings {
            trusted_proxies: proxies.iter().map(|p| p.parse().unwrap()).collect(),
            ..Settings::default()
        }
    }

    fn headers(forwarded: &[&str]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for f in forwarded {
            headers.append(header::FORWARDED, f.parse().unwrap());
        }
        headers
    }

    #[test]
    fn blocks_parse_and_contain() {
        let block: TrustedProxy = "10.0.0.0/8".parse().unwrap();
        assert!(block.contains("10.200.0.1".parse().unwrap()));
        assert!(block.contains("::ffff:10.0.0.1".parse().unwrap()));
        assert!(!block.contains("11.0.0.1".parse().unwrap()));
        assert!(!block.contains("::a00:1".parse().unwrap()));
        let one: TrustedProxy = "2001:db8::1".parse().unwrap();
        assert!(one.contains("2001:db8::1".parse().unwrap()));
        assert!(!one.contains("2001:db8::2".parse().unwrap()));
        let all: TrustedProxy = "0.0.0.0/0".parse().unwrap();
        assert!(all.contains("192.0.2.1".parse().unwrap()));
        for bad in ["10.0.0.1/8", "10.0.0.0/33", "proxy", "10.0.0.0/", "::/129"] {
            assert!(bad.parse::<TrustedProxy>().is_err(), "{bad}");
        }
    }

    #[test]
    fn the_client_follows_the_trusted_proxies_only() {
        let s = settings(&["10.0.0.0/8"]);
        let proxy: IpAddr = "10.0.0.5".parse().unwrap();
        let stranger: IpAddr = "192.0.2.9".parse().unwrap();
        let ip = |t: &str| t.parse::<IpAddr>().unwrap();
        for (peer, tls, forwarded, addr, want_tls) in [
            // An untrusted peer: its `Forwarded` is ignored.
            (
                stranger,
                false,
                vec!["for=198.51.100.1;proto=https"],
                stranger,
                false,
            ),
            (
                stranger,
                true,
                vec!["for=198.51.100.1;proto=http"],
                stranger,
                true,
            ),
            // A trusted proxy: its element.
            (
                proxy,
                false,
                vec!["for=198.51.100.1;proto=https"],
                ip("198.51.100.1"),
                true,
            ),
            (
                proxy,
                true,
                vec!["for=198.51.100.1;proto=http"],
                ip("198.51.100.1"),
                false,
            ),
            (
                proxy,
                false,
                vec!["for=198.51.100.1"],
                ip("198.51.100.1"),
                false,
            ),
            // Two trusted hops, over two fields; the client cannot inject a hop.
            (
                proxy,
                false,
                vec![
                    "for=203.0.113.7;proto=http, for=198.51.100.1;proto=https",
                    "for=10.1.1.1;proto=http",
                ],
                ip("198.51.100.1"),
                true,
            ),
            // Quoted forms, IPv6 with port.
            (
                proxy,
                false,
                vec![r#"for="[2001:db8::7]:4711";proto="HTTPS""#],
                ip("2001:db8::7"),
                true,
            ),
            (
                proxy,
                false,
                vec![r#"For="198.51.100.1:80""#],
                ip("198.51.100.1"),
                false,
            ),
            // No address: stop at the proxy.
            (proxy, false, vec!["for=unknown;proto=https"], proxy, true),
            (proxy, false, vec!["for=_hidden"], proxy, false),
            (proxy, true, vec![], proxy, true),
        ] {
            let c = client(&s, peer, tls, &headers(&forwarded));
            assert_eq!(
                c,
                Client {
                    addr,
                    tls: want_tls
                },
                "{peer} {forwarded:?}"
            );
        }
    }
}
