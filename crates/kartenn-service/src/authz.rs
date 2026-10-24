//! Authorisation port (spec 004, task 004-11; spec 003, REQ-003-17, 18; spec 005,
//! REQ-005-18, 23).
//!
//! The service asks an [`Authorizer`] whether a [`Subject`] holds a permission on objects;
//! the RBAC of spec 005 implements it in the storage adapter. [`Grants`] is a configurable
//! implementation for tests and [`Unrestricted`] grants everything, for system work and the
//! tests that are not about rights.

use std::collections::{BTreeSet, HashMap};
use std::future::Future;

use uuid::Uuid;

use crate::port::StoreError;

/// Permission to create, modify or disable an approval policy (REQ-004-39, 40), on the
/// policy (catalogue of spec 005).
pub const POLICY_ADMIN: &str = "core.approval_policy.admin";

/// Who asks (005/K4): an identity, the issuer that authenticated it and its groups at that
/// issuer (read from the token, never stored, except as the snapshot of a pending
/// changeset's author).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Subject {
    /// Internal identifier of the identity.
    pub id: Uuid,
    /// Issuer, for a person authenticated by OIDC.
    pub issuer: Option<String>,
    /// Groups at that issuer.
    pub groups: Vec<String>,
}

impl std::fmt::Display for Subject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
    }
}

impl From<Uuid> for Subject {
    /// An identity alone: no issuer, no group (service accounts, system work, tests).
    fn from(id: Uuid) -> Self {
        Self {
            id,
            issuer: None,
            groups: Vec::new(),
        }
    }
}

/// Decides whether a subject holds a permission on objects (D36).
pub trait Authorizer: Send + Sync {
    /// The objects among `objects` on which `subject` lacks `permission`; grouped, so that
    /// the RBAC answers for many objects at once (003/design, § Droits).
    fn authorize_many(
        &self,
        subject: &Subject,
        permission: &str,
        objects: &[Uuid],
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;

    /// [`Self::authorize_many`] as seen from inside the transaction `tx` of a changeset,
    /// whose own writes count (005, REQ-005-22); by default the transaction is not
    /// consulted.
    fn authorize_in<T: crate::port::WriteTx + Send>(
        &self,
        tx: &mut T,
        subject: &Subject,
        permission: &str,
        objects: &[Uuid],
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send {
        let _ = tx;
        self.authorize_many(subject, permission, objects)
    }
}

/// Grants every permission on every object.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unrestricted;

impl Authorizer for Unrestricted {
    fn authorize_many(
        &self,
        _subject: &Subject,
        _permission: &str,
        _objects: &[Uuid],
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send {
        std::future::ready(Ok(Vec::new()))
    }
}

/// Explicit grants: a permission on every object, or on a set of objects.
#[derive(Debug, Clone, Default)]
pub struct Grants {
    grants: HashMap<(Uuid, String), Option<BTreeSet<Uuid>>>,
    /// Permissions on every object for the members of a group of an issuer.
    groups: BTreeSet<(String, String, String)>,
}

impl Grants {
    /// No grant.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Grants `permission` on every object to the members of `group` at `issuer`.
    #[must_use]
    pub fn group(mut self, issuer: &str, group: &str, permission: &str) -> Self {
        self.groups
            .insert((issuer.to_owned(), group.to_owned(), permission.to_owned()));
        self
    }

    /// Grants `permission` to `subject` on every object.
    #[must_use]
    pub fn everywhere(mut self, subject: Uuid, permission: &str) -> Self {
        self.grants.insert((subject, permission.to_owned()), None);
        self
    }

    /// Grants `permission` to `subject` on `objects`.
    #[must_use]
    pub fn on(
        mut self,
        subject: Uuid,
        permission: &str,
        objects: impl IntoIterator<Item = Uuid>,
    ) -> Self {
        let entry = self
            .grants
            .entry((subject, permission.to_owned()))
            .or_insert_with(|| Some(BTreeSet::new()));
        if let Some(set) = entry {
            set.extend(objects);
        }
        self
    }
}

impl Authorizer for Grants {
    fn authorize_many(
        &self,
        subject: &Subject,
        permission: &str,
        objects: &[Uuid],
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send {
        let by_group = subject.issuer.as_ref().is_some_and(|issuer| {
            subject.groups.iter().any(|g| {
                self.groups
                    .contains(&(issuer.clone(), g.clone(), permission.to_owned()))
            })
        });
        if by_group {
            return std::future::ready(Ok(Vec::new()));
        }
        std::future::ready(Ok(
            match self.grants.get(&(subject.id, permission.to_owned())) {
                Some(None) => Vec::new(),
                Some(Some(allowed)) => objects
                    .iter()
                    .filter(|o| !allowed.contains(o))
                    .copied()
                    .collect(),
                None => objects.to_vec(),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn grants_deny_what_they_do_not_cover() {
        let (s, a, b) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(3));
        let g = Grants::new()
            .on(s, "prop.custom.x.edit", [a])
            .everywhere(s, "prop.custom.y.edit");
        assert_eq!(
            g.authorize_many(&s.into(), "prop.custom.x.edit", &[a, b])
                .await
                .unwrap(),
            vec![b]
        );
        assert!(
            g.authorize_many(&s.into(), "prop.custom.y.edit", &[a, b])
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            g.authorize_many(&s.into(), "prop.custom.z.edit", &[a])
                .await
                .unwrap(),
            vec![a]
        );
        assert!(
            Unrestricted
                .authorize_many(&s.into(), "any", &[a])
                .await
                .unwrap()
                .is_empty()
        );
    }
}
