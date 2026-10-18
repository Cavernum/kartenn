//! Cookies (REQ-006-25): `__Host-`, `Secure`, `HttpOnly`, `SameSite=Strict`, `Path=/`,
//! no `Domain` (RFC 6265bis §4.1.3.2); a sealed value over one cookie `name`, or over
//! `name-1`… when it spans several; removed by `Max-Age=0`. Only ever set over TLS: the
//! callers refuse a request in clear before (REQ-006-27).

use axum::http::{HeaderMap, HeaderValue, header};
use kartenn_auth::seal::CHUNKS;

/// The session is sent again when its last activity is older than this, in seconds: at
/// most once a minute (REQ-006-25).
pub const REISSUE_AFTER: i64 = 60;

/// The attributes of every cookie of kartenn.
const ATTRIBUTES: &str = "Path=/; Secure; HttpOnly; SameSite=Strict";

/// A response that ended the session: the session cookie is not sent again after it.
#[derive(Debug, Clone, Copy)]
pub struct Ended;

/// The names a value of `name` may take: `name` alone, then `name-1`….
fn names(name: &str) -> impl Iterator<Item = String> + '_ {
    std::iter::once(name.to_owned()).chain((1..=CHUNKS).map(move |n| format!("{name}-{n}")))
}

fn value(text: &str) -> HeaderValue {
    HeaderValue::from_str(text).expect("a sealed value is base64url, a name is ASCII")
}

/// The `Set-Cookie` values that set `chunks` under `name`, for `max_age` seconds or the
/// browser session, and remove the other forms of `name`.
#[must_use]
pub fn set(name: &str, chunks: &[String], max_age: Option<i64>) -> Vec<HeaderValue> {
    let age = max_age.map_or_else(String::new, |s| format!("; Max-Age={s}"));
    let set: Vec<(String, &str)> = if chunks.len() == 1 {
        vec![(name.to_owned(), chunks[0].as_str())]
    } else {
        chunks
            .iter()
            .enumerate()
            .map(|(i, chunk)| (format!("{name}-{}", i + 1), chunk.as_str()))
            .collect()
    };
    let mut values: Vec<HeaderValue> = set
        .iter()
        .map(|(n, v)| value(&format!("{n}={v}; {ATTRIBUTES}{age}")))
        .collect();
    values.extend(
        names(name)
            .filter(|n| set.iter().all(|(used, _)| used != n))
            .map(|n| value(&format!("{n}=; {ATTRIBUTES}; Max-Age=0"))),
    );
    values
}

/// The `Set-Cookie` values that remove every form of `name`.
#[must_use]
pub fn remove(name: &str) -> Vec<HeaderValue> {
    set(name, &[], None)
}

/// Appends `values` to the `Set-Cookie` of `headers`.
pub fn append(headers: &mut HeaderMap, values: Vec<HeaderValue>) {
    for value in values {
        headers.append(header::SET_COOKIE, value);
    }
}

/// The chunks of the cookie `name`: `name` alone, or `name-1`… in order; `None` without
/// any.
#[must_use]
pub fn read(headers: &HeaderMap, name: &str) -> Option<Vec<String>> {
    let mut whole = None;
    let mut chunks: Vec<(u8, String)> = Vec::new();
    let pairs = headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='));
    for (found, value) in pairs {
        if found == name {
            whole = Some(value.to_owned());
        } else if let Some(n) = found
            .strip_prefix(name)
            .and_then(|rest| rest.strip_prefix('-'))
            .and_then(|n| n.parse::<u8>().ok())
        {
            chunks.push((n, value.to_owned()));
        }
    }
    if let Some(whole) = whole {
        return Some(vec![whole]);
    }
    if chunks.is_empty() {
        return None;
    }
    chunks.sort_by_key(|(n, _)| *n);
    Some(chunks.into_iter().map(|(_, value)| value).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[&str]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for value in pairs {
            map.append(header::COOKIE, HeaderValue::from_str(value).unwrap());
        }
        map
    }

    #[test]
    fn a_cookie_is_read_whole_or_by_chunks() {
        let name = "__Host-kartenn";
        assert_eq!(
            read(&headers(&["a=1; __Host-kartenn=xyz"]), name),
            Some(vec!["xyz".into()])
        );
        assert_eq!(
            read(
                &headers(&["__Host-kartenn-2=b", "x=y; __Host-kartenn-1=a"]),
                name
            ),
            Some(vec!["a".into(), "b".into()])
        );
        assert_eq!(
            read(&headers(&["__Host-kartenn-login=l"]), name),
            None,
            "the login cookie is no session"
        );
        assert_eq!(
            read(
                &headers(&["__Host-kartenn-login=l"]),
                "__Host-kartenn-login"
            ),
            Some(vec!["l".into()])
        );
    }

    #[test]
    fn a_cookie_carries_its_attributes_and_removes_its_other_forms() {
        let set = set("__Host-k", &["v".into()], Some(600));
        let set: Vec<&str> = set.iter().map(|v| v.to_str().unwrap()).collect();
        assert_eq!(
            set[0],
            "__Host-k=v; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=600"
        );
        assert!(!set[0].contains("Domain"));
        assert_eq!(set.len(), 1 + CHUNKS);
        assert!(set[1..].iter().all(|v| v.ends_with("Max-Age=0")));
        let split = super::set("__Host-k", &["a".into(), "b".into()], None);
        let split: Vec<&str> = split.iter().map(|v| v.to_str().unwrap()).collect();
        assert!(split[0].starts_with("__Host-k-1=a;") && split[1].starts_with("__Host-k-2=b;"));
        assert!(split.contains(&"__Host-k=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0"));
        assert!(
            remove("__Host-k")
                .iter()
                .all(|v| v.to_str().unwrap().ends_with("Max-Age=0"))
        );
    }
}
