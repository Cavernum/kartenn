//! The CSRF check (spec 006, REQ-006-28; layer 8; design « Authentification »).
//!
//! A method that is not safe (`GET`, `HEAD`, `OPTIONS`, `QUERY` are), authenticated by a
//! session cookie, passes only with `Sec-Fetch-Site: same-origin` (Fetch Metadata §2.3)
//! or, without that header, with an `Origin` equal to the public origin; otherwise 403
//! `cross-site-request`. `Bearer` and `Basic` are not concerned: a browser does not add
//! them on its own. No CORS: the BFF is of the same origin.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use kartenn_service::authn::{Method, Principal};

use crate::problem::{Problem, ProblemKind};
use crate::quota;
use crate::transport::Settings;

const SEC_FETCH_SITE: HeaderName = HeaderName::from_static("sec-fetch-site");

/// Whether the headers of a request show that it comes from the public origin:
/// `Sec-Fetch-Site` first, `Origin` only in its absence. Without a configured origin, only
/// `Sec-Fetch-Site` can pass.
#[must_use]
pub fn same_origin(headers: &HeaderMap, public_origin: Option<&str>) -> bool {
    if let Some(site) = headers.get(SEC_FETCH_SITE) {
        return site.as_bytes() == b"same-origin";
    }
    match (headers.get(header::ORIGIN), public_origin) {
        (Some(origin), Some(public)) => origin
            .to_str()
            .is_ok_and(|origin| origin.eq_ignore_ascii_case(public)),
        _ => false,
    }
}

/// Layer 8: refuses an unsafe method of a cookie session from another origin.
pub async fn layer(
    State(settings): State<Arc<Settings>>,
    request: Request,
    next: Next,
) -> Response {
    let by_cookie = request
        .extensions()
        .get::<Principal>()
        .is_some_and(|p| matches!(p.method, Method::OidcSession | Method::Breakglass));
    if by_cookie
        && !quota::reads(request.method())
        && !same_origin(request.headers(), settings.public_origin.as_deref())
    {
        return Problem::new(ProblemKind::CrossSiteRequest).into_response();
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    const PUBLIC: &str = "https://kartenn.example";

    fn headers(site: Option<&'static str>, origin: Option<&'static str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(site) = site {
            headers.insert(SEC_FETCH_SITE, HeaderValue::from_static(site));
        }
        if let Some(origin) = origin {
            headers.insert(header::ORIGIN, HeaderValue::from_static(origin));
        }
        headers
    }

    /// REQ-006-28: only `same-origin`, and an equal `Origin` without `Sec-Fetch-Site`,
    /// pass.
    #[test]
    fn matrix() {
        for site in [
            Some("same-origin"),
            Some("same-site"),
            Some("cross-site"),
            Some("none"),
            None,
        ] {
            for origin in [Some(PUBLIC), Some("https://other.example"), None] {
                let expected =
                    site == Some("same-origin") || (site.is_none() && origin == Some(PUBLIC));
                assert_eq!(
                    same_origin(&headers(site, origin), Some(PUBLIC)),
                    expected,
                    "{site:?} × {origin:?}"
                );
            }
        }
    }

    #[test]
    fn origin_compared_whole() {
        assert!(same_origin(
            &headers(None, Some("HTTPS://Kartenn.Example")),
            Some(PUBLIC)
        ));
        for other in [
            "null",
            "https://kartenn.example:8443",
            "http://kartenn.example",
        ] {
            assert!(
                !same_origin(&headers(None, Some(other)), Some(PUBLIC)),
                "{other}"
            );
        }
        assert!(!same_origin(&headers(None, Some(PUBLIC)), None));
    }
}
