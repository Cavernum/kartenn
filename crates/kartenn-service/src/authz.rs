//! Authorisation port of the pipeline (spec 004, task 004-11; spec 003, REQ-003-17, 18).
//!
//! The RBAC itself (scopes, roles, bindings) is spec 005. Until then the pipeline asks an
//! [`Authorizer`]; [`Grants`] is a configurable implementation for tests and
//! [`Unrestricted`] grants everything, for system work and the tests that are not about
//! rights.

use std::collections::{BTreeSet, HashMap};
use std::future::Future;

use uuid::Uuid;

use crate::port::StoreError;

/// Permission to create, modify or disable an approval policy (REQ-004-39, 40), on the
/// policy; until the catalogue of permissions of spec 005.
pub const POLICY_ADMIN: &str = "core.approval_policy.admin";

/// Decides whether a subject holds a permission on objects (D36).
pub trait Authorizer: Send + Sync {
    /// The objects among `objects` on which `subject` lacks `permission`; grouped, so that
    /// the RBAC answers for many objects at once (003/design, § Droits).
    fn authorize_many(
        &self,
        subject: Uuid,
        permission: &str,
        objects: &[Uuid],
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send;
}

/// Grants every permission on every object.
#[derive(Debug, Clone, Copy, Default)]
pub struct Unrestricted;

impl Authorizer for Unrestricted {
    fn authorize_many(
        &self,
        _subject: Uuid,
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
}

impl Grants {
    /// No grant.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
        subject: Uuid,
        permission: &str,
        objects: &[Uuid],
    ) -> impl Future<Output = Result<Vec<Uuid>, StoreError>> + Send {
        std::future::ready(Ok(
            match self.grants.get(&(subject, permission.to_owned())) {
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
            .on(s, "prop.custom.x.write", [a])
            .everywhere(s, "prop.custom.y.write");
        assert_eq!(
            g.authorize_many(s, "prop.custom.x.write", &[a, b])
                .await
                .unwrap(),
            vec![b]
        );
        assert!(
            g.authorize_many(s, "prop.custom.y.write", &[a, b])
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            g.authorize_many(s, "prop.custom.z.write", &[a])
                .await
                .unwrap(),
            vec![a]
        );
        assert!(
            Unrestricted
                .authorize_many(s, "any", &[a])
                .await
                .unwrap()
                .is_empty()
        );
    }
}
