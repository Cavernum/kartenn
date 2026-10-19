//! Pagination by signed cursor (spec 006, REQ-006-7, 8; task 006-10): 100 elements by
//! default, 1 000 at most, no total; the next page in `Link` (`rel="next"`, RFC 8288) and
//! in the body, none at the end of the list.
//!
//! A cursor holds what the client sent and keys it may see — the collection, the dates
//! frozen at the first page, the normalised filters, the page size and the last key read —
//! as compact JSON signed by the sub-key `Purpose::Cursor`: readable, not forgeable. It
//! grants no right: the read scope applies again to each page (design « Curseurs »).

use axum::http::header::{CONTENT_TYPE, LINK};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Response};
use kartenn_auth::keys::{KeyRing, Purpose};
use kartenn_auth::signed;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::dated::{Dates, ETag, Moment, respond};
use crate::problem::{Problem, ProblemKind};

/// Elements of a page when the client does not say.
pub const DEFAULT_SIZE: u16 = 100;
/// Most elements of a page.
pub const MAX_SIZE: u16 = 1_000;

/// Format version of the content of a cursor.
const VERSION: u8 = 1;

/// What a cursor carries.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Content {
    v: u8,
    collection: String,
    valid_at: String,
    as_of: String,
    filters: Vec<(String, String)>,
    size: u16,
    after: Value,
}

fn bad(detail: impl Into<String>) -> Problem {
    Problem::new(ProblemKind::BadRequest).detail(detail)
}

fn invalid() -> Problem {
    Problem::new(ProblemKind::InvalidCursor)
}

fn size(text: &str) -> Result<u16, Problem> {
    text.parse::<u16>()
        .ok()
        .filter(|n| (1..=MAX_SIZE).contains(n))
        .ok_or_else(|| bad(format!("page_size must be an integer from 1 to {MAX_SIZE}")))
}

fn rfc3339(at: OffsetDateTime) -> String {
    at.format(&Rfc3339)
        .expect("a date of the database has a four-digit year")
}

fn instant(text: &str) -> Result<OffsetDateTime, Problem> {
    OffsetDateTime::parse(text, &Rfc3339).map_err(|_| invalid())
}

/// A page asked for: the first one from the parameters of the request, a next one from
/// its cursor.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    /// Elements of the page.
    pub size: u16,
    /// The dates of the read: frozen by the cursor after the first page.
    pub dates: Dates,
    /// The parameters left to the route (its filters), sorted by name.
    pub filters: Vec<(String, String)>,
    after: Option<Value>,
}

impl Page {
    /// The page `query` asks for in `collection`; with a cursor, `page_size` is its only
    /// other parameter.
    ///
    /// # Errors
    ///
    /// 400 `bad-request` for a page size out of 1 to 1 000, a parameter given twice or
    /// beside a cursor, a malformed date; 400 `invalid-cursor` for a cursor altered,
    /// forged, signed by a key the ring no longer holds, or of another collection.
    pub fn from_query(query: &str, ring: &KeyRing, collection: &str) -> Result<Self, Problem> {
        let mut cursor = None;
        let mut page_size = None;
        let mut rest = Vec::new();
        for (name, value) in form_urlencoded::parse(query.as_bytes()) {
            let slot = match name.as_ref() {
                "cursor" => &mut cursor,
                "page_size" => &mut page_size,
                _ => {
                    rest.push((name.into_owned(), value.into_owned()));
                    continue;
                }
            };
            if slot.is_some() {
                return Err(bad(format!("{name} is given more than once")));
            }
            *slot = Some(value.into_owned());
        }
        let size = page_size.as_deref().map(size).transpose()?;
        let Some(cursor) = cursor else {
            let dates = Dates::from_query(query)?;
            rest.retain(|(name, _)| name != "valid_at" && name != "as_of");
            rest.sort();
            return Ok(Self {
                size: size.unwrap_or(DEFAULT_SIZE),
                dates,
                filters: rest,
                after: None,
            });
        };
        if let Some((name, _)) = rest.first() {
            return Err(bad(format!(
                "{name} cannot go with a cursor, which holds the parameters of the list; \
                 only page_size can"
            )));
        }
        let content = signed::verify(ring, Purpose::Cursor, &cursor).map_err(|_| invalid())?;
        let content: Content = serde_json::from_slice(&content).map_err(|_| invalid())?;
        if content.v != VERSION || content.collection != collection {
            return Err(invalid());
        }
        Ok(Self {
            size: size.unwrap_or(content.size),
            dates: Dates {
                valid_at: Some(instant(&content.valid_at)?),
                as_of: Some(instant(&content.as_of)?),
            },
            filters: content.filters,
            after: Some(content.after),
        })
    }

    /// For a list that takes no filter.
    ///
    /// # Errors
    ///
    /// 400 `bad-request` naming the first parameter left to the route.
    pub fn unfiltered(&self) -> Result<(), Problem> {
        match self.filters.first() {
            Some((name, _)) => Err(bad(format!("unknown parameter {name}"))),
            None => Ok(()),
        }
    }

    /// The last key of the previous page, as the route wrote it in [`Page::next`]; `None`
    /// on the first page.
    #[must_use]
    pub const fn after(&self) -> Option<&Value> {
        self.after.as_ref()
    }

    /// The cursor of the page after this one in `collection`, read at `moment`, whose
    /// last key is `after`: the dates of `moment` are frozen in it.
    ///
    /// # Panics
    ///
    /// Never: a cursor is made of strings, integers and a JSON value.
    #[must_use]
    pub fn next(&self, ring: &KeyRing, collection: &str, moment: &Moment, after: Value) -> String {
        let content = Content {
            v: VERSION,
            collection: collection.to_owned(),
            valid_at: rfc3339(moment.valid_at),
            as_of: rfc3339(moment.as_of()),
            filters: self.filters.clone(),
            size: self.size,
            after,
        };
        let json = serde_json::to_vec(&content).expect("a cursor serialises");
        signed::sign(ring, Purpose::Cursor, &json)
    }
}

/// The reference of the next page of the list at `path`.
#[must_use]
pub fn next_href(path: &str, cursor: &str) -> String {
    let cursor: String = form_urlencoded::byte_serialize(cursor.as_bytes()).collect();
    format!("{path}?cursor={cursor}")
}

/// The response of a page: `{"items": […], "links": {"next": …}}`, the next page also in
/// `Link`, a weak `ETag` of the content (REQ-006-6), 304 when `If-None-Match` names it.
///
/// # Panics
///
/// Never: a path and a percent-encoded cursor make a header value.
pub fn respond_page(
    request: &HeaderMap,
    moment: &Moment,
    items: &[Value],
    next: Option<&str>,
) -> Response {
    let link = next.map(|next| {
        HeaderValue::try_from(format!("<{next}>; rel=\"next\""))
            .expect("a path and a percent-encoded cursor make a header value")
    });
    let body = json!({ "items": items, "links": { "next": next } });
    let bytes = serde_json::to_vec(&body).expect("JSON values serialise");
    let etag = ETag::page(&bytes);
    let mut response = ([(CONTENT_TYPE, "application/json")], bytes).into_response();
    if let Some(link) = link {
        response.headers_mut().insert(LINK, link);
    }
    respond(request, moment, &etag, response)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use time::macros::datetime;

    use super::*;

    fn ring(id: &str) -> KeyRing {
        KeyRing::parse(&KeyRing::generate_line(id)).unwrap()
    }

    fn kind(result: Result<Page, Problem>) -> ProblemKind {
        result.unwrap_err().kind()
    }

    #[test]
    fn the_page_size_is_bounded() {
        let ring = ring("k1");
        let size = |query| Page::from_query(query, &ring, "prefixes").map(|p| p.size);
        assert_eq!(size("").unwrap(), DEFAULT_SIZE);
        assert_eq!(size("page_size=1000").unwrap(), 1000);
        for wrong in [
            "page_size=0",
            "page_size=1001",
            "page_size=x",
            "page_size=-1",
        ] {
            assert_eq!(
                size(wrong).unwrap_err().kind(),
                ProblemKind::BadRequest,
                "{wrong}"
            );
        }
    }

    #[test]
    fn a_cursor_freezes_the_list_and_stands_alone() {
        let ring = ring("k1");
        let first = Page::from_query("page_size=2&z=1&a=2", &ring, "prefixes").unwrap();
        assert_eq!(
            first.filters,
            [("a".into(), "2".into()), ("z".into(), "1".into())]
        );
        let now = datetime!(2026-10-18 12:00 UTC);
        let moment = first.dates.at(now, Duration::from_secs(60)).unwrap();
        let cursor = first.next(&ring, "prefixes", &moment, json!(["k", 3]));
        let href = next_href("/v1/prefixes", &cursor);
        let query = href.split_once('?').unwrap().1;

        let next = Page::from_query(query, &ring, "prefixes").unwrap();
        assert_eq!(next.size, 2);
        assert_eq!(next.filters, first.filters);
        assert_eq!(next.after(), Some(&json!(["k", 3])));
        assert_eq!(next.dates.as_of, Some(now));
        assert_eq!(next.dates.valid_at, Some(now));
        let resized = Page::from_query(&format!("{query}&page_size=7"), &ring, "prefixes");
        assert_eq!(resized.unwrap().size, 7);

        for extra in ["as_of=2026-01-01T00:00:00Z", "a=2", "cursor=x"] {
            assert_eq!(
                kind(Page::from_query(
                    &format!("{query}&{extra}"),
                    &ring,
                    "prefixes"
                )),
                ProblemKind::BadRequest,
                "{extra}"
            );
        }
        assert_eq!(
            kind(Page::from_query(query, &ring, "addresses")),
            ProblemKind::InvalidCursor
        );
        assert_eq!(
            kind(Page::from_query(query, &self::ring("k2"), "prefixes")),
            ProblemKind::InvalidCursor
        );
    }
}
