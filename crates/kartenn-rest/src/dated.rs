//! Dated reads and their cache (spec 006, REQ-006-5, 6; task 006-9): `valid_at` and
//! `as_of` parsed strictly (RFC 3339 with an offset), `as_of` in the future refused, the
//! frozen past `immutable`, an `ETag` and `If-None-Match` answered with 304.
//!
//! The frozen past rests on the `transaction_timeout` of the pool: a transaction begun at
//! `s` ends before `s + T`, so once `as_of ≤ now − T − 1 s` no write can still appear at
//! `as_of` (design « Lectures datées et cache »). `now` is that of the database, read
//! before the data: the data are then at least as recent as `now`.

use std::fmt::Write as _;
use std::time::Duration;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::problem::{Problem, ProblemKind};

/// The clock margin between kartenn and the database, added to the bound of the writes.
pub const CLOCK_MARGIN: Duration = Duration::from_secs(1);

/// `Cache-Control` of a read in the frozen past: a year, never revalidated, and still
/// `private`, since it depends on the rights of the caller (RFC 9111 §5.2.2.7, RFC 8246).
pub const FROZEN: &str = "private, max-age=31536000, immutable";

/// `Cache-Control` of any other read: stored by the client only, revalidated each time
/// (RFC 9111 §5.2.2.4).
pub const LIVE: &str = "private, no-cache";

/// The dates a read asks for; absent, the time of the database.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Dates {
    /// Valid time.
    pub valid_at: Option<OffsetDateTime>,
    /// System time: what kartenn asserted then.
    pub as_of: Option<OffsetDateTime>,
}

/// The dates of a read, as the contract names them.
#[derive(Debug, IntoParams)]
#[into_params(parameter_in = Query)]
#[allow(dead_code)]
pub struct DatesParams {
    /// Valid time, RFC 3339 with its offset; now by default.
    valid_at: Option<String>,
    /// System time, RFC 3339 with its offset: what kartenn asserted then; now by default.
    as_of: Option<String>,
}

fn bad(detail: impl Into<String>) -> Problem {
    Problem::new(ProblemKind::BadRequest).detail(detail)
}

/// An RFC 3339 date with its offset (`Z` or `±hh:mm`, §5.6): a local date, without one,
/// is refused (REQ-006-5).
pub(crate) fn instant(name: &str, text: &str) -> Result<OffsetDateTime, Problem> {
    OffsetDateTime::parse(text, &Rfc3339).map_err(|_| {
        let hint = if text.contains(' ') {
            " (a `+` in a query is written `%2B`)"
        } else {
            ""
        };
        bad(format!(
            "{name} must be an RFC 3339 date with an offset, such as 2026-01-01T00:00:00Z{hint}"
        ))
    })
}

impl Dates {
    /// The dates of a query string; its other parameters are left to the route.
    ///
    /// # Errors
    ///
    /// 400 for a date without offset or malformed, or a date given twice.
    pub fn from_query(query: &str) -> Result<Self, Problem> {
        let mut dates = Self::default();
        for (name, value) in form_urlencoded::parse(query.as_bytes()) {
            let slot = match name.as_ref() {
                "valid_at" => &mut dates.valid_at,
                "as_of" => &mut dates.as_of,
                _ => continue,
            };
            if slot.is_some() {
                return Err(bad(format!("{name} is given more than once")));
            }
            *slot = Some(instant(&name, &value)?);
        }
        Ok(dates)
    }

    /// The moment of the read at `now`, the time of the database, with `bound` the
    /// longest write transaction.
    ///
    /// # Errors
    ///
    /// 400 for an `as_of` later than `now`: kartenn has asserted nothing yet then.
    pub fn at(self, now: OffsetDateTime, bound: Duration) -> Result<Moment, Problem> {
        let frozen = match self.as_of {
            Some(as_of) if as_of > now => {
                return Err(bad("as_of is later than the time of kartenn"));
            }
            Some(as_of) => now - as_of >= bound + CLOCK_MARGIN,
            None => false,
        };
        Ok(Moment {
            valid_at: self.valid_at.unwrap_or(now),
            as_of: self.as_of,
            now,
            frozen,
        })
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Dates {
    type Rejection = Problem;

    fn from_request_parts(
        parts: &mut Parts,
        _: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        std::future::ready(Self::from_query(parts.uri.query().unwrap_or_default()))
    }
}

/// The dates of a read once checked against the time of the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment {
    /// Valid time.
    pub valid_at: OffsetDateTime,
    /// System time; `None` for the current assertions.
    pub as_of: Option<OffsetDateTime>,
    /// The time of the database, read before the data.
    pub now: OffsetDateTime,
    frozen: bool,
}

impl Moment {
    /// Whether no write can still appear at `as_of`: the read is the same forever.
    #[must_use]
    pub const fn frozen(&self) -> bool {
        self.frozen
    }

    /// The system time of the read, for the client to freeze its next reads on.
    #[must_use]
    pub fn as_of(&self) -> OffsetDateTime {
        self.as_of.unwrap_or(self.now)
    }

    /// `Cache-Control` of the response.
    #[must_use]
    pub const fn cache_control(&self) -> &'static str {
        if self.frozen { FROZEN } else { LIVE }
    }
}

/// An entity tag (RFC 9110 §8.8.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ETag {
    weak: bool,
    opaque: String,
}

impl ETag {
    /// The strong tag of an object, `<version>.<digest>` (REQ-006-14, 006/H19): the
    /// changeset that wrote its version, the one a write expects (REQ-004-11), then the
    /// first 64 bits of the SHA-256 of `body`, the bytes served. The digest covers what
    /// the body shows besides the object (summaries of linked objects, inherited values,
    /// embedded collections), so that the tag changes whenever the body does (RFC 9110
    /// §8.8.1); `If-Match` compares the version only ([`ETag::own_version`]).
    #[must_use]
    pub fn record(changeset: Uuid, body: &[u8]) -> Self {
        Self {
            weak: false,
            opaque: format!("{changeset}.{}", hex(&Sha256::digest(body)[..8])),
        }
    }

    /// The version an `If-Match` opaque tag names: `<version>.<digest>` as
    /// [`ETag::record`] makes it, or the bare `<version>` (006/H19); `None` for any
    /// other form.
    #[must_use]
    pub fn own_version(opaque: &str) -> Option<Uuid> {
        let (version, digest) = match opaque.split_once('.') {
            Some((version, digest)) => (version, Some(digest)),
            None => (opaque, None),
        };
        let digest_ok = digest.is_none_or(|d| {
            d.len() == DIGEST_DIGITS && d.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        });
        if digest_ok {
            version.parse().ok()
        } else {
            None
        }
    }

    /// The strong tag of a representation its producer fingerprints, `opaque` made of
    /// hexadecimal digits: the matrix of a flow type, whose bytes the fingerprint fixes and
    /// which a write names in `If-Match` (010/S35).
    #[must_use]
    pub fn fingerprint(opaque: String) -> Self {
        debug_assert!(opaque.bytes().all(|b| b.is_ascii_hexdigit()));
        Self {
            weak: false,
            opaque,
        }
    }

    /// The weak tag of a page: a digest of its content, which an equivalent page with
    /// other bytes would not keep (RFC 9110 §8.8.1).
    #[must_use]
    pub fn page(body: &[u8]) -> Self {
        Self {
            weak: true,
            opaque: hex(&Sha256::digest(body)[..16]),
        }
    }

    /// The header value: `"…"` or `W/"…"`.
    ///
    /// # Panics
    ///
    /// Never: an opaque tag is made of hexadecimal digits, dashes and a dot.
    #[must_use]
    pub fn header_value(&self) -> HeaderValue {
        let prefix = if self.weak { "W/" } else { "" };
        HeaderValue::try_from(format!("{prefix}\"{}\"", self.opaque))
            .expect("an opaque tag of hexadecimal digits, dashes and a dot is a header value")
    }

    /// Whether `If-None-Match` names this tag, by the weak comparison (RFC 9110
    /// §13.1.2), or is `*`. A malformed list names nothing.
    #[must_use]
    pub fn none_match(&self, headers: &HeaderMap) -> bool {
        headers
            .get_all(header::IF_NONE_MATCH)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .any(|list| list.trim() == "*" || tags(list).iter().any(|t| *t == self.opaque))
    }
}

/// The hexadecimal digits of the digest of an object's tag.
const DIGEST_DIGITS: usize = 16;

/// `bytes` in lower-case hexadecimal.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    })
}

/// The opaque tags of an `If-None-Match` list, empty if it is malformed.
fn tags(list: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = list;
    loop {
        rest = rest.trim_start_matches([' ', '\t', ',']);
        if rest.is_empty() {
            return found;
        }
        let quoted = rest.strip_prefix("W/").unwrap_or(rest);
        let Some(inner) = quoted.strip_prefix('"') else {
            return Vec::new();
        };
        let Some(end) = inner.find('"') else {
            return Vec::new();
        };
        found.push(&inner[..end]);
        rest = &inner[end + 1..];
    }
}

/// The response of the read of an object: `body` as JSON, its composite strong `ETag`
/// of `version` and of the bytes served (REQ-006-14, 006/H19), 304 when `If-None-Match`
/// names it.
///
/// # Panics
///
/// Never: a value made of serialisable JSON members serialises.
pub fn respond_object(
    request: &HeaderMap,
    moment: &Moment,
    version: Uuid,
    body: &impl serde::Serialize,
) -> Response {
    let bytes = serde_json::to_vec(body).expect("JSON values serialise");
    let etag = ETag::record(version, &bytes);
    let response = ([(header::CONTENT_TYPE, "application/json")], bytes);
    respond(request, moment, &etag, response)
}

/// The response of a read: 304 without a body when `If-None-Match` names `etag`,
/// `response` otherwise; both carry `Cache-Control` and `ETag` (RFC 9110 §15.4.5).
pub fn respond(
    request: &HeaderMap,
    moment: &Moment,
    etag: &ETag,
    response: impl IntoResponse,
) -> Response {
    let mut response = if etag.none_match(request) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        response.into_response()
    };
    let headers = response.headers_mut();
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(moment.cache_control()),
    );
    headers.insert(header::ETAG, etag.header_value());
    response
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    const T: Duration = Duration::from_secs(60);

    #[test]
    fn a_date_needs_its_offset() {
        let cases = [
            ("as_of=2026-01-01T00:00:00Z", true),
            ("as_of=2026-01-01T00:00:00%2B02:00", true),
            ("as_of=2026-01-01T00:00:00.123456-05:30", true),
            ("as_of=2026-01-01T00:00:00", false),
            ("as_of=2026-01-01", false),
            ("as_of=2026-01-01T00:00:00+02:00", false), // `+` decodes to a space
            ("as_of=", false),
            (
                "valid_at=2026-01-01T00:00:00Z&valid_at=2026-01-02T00:00:00Z",
                false,
            ),
            ("page_size=10", true),
        ];
        for (query, ok) in cases {
            let parsed = Dates::from_query(query);
            assert_eq!(parsed.is_ok(), ok, "{query}");
            if let Err(p) = parsed {
                assert_eq!(p.kind(), ProblemKind::BadRequest);
            }
        }
        let dates = Dates::from_query("valid_at=2026-01-01T02:00:00%2B02:00").unwrap();
        assert_eq!(dates.valid_at, Some(datetime!(2026-01-01 00:00 UTC)));
        assert_eq!(dates.as_of, None);
    }

    #[test]
    fn as_of_cannot_be_in_the_future() {
        let now = datetime!(2026-06-01 12:00 UTC);
        let later = Dates {
            as_of: Some(now + Duration::from_millis(1)),
            ..Dates::default()
        };
        assert_eq!(
            later.at(now, T).unwrap_err().kind(),
            ProblemKind::BadRequest
        );
        let at_now = Dates {
            as_of: Some(now),
            ..Dates::default()
        };
        assert!(!at_now.at(now, T).unwrap().frozen());
    }

    #[test]
    fn only_beyond_the_bound_is_the_past_frozen() {
        let now = datetime!(2026-06-01 12:00 UTC);
        let moment = |ago: Duration| {
            Dates {
                as_of: Some(now - ago),
                ..Dates::default()
            }
            .at(now, T)
            .unwrap()
        };
        assert!(
            !moment(T).frozen(),
            "a write begun at as_of may still commit"
        );
        assert!(!moment(T + Duration::from_millis(999)).frozen());
        assert!(moment(T + CLOCK_MARGIN).frozen());
        assert_eq!(moment(T + CLOCK_MARGIN).cache_control(), FROZEN);
        assert_eq!(moment(T).cache_control(), LIVE);
        let current = Dates::default().at(now, T).unwrap();
        assert!(!current.frozen());
        assert_eq!(current.valid_at, now);
        assert_eq!(current.as_of(), now);
    }

    #[test]
    fn if_none_match_compares_weakly() {
        let version = ETag::record(Uuid::nil(), b"{}");
        let page = ETag::page(b"[]");
        assert_eq!(
            version.header_value(),
            "\"00000000-0000-0000-0000-000000000000.44136fa355b3678a\""
        );
        assert!(page.header_value().to_str().unwrap().starts_with("W/\""));
        let headers = |value: &str| {
            let mut h = HeaderMap::new();
            h.insert(header::IF_NONE_MATCH, value.parse().unwrap());
            h
        };
        let nil = "00000000-0000-0000-0000-000000000000.44136fa355b3678a";
        assert!(version.none_match(&headers(&format!("\"{nil}\""))));
        assert!(version.none_match(&headers(&format!("W/\"{nil}\""))));
        assert!(version.none_match(&headers(&format!("\"a\", W/\"{nil}\""))));
        assert!(version.none_match(&headers("*")));
        assert!(!version.none_match(&headers("\"a\"")));
        assert!(
            !version.none_match(&headers("\"00000000-0000-0000-0000-000000000000\"")),
            "the version alone is not the tag"
        );
        assert!(!version.none_match(&headers(nil)), "unquoted: malformed");
        assert!(!version.none_match(&HeaderMap::new()));
        let weak = page.header_value();
        assert!(page.none_match(&headers(weak.to_str().unwrap())));
        assert_ne!(ETag::page(b"[1]"), page);
        assert_ne!(ETag::record(Uuid::nil(), b"{\"a\":1}"), version);
    }

    #[test]
    fn if_match_names_the_version_of_a_tag() {
        let id = Uuid::now_v7();
        let tag = ETag::record(id, b"{}").opaque;
        assert_eq!(ETag::own_version(&tag), Some(id));
        assert_eq!(ETag::own_version(&id.to_string()), Some(id), "bare form");
        for other in [
            format!("{id}."),
            format!("{id}.44136FA355B3678A"),
            format!("{id}.44136fa355b3678"),
            format!("{id}.44136fa355b3678a.0"),
            format!(".{}", &tag[37..]),
            "abc".to_owned(),
        ] {
            assert_eq!(ETag::own_version(&other), None, "{other}");
        }
    }

    #[test]
    fn a_match_answers_304_with_the_cache_headers() {
        let now = datetime!(2026-06-01 12:00 UTC);
        let moment = Dates::default().at(now, T).unwrap();
        let etag = ETag::record(Uuid::nil(), b"body");
        let mut request = HeaderMap::new();
        let fresh = respond(&request, &moment, &etag, "body");
        assert_eq!(fresh.status(), StatusCode::OK);
        assert_eq!(fresh.headers()[header::CACHE_CONTROL], LIVE);
        request.insert(header::IF_NONE_MATCH, etag.header_value());
        let unchanged = respond(&request, &moment, &etag, "body");
        assert_eq!(unchanged.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(unchanged.headers()[header::ETAG], etag.header_value());
        assert_eq!(unchanged.headers()[header::CACHE_CONTROL], LIVE);
    }
}
