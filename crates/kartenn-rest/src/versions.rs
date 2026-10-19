//! The versions of the API (spec 006, REQ-006-33; task 006-22; H12): `/v1` only grows. A
//! breaking change opens `/v2`, served beside `/v1` for a notice during which each route
//! of `/v1` answers with `Deprecation` (RFC 9745 §2), `Sunset` (RFC 8594 §3) and a link
//! `rel="successor-version"` (RFC 5829 §3.4) to its successor, and is marked deprecated
//! in the contract.

use std::sync::Arc;

use axum::http::header::LINK;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::map_response;
use axum::response::Response;
use time::{OffsetDateTime, UtcOffset};
use utoipa::openapi::Deprecated;
use utoipa_axum::router::UtoipaMethodRouter;

/// The header of a deprecation (RFC 9745 §2).
pub static DEPRECATION: HeaderName = HeaderName::from_static("deprecation");
/// The header of a sunset (RFC 8594 §3).
pub static SUNSET: HeaderName = HeaderName::from_static("sunset");

/// The retirement of a route: deprecated since, removed at, replaced by.
#[derive(Debug, Clone)]
pub struct Retirement {
    /// When the route was deprecated.
    pub deprecated: OffsetDateTime,
    /// When it will be removed: the end of the notice.
    pub sunset: OffsetDateTime,
    /// The URL of its successor in the next version.
    pub successor: String,
}

impl Retirement {
    /// `Deprecation`: a structured-field date, `@` and the seconds since the epoch
    /// (RFC 9745 §2.1, RFC 9651 §3.3.7).
    #[must_use]
    pub fn deprecation(&self) -> String {
        format!("@{}", self.deprecated.unix_timestamp())
    }

    /// `Sunset`: an HTTP-date, `IMF-fixdate` (RFC 8594 §3, RFC 9110 §5.6.7).
    #[must_use]
    pub fn sunset(&self) -> String {
        const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let at = self.sunset.to_offset(UtcOffset::UTC);
        format!(
            "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
            DAYS[usize::from(at.weekday().number_days_from_monday())],
            at.day(),
            MONTHS[usize::from(u8::from(at.month())) - 1],
            at.year(),
            at.hour(),
            at.minute(),
            at.second(),
        )
    }

    /// `Link` to the successor (RFC 8288, RFC 5829 §3.4).
    #[must_use]
    pub fn link(&self) -> String {
        format!("<{}>; rel=\"successor-version\"", self.successor)
    }

    fn announce(&self, response: &mut Response) {
        let headers = response.headers_mut();
        for (name, value) in [
            (&DEPRECATION, self.deprecation()),
            (&SUNSET, self.sunset()),
            (&LINK, self.link()),
        ] {
            if let Ok(value) = HeaderValue::try_from(value) {
                headers.append(name.clone(), value);
            }
        }
    }
}

/// The route `route` retired as `retirement` says: every answer announces it, and its
/// operations are deprecated in the contract.
pub fn retire<S>(
    (schemas, mut paths, router): UtoipaMethodRouter<S>,
    retirement: Retirement,
) -> UtoipaMethodRouter<S>
where
    S: Clone + Send + Sync + 'static,
{
    for item in paths.paths.values_mut() {
        for operation in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
        ]
        .into_iter()
        .flatten()
        {
            operation.deprecated = Some(Deprecated::True);
        }
    }
    let retirement = Arc::new(retirement);
    let router = router.layer(map_response(move |mut response: Response| {
        let retirement = retirement.clone();
        async move {
            retirement.announce(&mut response);
            response
        }
    }));
    (schemas, paths, router)
}
