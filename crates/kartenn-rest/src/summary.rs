//! The summary of an object a body refers to (REQ-006-2, REQ-006-42, H23): its
//! identifier, its natural key and its URL for an object the caller may read, its
//! identifier alone otherwise.
//!
//! The readability of the objects a page refers to is asked once per kind referred to,
//! not once per reference ([`readable`]); an object the caller may not read and one that
//! did not exist at the dates of the read leave the same summary, `{id}` (005: invisible
//! equals absent).

use std::collections::BTreeSet;

use kartenn_service::audit::RequestContext;
use kartenn_service::authz::Authorizer;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::objects::{Api, Target};
use crate::problem::Problem;

/// The summary of a linked object whose natural key is its slug.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Summary {
    /// Identifier.
    pub id: Uuid,
    /// Slug; absent if the caller may not read the object or if it was absent then.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// Its URL; absent with the slug.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

impl Summary {
    /// The summary of the object `id` of `target`, of slug `slug` at the dates of the read
    /// (`None` if it did not exist then), among the objects `readable` by the caller.
    #[must_use]
    pub fn of(target: Target, id: Uuid, slug: Option<String>, readable: &Readable) -> Self {
        match slug {
            Some(slug) if readable.contains(id) => Self {
                id,
                slug: Some(slug),
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

#[cfg(test)]
mod tests {
    use super::*;
    use kartenn_store_pg::list::Kind;

    #[test]
    fn an_unreadable_object_and_an_absent_one_leave_the_same_bytes() {
        let id = Uuid::now_v7();
        let target = Target::Core(Kind::Domain);
        let all = Readable::all();
        let none = Readable {
            denied: Some(BTreeSet::from([id])),
        };
        let shown = serde_json::to_vec(&Summary::of(target, id, Some("lab".into()), &all));
        let unreadable = serde_json::to_vec(&Summary::of(target, id, Some("lab".into()), &none));
        let absent = serde_json::to_vec(&Summary::of(target, id, None, &all));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&shown.unwrap()).unwrap(),
            serde_json::json!({"id": id, "slug": "lab", "href": format!("/v1/domains/{id}")})
        );
        let bare = format!(r#"{{"id":"{id}"}}"#).into_bytes();
        assert_eq!(unreadable.unwrap(), bare);
        assert_eq!(absent.unwrap(), bare);
    }
}
