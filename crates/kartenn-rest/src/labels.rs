//! The dictionary of labels `GET /v1/labels` (spec 018, R19 to R24, REQ-018-14 to 17,
//! 19; ADR 0040): for one language, the labels of everything that has one, by space —
//! the labels stored with their owning objects (values of closed lists, declarations,
//! roles, objects of the modules) at the dates of the read, and the labels compiled with
//! the codes of the build (permissions, rules, problems, kinds, states…), whose
//! fingerprint is computed once per build. The only route whose body depends on the
//! language: `lang` or `Accept-Language` (Lookup, RFC 4647 §3.4), `Content-Language`,
//! `Vary: Accept-Language`, a strong `ETag` `"<revision>.<digest>"`, 304.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::Extension;
use axum::extract::{RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use kartenn_core::authz::grants;
use kartenn_core::labels::{CodeLabel, Labels, Locale, default_locale};
use kartenn_service::authn::Principal;
use kartenn_service::module::ModuleRegistry;
use kartenn_store_pg::{labels_read, read};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::cursor::rfc3339;
use crate::dated::{Dates, DatesParams, ETag, LIVE, Moment};
use crate::objects::{Api, store};
use crate::problem::{Problem, ProblemKind};

/// `Cache-Control` of a read at an `as_of` in the past: the stored labels no longer
/// change there, the compiled ones only with the version of the server (R22).
pub const PAST: &str = "private, max-age=86400";

/// How long an instance keeps a dictionary read at present before reading it again,
/// even at the same revision (a value whose validity starts later appears without a
/// write).
const KEPT: Duration = Duration::from_secs(60);

/// The most dictionaries an instance keeps.
const KEPT_MAX: usize = 256;

/// The spaces of the codes of the software, served to any authenticated caller (R24).
const CODE_SPACES: [&str; 7] = [
    "permission",
    "rule",
    "problem",
    "kind",
    "state",
    "nature",
    "usage",
];

/// The labels compiled with the build, sorted by space and key, and their fingerprint.
#[derive(Debug, Clone)]
pub struct CodeCatalogue {
    labels: Vec<CodeLabel>,
    digest: String,
}

impl CodeCatalogue {
    /// The labels of the core, of the modules of `registry`, of the problems, and of the
    /// former names of the renamed permissions (R6: the history keeps them).
    #[must_use]
    pub fn of(registry: &ModuleRegistry) -> Self {
        let mut labels = kartenn_core::code_labels::core();
        labels.extend(registry.labels());
        labels.extend(ProblemKind::ALL.iter().map(|kind| {
            let compiled = kind.labels();
            CodeLabel::new("problem", kind.slug(), compiled.fr, compiled.en)
        }));
        // Spec 024, REQ-024-9: a retired name is labelled after the first name replacing it.
        let catalogue: Vec<&kartenn_core::authz::Permission> = kartenn_core::authz::PERMISSIONS
            .iter()
            .copied()
            .chain(registry.permissions())
            .collect();
        let former: Vec<CodeLabel> = kartenn_core::authz::retired_names(&catalogue)
            .into_iter()
            .filter_map(|(old, new)| {
                let current = labels
                    .iter()
                    .find(|l| l.space == "permission" && new.first() == Some(&l.key))?;
                let new = new.join(", ");
                Some(CodeLabel {
                    space: "permission",
                    key: old,
                    fr: format!("{} (ancien nom de {new})", current.fr),
                    en: format!("{} (former name of {new})", current.en),
                })
            })
            .collect();
        labels.extend(former);
        labels.sort();
        labels.dedup_by(|a, b| a.space == b.space && a.key == b.key);
        let mut hasher = Sha256::new();
        for l in &labels {
            for part in [l.space, &l.key, &l.fr, &l.en] {
                hasher.update(part.as_bytes());
                hasher.update([0]);
            }
        }
        let digest = crate::dated::hex(&hasher.finalize()[..8]);
        Self { labels, digest }
    }

    /// The labels, sorted by space and key.
    #[must_use]
    pub fn labels(&self) -> &[CodeLabel] {
        &self.labels
    }

    /// The fingerprint of the labels (16 hexadecimal characters), part of the revision of
    /// the dictionary.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// The catalogue of the build of `registry`, computed once per set of modules.
pub(crate) fn catalogue(registry: &ModuleRegistry) -> Arc<CodeCatalogue> {
    static BUILT: OnceLock<Mutex<HashMap<Vec<String>, Arc<CodeCatalogue>>>> = OnceLock::new();
    let modules: Vec<String> = registry
        .modules()
        .map(|m| format!("{}@{}", m.name(), m.version()))
        .collect();
    let mut built = BUILT
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    built
        .entry(modules)
        .or_insert_with(|| Arc::new(CodeCatalogue::of(registry)))
        .clone()
}

/// The routes of the dictionary, under `/v1`.
pub(crate) fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new().routes(routes!(labels))
}

/// The parameters of the dictionary besides the dates.
#[derive(Debug, IntoParams)]
#[into_params(parameter_in = Query)]
#[allow(dead_code)]
struct LabelsParams {
    /// The language wanted, a BCP 47 tag; it prevails over `Accept-Language`.
    lang: Option<String>,
    /// The spaces wanted, separated by commas (`value,role`); all by default.
    spaces: Option<String>,
}

/// The entries the dictionary served in the default language, lacking one in the
/// language served.
#[derive(Debug, Serialize, ToSchema)]
struct Fallback {
    /// The default language of the instance.
    lang: String,
    /// `<space>/<key>`, or `value/<property>/<value>`.
    keys: Vec<String>,
}

/// The dictionary of labels for one language.
#[derive(Debug, Serialize, ToSchema)]
#[schema(as = Labels)]
struct LabelsBody {
    /// The language served (BCP 47, lower case), also in `Content-Language`.
    lang: String,
    /// The default language of the instance.
    default_lang: String,
    /// The revision of the labels: `<stored>-<compiled>`.
    revision: String,
    /// The valid time asked for, when one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    valid_at: Option<String>,
    /// The system time asked for, when one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    as_of: Option<String>,
    /// The labels by space then key; `value` by property, then value.
    #[schema(value_type = Object)]
    spaces: BTreeMap<String, Value>,
    /// The entries served in the default language.
    #[serde(skip_serializing_if = "Option::is_none")]
    fallback: Option<Fallback>,
}

fn bad(detail: impl Into<String>) -> Problem {
    Problem::new(ProblemKind::BadRequest).detail(detail)
}

/// `lang` and `spaces` of a query string.
fn params(query: &str) -> Result<(Option<Locale>, Option<BTreeSet<String>>), Problem> {
    let mut lang = None;
    let mut spaces = None;
    for (name, value) in form_urlencoded::parse(query.as_bytes()) {
        match name.as_ref() {
            "lang" => {
                if lang.is_some() {
                    return Err(bad("lang is given more than once"));
                }
                lang = Some(Locale::parse(&value).ok_or_else(|| {
                    bad(format!("lang {value:?} is not a language tag (BCP 47)"))
                })?);
            }
            "spaces" => {
                spaces = Some(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .collect(),
                );
            }
            _ => {}
        }
    }
    Ok((lang, spaces))
}

/// A weight `qvalue` in thousandths (RFC 9110 §12.4.2: `0[.ddd]` or `1[.000]`).
fn weight(q: &str) -> Option<u16> {
    let q = q.trim();
    let (whole, decimals) = q.split_once('.').unwrap_or((q, ""));
    if decimals.len() > 3 || !decimals.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let thousandths: u16 = format!("{decimals:0<3}").parse().ok()?;
    match whole {
        "0" => Some(thousandths),
        "1" if thousandths == 0 => Some(1000),
        _ => None,
    }
}

/// The language ranges of `Accept-Language`, by decreasing weight (RFC 9110 §12.5.4,
/// §12.4.2): `q=0` and `*` dropped, a malformed range ignored.
fn ranges(accept: &str) -> Vec<Locale> {
    let mut weighted: Vec<(u16, usize, Locale)> = accept
        .split(',')
        .enumerate()
        .filter_map(|(i, item)| {
            let mut parts = item.split(';');
            let tag = parts.next()?.trim();
            let q = parts
                .find_map(|p| p.trim().strip_prefix("q="))
                .map_or(Some(1000), weight)?;
            (q > 0 && tag != "*")
                .then(|| Locale::parse(tag))
                .flatten()
                .map(|l| (q, i, l))
        })
        .collect();
    weighted.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    weighted.into_iter().map(|(_, _, l)| l).collect()
}

/// The language served: the first range whose Lookup (RFC 4647 §3.4) meets an available
/// language, else the default language (R9).
fn negotiate(wanted: &[Locale], available: &BTreeSet<Locale>, default: &Locale) -> Locale {
    wanted
        .iter()
        .flat_map(Locale::truncations)
        .find(|l| available.contains(l))
        .unwrap_or_else(|| default.clone())
}

/// What the caller reads, on any scope (R24).
struct Rights {
    entries: Vec<String>,
    roles: Vec<String>,
    /// The groups of kartenn of the caller (spec 025).
    groups: Vec<String>,
}

impl Rights {
    fn holds(&self, permission: &str, sensitive: bool) -> bool {
        self.entries
            .iter()
            .any(|e| grants(e, permission, sensitive))
    }

    /// The key of the in-memory dictionary for this caller.
    fn key(&self) -> String {
        format!(
            "{}|{}|{}",
            self.entries.join(","),
            self.roles.join(","),
            self.groups.join(",")
        )
    }
}

/// The dictionaries an instance keeps, by request at present.
type Kept = HashMap<String, (Instant, Arc<(Vec<u8>, String)>)>;

fn kept() -> &'static Mutex<Kept> {
    static KEPT_ONES: OnceLock<Mutex<Kept>> = OnceLock::new();
    KEPT_ONES.get_or_init(Mutex::default)
}

/// `GET /v1/labels`: the dictionary of labels for one language (REQ-018-14 to 17, 19).
#[utoipa::path(
    get,
    path = "/labels",
    operation_id = "labels",
    tag = "operations",
    params(LabelsParams, DatesParams),
    responses(
        (status = 200, description = "The labels by space, in the language served (`Content-Language`).", body = LabelsBody),
        (status = 304, description = "Unchanged since the `ETag` of `If-None-Match`."),
        (status = 400, description = "A malformed `lang` or date.", body = Problem),
    )
)]
async fn labels(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let query = query.unwrap_or_default();
    let dates = Dates::from_query(&query)?;
    let (lang, spaces) = params(&query)?;
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = dates.at(now, api.bound)?;
    let codes = catalogue(&api.modules);
    let stored_revision = labels_read::revision(&api.pool)
        .await
        .map_err(|e| store(&e))?;
    let revision = format!("{stored_revision}-{}", codes.digest());
    let subject = principal.subject();
    let (entries, roles) = labels_read::held(&api.pool, &subject)
        .await
        .map_err(|e| store(&e))?;
    let groups = labels_read::groups_held(&api.pool, &subject)
        .await
        .map_err(|e| store(&e))?;
    let rights = Rights {
        entries,
        roles,
        groups,
    };
    let accept = headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let present = dates.valid_at.is_none() && dates.as_of.is_none();
    let kept_key = format!(
        "{}|{revision}|{}|{accept}|{:?}|{}",
        api.pool
            .connect_options()
            .get_database()
            .unwrap_or_default(),
        lang.as_ref().map_or("", Locale::as_str),
        spaces,
        rights.key()
    );
    let found = if present {
        kept()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&kept_key)
            .filter(|(at, _)| at.elapsed() < KEPT)
            .map(|(_, found)| found.clone())
    } else {
        None
    };
    let found = if let Some(found) = found {
        found
    } else {
        let built = Arc::new(
            build(
                &api,
                &moment,
                &codes,
                (&rights, &revision),
                (lang, spaces, &accept),
            )
            .await?,
        );
        if present {
            let mut kept = kept()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if kept.len() >= KEPT_MAX {
                kept.clear();
            }
            kept.insert(kept_key, (Instant::now(), built.clone()));
        }
        built
    };
    let (body, served) = &*found;
    let etag = ETag::labels(&revision, body);
    let mut response = if etag.none_match(&headers) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        ([(header::CONTENT_TYPE, "application/json")], body.clone()).into_response()
    };
    let out = response.headers_mut();
    out.insert(header::ETAG, etag.header_value());
    out.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if moment.as_of.is_some() && moment.frozen() {
            PAST
        } else {
            LIVE
        }),
    );
    out.insert(header::VARY, HeaderValue::from_static("Accept-Language"));
    if let Ok(language) = HeaderValue::from_str(served) {
        out.insert(header::CONTENT_LANGUAGE, language);
    }
    Ok(response)
}

/// The body of the dictionary and the language served.
#[allow(clippy::too_many_lines)] // one pass over the stored, then the compiled labels
async fn build(
    api: &Api,
    moment: &Moment,
    codes: &CodeCatalogue,
    (rights, revision): (&Rights, &str),
    (lang, wanted_spaces, accept): (Option<Locale>, Option<BTreeSet<String>>, &str),
) -> Result<(Vec<u8>, String), Problem> {
    let wanted = |space: &str| wanted_spaces.as_ref().is_none_or(|w| w.contains(space));
    let tables: Vec<(&str, &str)> = api
        .modules
        .label_tables()
        .filter(|t| wanted(t.space) && rights.holds(t.permission, false))
        .map(|t| (t.space, t.table))
        .collect();
    let rbac_reader = rights.holds("core.rbac.read", false);
    let stored: Vec<(labels_read::Stored, Labels)> =
        labels_read::stored(&api.pool, moment.valid_at, moment.as_of, &tables)
            .await
            .map_err(|e| store(&e))?
            .into_iter()
            .filter(|s| wanted(&s.space))
            .filter(|s| match s.space.as_str() {
                // R24: a value of a sensitive property needs its own read permission.
                "value" => {
                    !s.sensitive
                        || s.property
                            .as_ref()
                            .is_some_and(|p| rights.holds(&format!("prop.{p}.read"), true))
                }
                "role" => rbac_reader || rights.roles.contains(&s.key),
                // Spec 025: the groups of the caller, all of them for a reader of the rights.
                "group" => rbac_reader || rights.groups.contains(&s.key),
                _ => true,
            })
            .map(|s| {
                let labels = Labels::stored(&s.labels);
                (s, labels)
            })
            .collect();
    let default = default_locale();
    let mut available: BTreeSet<Locale> =
        ["fr", "en"].into_iter().filter_map(Locale::parse).collect();
    available.insert(default.clone());
    for (_, labels) in &stored {
        available.extend(labels.iter().map(|(l, _)| l.clone()));
    }
    let ranges = lang.map_or_else(|| ranges(accept), |l| vec![l]);
    let served = negotiate(&ranges, &available, default);
    let mut spaces: BTreeMap<String, Value> = BTreeMap::new();
    let mut fallback = Vec::new();
    let mut put = |space: &str, path: Vec<&str>, text: String, fell: bool, key: String| {
        let mut node = spaces
            .entry(space.to_owned())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        let (last, parents) = path.split_last().expect("a key");
        for p in parents {
            node = node
                .as_object_mut()
                .expect("an object")
                .entry((*p).to_owned())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
        }
        node.as_object_mut()
            .expect("an object")
            .insert((*last).to_owned(), Value::String(text));
        if fell {
            fallback.push(key);
        }
    };
    for (s, labels) in &stored {
        let (text, fell) = match labels.get(&served) {
            Some(text) => (text, false),
            None => match labels.get(default) {
                Some(text) => (text, true),
                None => continue,
            },
        };
        let path: Vec<&str> = s
            .property
            .as_deref()
            .into_iter()
            .chain([s.key.as_str()])
            .collect();
        let key = format!("{}/{}", s.space, path.join("/"));
        put(&s.space, path, text.to_owned(), fell, key);
    }
    for code in codes.labels() {
        if !CODE_SPACES.contains(&code.space) || !wanted(code.space) {
            continue;
        }
        let (text, fell) = match code.get(&served) {
            Some(text) => (text, false),
            None => match code.get(default) {
                Some(text) => (text, true),
                None => (code.en.as_str(), true),
            },
        };
        let key = format!("{}/{}", code.space, code.key);
        put(
            code.space,
            vec![code.key.as_str()],
            text.to_owned(),
            fell,
            key,
        );
    }
    fallback.sort();
    let body = LabelsBody {
        lang: served.as_str().to_owned(),
        default_lang: default.as_str().to_owned(),
        revision: revision.to_owned(),
        valid_at: moment
            .valid_at
            .ne(&moment.now)
            .then(|| rfc3339(moment.valid_at)),
        as_of: moment.as_of.map(rfc3339),
        spaces,
        fallback: (!fallback.is_empty()).then(|| Fallback {
            lang: default.as_str().to_owned(),
            keys: fallback,
        }),
    };
    let bytes = serde_json::to_vec(&body).expect("JSON values serialise");
    Ok((bytes, served.as_str().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_looked_up_then_defaulted() {
        let available: BTreeSet<Locale> = ["fr", "en", "de-ch"]
            .into_iter()
            .filter_map(Locale::parse)
            .collect();
        let fr = Locale::parse("fr").unwrap();
        let served = |accept: &str| negotiate(&ranges(accept), &available, &fr).to_string();
        assert_eq!(served("fr-CA, en;q=0.5"), "fr");
        assert_eq!(served("en;q=0.4, de-CH-1996;q=0.9"), "de-ch");
        assert_eq!(served("it, *"), "fr");
        assert_eq!(served("en;q=0, de;q=0.1"), "fr");
        assert_eq!(served("en-GB;q=x, en-US"), "en");
        assert_eq!(served(""), "fr");
        assert!(params("lang=fr_FR").is_err());
        assert_eq!(params("lang=EN-gb").unwrap().0.unwrap().as_str(), "en-gb");
    }

    #[test]
    fn the_problems_and_former_names_are_labelled() {
        let catalogue = CodeCatalogue::of(ModuleRegistry::empty());
        let find = |space: &str, key: &str| {
            catalogue
                .labels()
                .iter()
                .find(|l| l.space == space && l.key == key)
                .cloned()
        };
        assert_eq!(
            find("problem", "changeset-refused").map(|l| l.en),
            Some("The changeset is refused".to_owned())
        );
        assert!(
            ProblemKind::ALL
                .iter()
                .all(|k| find("problem", k.slug()).is_some())
        );
        assert_eq!(catalogue.digest().len(), 16);
        assert_eq!(
            catalogue.digest(),
            CodeCatalogue::of(ModuleRegistry::empty()).digest(),
            "the fingerprint is stable"
        );
    }
}
