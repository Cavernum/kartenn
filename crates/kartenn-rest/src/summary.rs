//! The summary of an object a body refers to (REQ-006-2, REQ-006-42, H23): its
//! identifier, its natural key (`label`, and `slug` for a kind that has one) and its URL
//! for an object the caller may read, its identifier alone otherwise.
//!
//! The readability of the objects a page refers to is asked once per kind referred to,
//! not once per reference ([`readable`]); an object the caller may not read and one that
//! did not exist at the dates of the read leave the same summary, `{id}` (005: invisible
//! equals absent). A body that refers to objects of any kind reads their keys together
//! ([`Summaries::read`]).
//!
//! The `label` of each kind (H23, précision du 2026-11-15) is its key as a human reads
//! it, and says nothing of another object: the slug of a domain, an entity, a site or a
//! group of VLANs; the canonical value of a prefix or an address; `<first> – <last>` for
//! a range; `<vid> · <name>` for a VLAN; for a type of a module, the key its module gives
//! ([`kartenn_service::module::ModuleHandler::keys`]).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use kartenn_service::audit::RequestContext;
use kartenn_service::authz::Authorizer;
use kartenn_service::operations::ModuleKind;
use kartenn_store_pg::keys::natural_keys;
use kartenn_store_pg::list::Kind;
use kartenn_store_pg::modules::PgModuleRead;
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::objects::{Api, Target, address_text, instant, prefix_text, range_bounds, store};
use crate::problem::Problem;

/// The summary of a linked object: `{id, label, href}`, with `slug` for a kind that has
/// one; `{id}` alone for an object the caller may not read or that was absent then.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Summary {
    /// Identifier.
    pub id: Uuid,
    /// Slug, for a kind that has one; absent with `label`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// Its natural key as a human reads it (REQ-006-42); absent if the caller may not
    /// read the object or if it was absent then.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Its URL; absent with `label`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

impl Summary {
    /// The summary of the object `id` of `target`, whose natural key is its slug `slug`
    /// at the dates of the read (`None` if it did not exist then), among the objects
    /// `readable` by the caller: the slug is its label.
    #[must_use]
    pub fn of(target: Target, id: Uuid, slug: Option<String>, readable: &Readable) -> Self {
        match slug {
            Some(slug) if readable.contains(id) => Self {
                id,
                label: Some(slug.clone()),
                slug: Some(slug),
                href: Some(target.href(id)),
            },
            _ => Self::bare(id),
        }
    }

    /// The summary of the object `id` of `target`, of a kind without a slug, whose key
    /// reads `label` at the dates of the read (`None` if it did not exist then), among the
    /// objects `readable` by the caller.
    #[must_use]
    pub fn labelled(target: Target, id: Uuid, label: Option<String>, readable: &Readable) -> Self {
        match label {
            Some(label) if readable.contains(id) => Self {
                id,
                slug: None,
                label: Some(label),
                href: Some(target.href(id)),
            },
            _ => Self::bare(id),
        }
    }

    /// The identifier alone: what an unreadable or absent object leaves.
    #[must_use]
    pub const fn bare(id: Uuid) -> Self {
        Self {
            id,
            slug: None,
            label: None,
            href: None,
        }
    }
}

/// The objects a caller may read now among those a body refers to.
#[derive(Debug, Clone)]
pub struct Readable {
    /// `None`: a request without a subject, which reads all (as [`crate::properties`]).
    denied: Option<BTreeSet<Uuid>>,
}

impl Readable {
    /// Every object: for a body that refers to none, or a request without a subject.
    #[must_use]
    pub const fn all() -> Self {
        Self { denied: None }
    }

    /// Whether the caller may read `id`.
    #[must_use]
    pub fn contains(&self, id: Uuid) -> bool {
        self.denied
            .as_ref()
            .is_none_or(|denied| !denied.contains(&id))
    }

    /// Adds the objects among `ids`, all of `target`, that the caller of `context` may
    /// not read now: one authorisation for them all, without a trace of refusal (an
    /// unreadable object is not asked for, it is referred to).
    ///
    /// # Errors
    ///
    /// The authoriser's error.
    pub async fn of(
        mut self,
        api: &Api,
        context: &RequestContext,
        target: Target,
        ids: impl IntoIterator<Item = Uuid>,
    ) -> Result<Self, Problem> {
        let Some(subject) = context.subject() else {
            return Ok(self);
        };
        let ids: Vec<Uuid> = ids
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let denied = self.denied.get_or_insert_with(BTreeSet::new);
        if ids.is_empty() {
            return Ok(self);
        }
        denied.extend(
            api.authorizer()
                .authorize_many(&subject, &target.read_permission(), &ids)
                .await?,
        );
        Ok(self)
    }
}

/// The objects among `ids`, all of `target`, that the caller of `context` may read now.
///
/// # Errors
///
/// The authoriser's error.
pub async fn readable(
    api: &Api,
    context: &RequestContext,
    target: Target,
    ids: impl IntoIterator<Item = Uuid>,
) -> Result<Readable, Problem> {
    Readable { denied: None }
        .of(api, context, target, ids)
        .await
}

/// The key of an object, as it shows in its summary.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Key {
    /// A slug, which is also the label.
    Slug(String),
    /// A label of a kind without a slug.
    Label(String),
}

/// The summaries of objects of any kind a body refers to (an anomaly, a value of type
/// reference): their kinds and keys read in one query at the dates of the body, plus one
/// per module whose types are referred to, then one authorisation per kind.
#[derive(Debug, Clone)]
pub struct Summaries {
    found: HashMap<Uuid, (Target, Key)>,
    readable: Readable,
}

impl Summaries {
    /// The summaries of `ids` at `valid_at` as asserted at `as_of`, as the caller of
    /// `context` may read them now.
    ///
    /// # Errors
    ///
    /// The store's or the authoriser's error.
    pub async fn read(
        api: &Api,
        context: &RequestContext,
        ids: impl IntoIterator<Item = Uuid>,
        valid_at: OffsetDateTime,
        as_of: Option<OffsetDateTime>,
    ) -> Result<Self, Problem> {
        let found = keys(api, ids, valid_at, as_of).await?;
        let mut by_target: BTreeMap<&'static str, (Target, Vec<Uuid>)> = BTreeMap::new();
        for (id, (target, _)) in &found {
            by_target
                .entry(target.as_str())
                .or_insert_with(|| (*target, Vec::new()))
                .1
                .push(*id);
        }
        let mut readable = Readable::all();
        for (target, ids) in by_target.into_values() {
            readable = readable.of(api, context, target, ids).await?;
        }
        Ok(Self { found, readable })
    }

    /// The labels of `ids` at `valid_at` as asserted at `as_of`, of those that existed
    /// then and whose key is known, without a judgement of their readability: for objects
    /// the caller is known to read (task 006-33f3).
    ///
    /// # Errors
    ///
    /// The store's error.
    pub async fn labels(
        api: &Api,
        ids: impl IntoIterator<Item = Uuid>,
        valid_at: OffsetDateTime,
        as_of: Option<OffsetDateTime>,
    ) -> Result<HashMap<Uuid, String>, Problem> {
        Ok(keys(api, ids, valid_at, as_of)
            .await?
            .into_iter()
            .map(|(id, (_, key))| match key {
                Key::Slug(label) | Key::Label(label) => (id, label),
            })
            .collect())
    }
}

/// The kinds and the keys of `ids` at `valid_at` as asserted at `as_of`: one query for the
/// core, one per module visited; none for no identifier.
async fn keys(
    api: &Api,
    ids: impl IntoIterator<Item = Uuid>,
    valid_at: OffsetDateTime,
    as_of: Option<OffsetDateTime>,
) -> Result<HashMap<Uuid, (Target, Key)>, Problem> {
    let ids: Vec<Uuid> = ids
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut found = HashMap::new();
    if ids.is_empty() {
        return Ok(found);
    }
    // The objects of the modules, by module: their keys are the modules'.
    let mut of_modules: BTreeMap<&'static str, (Target, Vec<Uuid>)> = BTreeMap::new();
    for (id, kind, key) in natural_keys(&api.pool, &ids, valid_at, as_of)
        .await
        .map_err(|e| store(&e))?
    {
        let Some(target) = Target::named(&kind, &api.modules) else {
            continue;
        };
        match key {
            Some(raw) => {
                if let Some(key) = key_of(target, &raw) {
                    found.insert(id, (target, key));
                }
            }
            None => of_modules
                .entry(target.as_str())
                .or_insert_with(|| (target, Vec::new()))
                .1
                .push(id),
        }
    }
    for (target, objects) in of_modules.into_values() {
        let Some(handler) =
            ModuleKind::parse(target.as_str()).and_then(|kind| api.modules.handler_of(kind))
        else {
            continue;
        };
        let mut reader = PgModuleRead::open(&api.pool, instant(valid_at), as_of.map(instant))
            .await
            .map_err(|e| store(&e))?;
        let keys = handler.keys(&mut reader, &objects).await?;
        found.extend(
            keys.into_iter()
                .filter(|(id, _)| objects.contains(id))
                .map(|(id, label)| (id, (target, Key::Label(label)))),
        );
    }
    Ok(found)
}

impl Summaries {
    /// The kind of `id`, if it existed at the dates of the read.
    #[must_use]
    pub fn target(&self, id: Uuid) -> Option<Target> {
        self.found.get(&id).map(|(target, _)| *target)
    }

    /// The summary of `id`: `{id}` alone if the caller may not read it or if it did not
    /// exist at the dates of the read.
    #[must_use]
    pub fn of(&self, id: Uuid) -> Summary {
        match self.found.get(&id) {
            Some((target, Key::Slug(slug))) => {
                Summary::of(*target, id, Some(slug.clone()), &self.readable)
            }
            Some((target, Key::Label(label))) => {
                Summary::labelled(*target, id, Some(label.clone()), &self.readable)
            }
            None => Summary::bare(id),
        }
    }
}

/// The key of an object of `target` from its natural key `raw` as stored
/// ([`natural_keys`]); `None` for a stored value that does not decode.
fn key_of(target: Target, raw: &str) -> Option<Key> {
    Some(match target {
        Target::Core(Kind::Prefix) => Key::Label(prefix_text(raw)?),
        Target::Core(Kind::Range) => {
            let (first, last) = range_bounds(raw)?;
            Key::Label(format!("{first} – {last}"))
        }
        Target::Core(Kind::Address) => Key::Label(address_text(raw)?),
        Target::Core(Kind::Entity | Kind::Site | Kind::Domain) => Key::Slug(raw.to_owned()),
        Target::Module(ty) if ty.kind == kartenn_service::vlan::GROUP => Key::Slug(raw.to_owned()),
        Target::Module(_) => Key::Label(raw.to_owned()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(summary: &Summary) -> Vec<u8> {
        serde_json::to_vec(summary).expect("a summary serialises")
    }

    #[test]
    fn an_unreadable_object_and_an_absent_one_leave_the_same_bytes() {
        let id = Uuid::now_v7();
        let all = Readable::all();
        let none = Readable {
            denied: Some(BTreeSet::from([id])),
        };
        let bare = format!(r#"{{"id":"{id}"}}"#).into_bytes();
        // A kind with a slug.
        let domain = Target::Core(Kind::Domain);
        assert_eq!(
            serde_json::to_value(Summary::of(domain, id, Some("lab".into()), &all)).unwrap(),
            serde_json::json!({
                "id": id, "slug": "lab", "label": "lab", "href": format!("/v1/domains/{id}"),
            })
        );
        assert_eq!(
            bytes(&Summary::of(domain, id, Some("lab".into()), &none)),
            bare
        );
        assert_eq!(bytes(&Summary::of(domain, id, None, &all)), bare);
        // A kind without one, its key composed.
        let prefix = Target::Core(Kind::Prefix);
        let label = || Some("192.0.2.0/24".to_owned());
        assert_eq!(
            serde_json::to_value(Summary::labelled(prefix, id, label(), &all)).unwrap(),
            serde_json::json!({
                "id": id, "label": "192.0.2.0/24", "href": format!("/v1/prefixes/{id}"),
            })
        );
        assert_eq!(bytes(&Summary::labelled(prefix, id, label(), &none)), bare);
        assert_eq!(bytes(&Summary::labelled(prefix, id, None, &all)), bare);
        // Read together: an object of no kind read is absent, alike.
        let read = Summaries {
            found: HashMap::from([(id, (prefix, Key::Label("192.0.2.0/24".into())))]),
            readable: none,
        };
        assert_eq!(bytes(&read.of(id)), bare);
        let unknown = Uuid::now_v7();
        assert_eq!(
            bytes(&read.of(unknown)),
            format!(r#"{{"id":"{unknown}"}}"#).into_bytes()
        );
    }

    #[test]
    fn each_kind_reads_its_key_as_a_human_does() {
        let label = |target, raw| key_of(target, raw);
        assert_eq!(
            label(Target::Core(Kind::Prefix), "2001:0db8::/32"),
            Some(Key::Label("2001:db8::/32".into()))
        );
        assert_eq!(
            label(Target::Core(Kind::Range), "[192.0.2.10,192.0.2.20]"),
            Some(Key::Label("192.0.2.10 – 192.0.2.20".into()))
        );
        assert_eq!(
            label(Target::Core(Kind::Address), "192.0.2.7/32"),
            Some(Key::Label("192.0.2.7".into()))
        );
        assert_eq!(
            label(Target::Core(Kind::Site), "paris"),
            Some(Key::Slug("paris".into()))
        );
        assert_eq!(label(Target::Core(Kind::Prefix), "not a prefix"), None);
    }
}
