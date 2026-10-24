//! Administration of the RBAC by changesets (spec 005, REQ-005-3, 17, 25, 26; K6, K7,
//! K11): roles, bindings, attachments of issuers, state of identities; planned with
//! local checks, then checked against the rights of the author, without escalation.

use kartenn_core::authz::{BUILTIN_ROLES, valid_role_entry};
use kartenn_core::rules::{self, Violation};
use uuid::Uuid;

use crate::authz::{Authorizer, Subject};
use crate::declarations::SYSTEM_AUTHOR;
use crate::operations::{BindingScope, BindingSubject, Operation, Ref, SecretRecord};
use crate::pipeline::Planned;
use crate::port::{StoreError, WriteTx};

/// The object that stands for the settings of the RBAC (roles, issuers, accounts) in
/// authorisations: only a global binding reaches it (no owner, out of every scope).
pub const RBAC_OBJECT: Uuid = Uuid::from_u128(2);

/// An administration operation, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RbacOp {
    /// Defines a role or replaces its labels and entries.
    SetRole {
        /// Key.
        key: String,
        /// Labels by language, checked.
        labels: kartenn_core::labels::Labels,
        /// Entries.
        entries: Vec<String>,
    },
    /// Closes a role and its bindings.
    CloseRole {
        /// Key.
        key: String,
    },
    /// A new binding.
    Bind {
        /// Its identifier.
        id: Uuid,
        /// Who.
        subject: BindingSubject,
        /// Role key.
        role: String,
        /// `global`, `entity`, `site`, `domain` or `prefix`.
        scope_kind: &'static str,
        /// The scope; `None` for global.
        scope: Option<Uuid>,
    },
    /// Ends a binding.
    Unbind {
        /// The binding.
        id: Uuid,
    },
    /// Attaches an issuer.
    Attach {
        /// Issuer.
        issuer: String,
        /// Entity; `None`: everywhere.
        entity: Option<Uuid>,
    },
    /// Detaches an issuer.
    Detach {
        /// Issuer.
        issuer: String,
        /// Entity; `None`: the global attachment.
        entity: Option<Uuid>,
    },
    /// A new service account with its first secret.
    CreateAccount {
        /// Its identifier.
        id: Uuid,
        /// Name.
        name: String,
        /// First secret.
        secret: SecretRecord,
    },
    /// A new secret of a service account.
    AddSecret {
        /// The account.
        account: Uuid,
        /// The secret.
        secret: SecretRecord,
    },
    /// A secret revoked.
    RevokeSecret {
        /// The account.
        account: Uuid,
        /// The secret.
        secret: Uuid,
    },
    /// Disables or enables an identity, revokes its sessions.
    IdentityState {
        /// The identity.
        identity: Uuid,
        /// `Some(true)` disables, `Some(false)` enables.
        disabled: Option<bool>,
        /// Revokes the sessions opened until now.
        revoke_sessions: bool,
    },
}

impl RbacOp {
    /// The object it writes.
    #[must_use]
    pub const fn object(&self) -> Uuid {
        match self {
            Self::Bind { id, .. } | Self::Unbind { id } | Self::CreateAccount { id, .. } => *id,
            Self::IdentityState { identity, .. } => *identity,
            Self::AddSecret { account, .. } | Self::RevokeSecret { account, .. } => *account,
            _ => RBAC_OBJECT,
        }
    }
}

fn invalid(message: String) -> Violation {
    Violation::new(&rules::RBAC_INVALID, message)
}

fn role_key(key: &str) -> Result<String, Violation> {
    let key = key.trim();
    let valid = key.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if valid {
        Ok(key.to_owned())
    } else {
        Err(invalid(format!("invalid role key '{key}'")))
    }
}

fn issuer(value: &str) -> Result<String, Violation> {
    let value = value.trim();
    if value.is_empty() {
        Err(invalid("an issuer is not empty".into()))
    } else {
        Ok(value.to_owned())
    }
}

/// A secret record with a digest of 64 hexadecimal digits.
fn secret_record(secret: &SecretRecord) -> Result<SecretRecord, Violation> {
    if secret.digest.len() == 64 && secret.digest.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(secret.clone())
    } else {
        Err(invalid("a secret digest has 64 hexadecimal digits".into()))
    }
}

/// Plans a binding: a subject well formed, a role key, its scope resolved.
fn plan_bind(
    id: Uuid,
    subject: &BindingSubject,
    role: &str,
    scope: BindingScope,
    resolve: &impl Fn(Ref) -> Uuid,
) -> Result<RbacOp, Violation> {
    let subject = match subject {
        BindingSubject::Identity { id } => BindingSubject::Identity { id: *id },
        BindingSubject::Group { issuer: i, group } => {
            if group.trim().is_empty() {
                return Err(invalid("a group is not empty".into()));
            }
            BindingSubject::Group {
                issuer: issuer(i)?,
                group: group.trim().to_owned(),
            }
        }
        BindingSubject::Issuer { issuer: i } => BindingSubject::Issuer { issuer: issuer(i)? },
    };
    let (scope_kind, scope) = match scope {
        BindingScope::Global => ("global", None),
        BindingScope::Entity { id } => ("entity", Some(resolve(id))),
        BindingScope::Site { id } => ("site", Some(resolve(id))),
        BindingScope::Domain { id } => ("domain", Some(resolve(id))),
        BindingScope::Prefix { id } => ("prefix", Some(resolve(id))),
    };
    Ok(RbacOp::Bind {
        id,
        subject,
        role: role_key(role)?,
        scope_kind,
        scope,
    })
}

/// Plans `op` when it is an administration operation of the RBAC; `None` otherwise.
pub(crate) fn plan(
    op: &Operation,
    id: Uuid,
    resolve: impl Fn(Ref) -> Uuid,
) -> Option<Result<Planned, Violation>> {
    let planned = match op {
        Operation::SetRole {
            key,
            labels,
            entries,
        } => (|| {
            let key = role_key(key)?;
            let labels = kartenn_core::labels::Labels::parse_map(labels).and_then(|l| {
                if l.is_empty() {
                    Err(invalid(format!("the role '{key}' needs a label")))
                } else {
                    Ok(l)
                }
            })?;
            if let Some(bad) = entries.iter().find(|e| !valid_role_entry(e)) {
                // An old name is refused, its new ones named (spec 018, REQ-018-3; spec
                // 024, REQ-024-9).
                let catalogue: Vec<&kartenn_core::authz::Permission> =
                    kartenn_core::authz::PERMISSIONS
                        .iter()
                        .copied()
                        .chain(kartenn_core::authz::module_permissions())
                        .collect();
                let renamed = kartenn_core::authz::retired(bad, &catalogue)
                    .map_or_else(String::new, |new| {
                        format!(" (replaced by '{}')", new.join("', '"))
                    });
                return Err(invalid(format!(
                    "'{bad}' is not a permission of the catalogue{renamed}"
                )));
            }
            let mut entries = entries.clone();
            entries.sort();
            entries.dedup();
            Ok(RbacOp::SetRole {
                key,
                labels,
                entries,
            })
        })(),
        Operation::CloseRole { key } => role_key(key).map(|key| RbacOp::CloseRole { key }),
        Operation::Bind {
            subject,
            role,
            scope,
            ..
        } => plan_bind(id, subject, role, *scope, &resolve),
        Operation::Unbind { id } => Ok(RbacOp::Unbind { id: *id }),
        Operation::AttachIssuer { issuer: i, entity } => issuer(i).map(|issuer| RbacOp::Attach {
            issuer,
            entity: entity.map(&resolve),
        }),
        Operation::DetachIssuer { issuer: i, entity } => issuer(i).map(|issuer| RbacOp::Detach {
            issuer,
            entity: entity.map(&resolve),
        }),
        Operation::CreateServiceAccount { name, secret, .. } => (|| {
            let name = name.trim();
            if name.is_empty() || name.contains(':') || name.chars().any(char::is_control) {
                return Err(invalid(format!("'{name}' is not a service account name")));
            }
            Ok(RbacOp::CreateAccount {
                id,
                name: name.to_owned(),
                secret: secret_record(secret)?,
            })
        })(),
        Operation::AddServiceSecret { account, secret } => {
            secret_record(secret).map(|secret| RbacOp::AddSecret {
                account: *account,
                secret,
            })
        }
        Operation::RevokeServiceSecret { account, secret } => Ok(RbacOp::RevokeSecret {
            account: *account,
            secret: *secret,
        }),
        Operation::SetIdentityState {
            identity,
            disabled,
            revoke_sessions,
        } => Ok(RbacOp::IdentityState {
            identity: *identity,
            disabled: *disabled,
            revoke_sessions: *revoke_sessions,
        }),
        _ => return None,
    };
    Some(planned.map(Planned::Rbac))
}

/// Whether `key` is a delivered role.
fn builtin(key: &str) -> bool {
    BUILTIN_ROLES.iter().any(|r| r.key == key)
}

/// Checks the rights of `subject` on the administration operations of the changeset
/// (REQ-005-26): the administration permissions on the global scope; a binding needs
/// `core.rbac.admin` on its scope and every entry of its role there (no escalation); a
/// delivered role changes only by the synchronisation of the core. Each administration
/// permission asked is added to `required` when given: whoever applies the changeset
/// needs it too (spec 024, REQ-024-11).
pub(crate) async fn check<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    subject: &Subject,
    planned: &[Planned],
    mut required: Option<&mut crate::rights::Required>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let forbidden = |permission: &str| {
        Violation::new(
            &rules::RBAC_FORBIDDEN,
            format!("this administration needs {permission}"),
        )
    };
    undeclared(tx, planned, violations).await?;
    for p in planned {
        let Planned::Rbac(op) = p else { continue };
        let (permission, scope) = match op {
            RbacOp::SetRole { key, .. } | RbacOp::CloseRole { key } => {
                if builtin(key) && subject.id != SYSTEM_AUTHOR {
                    violations.push(Violation::new(
                        &rules::RBAC_BUILTIN,
                        format!("the role '{key}' is delivered with kartenn"),
                    ));
                    continue;
                }
                ("core.role.admin", RBAC_OBJECT)
            }
            RbacOp::Attach { .. } | RbacOp::Detach { .. } => ("core.issuer.admin", RBAC_OBJECT),
            RbacOp::IdentityState { .. }
            | RbacOp::CreateAccount { .. }
            | RbacOp::AddSecret { .. }
            | RbacOp::RevokeSecret { .. } => ("core.account.admin", RBAC_OBJECT),
            RbacOp::Bind { scope, .. } => ("core.rbac.admin", scope.unwrap_or(RBAC_OBJECT)),
            RbacOp::Unbind { id } => match tx.binding_scope(*id).await? {
                Some(scope) => ("core.rbac.admin", scope),
                // An unknown binding: refused by the write.
                None => continue,
            },
        };
        if let Some(required) = required.as_deref_mut() {
            required.push((permission.to_owned(), vec![scope]));
        }
        if !authorizer
            .authorize_in(tx, subject, permission, &[scope])
            .await?
            .is_empty()
        {
            violations.push(forbidden(permission).with_subjects([op.object()]));
            continue;
        }
        // No escalation: every entry of the role, held by the author on the scope.
        if let RbacOp::Bind { role, id, .. } = op {
            let entries = match planned.iter().find_map(|p| match p {
                Planned::Rbac(RbacOp::SetRole { key, entries, .. }) if key == role => {
                    Some(entries.clone())
                }
                _ => None,
            }) {
                Some(entries) => Some(entries),
                None => tx.role_entries(role).await?,
            };
            let Some(entries) = entries else {
                violations.push(
                    Violation::new(&rules::RBAC_UNKNOWN_ROLE, format!("no role '{role}'"))
                        .with_subjects([*id]),
                );
                continue;
            };
            let mut lacking = Vec::new();
            for entry in &entries {
                if !authorizer
                    .authorize_in(tx, subject, entry, &[scope])
                    .await?
                    .is_empty()
                {
                    lacking.push(entry.as_str());
                }
            }
            if !lacking.is_empty() {
                violations.push(
                    Violation::new(
                        &rules::RBAC_ESCALATION,
                        format!(
                            "the role '{role}' holds {} that the author lacks there",
                            lacking.join(", ")
                        ),
                    )
                    .with_subjects([*id]),
                );
            }
        }
    }
    Ok(())
}

/// The property permissions of the roles `planned` sets whose key is declared neither in
/// the base nor by the same changeset (REQ-005-17): `core.role.invalid`.
async fn undeclared<T: WriteTx + Send>(
    tx: &mut T,
    planned: &[Planned],
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    let mut declared: std::collections::BTreeSet<String> = planned
        .iter()
        .filter_map(|p| match p {
            Planned::Declare { declaration, .. } => Some(declaration.key.to_string()),
            _ => None,
        })
        .collect();
    let mut namespaces = std::collections::BTreeSet::new();
    for p in planned {
        let Planned::Rbac(RbacOp::SetRole {
            key: role, entries, ..
        }) = p
        else {
            continue;
        };
        for entry in entries {
            let Some((key, _)) = entry
                .strip_prefix("prop.")
                .and_then(|rest| rest.rsplit_once('.'))
            else {
                continue;
            };
            if key == "*" {
                continue;
            }
            let namespace = key.split('.').next().unwrap_or_default();
            if namespaces.insert(namespace.to_owned()) {
                declared.extend(
                    tx.declarations_in(namespace)
                        .await?
                        .into_iter()
                        .map(|(_, d, _)| d.key.to_string()),
                );
            }
            if !declared.contains(key) {
                violations.push(
                    invalid(format!(
                        "the role '{role}' names the property '{key}', which is not declared"
                    ))
                    .with_subjects([RBAC_OBJECT]),
                );
            }
        }
    }
    Ok(())
}

/// A role as stored now: its key, labels and entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredRole {
    /// Its key.
    pub key: String,
    /// Its labels by language.
    pub labels: crate::operations::RawLabels,
    /// Its entries, sorted.
    pub entries: Vec<String>,
}

/// The motive of the changeset that renames the permissions of the custom roles.
pub const RENAME_MOTIVE: &str = "permission rename 024";

/// Replaces the old permissions of the custom roles (spec 018, REQ-018-4, 5; Q-018-3;
/// spec 024, REQ-024-9): one system changeset setting each role not delivered with
/// kartenn that holds a retired name ([`kartenn_core::authz::retired_names`] of the
/// catalogue of the build), with the names replacing it in its place (once, when the role
/// holds several): `write` → `edit` + `apply_own`, `propose` → `edit`,
/// the retired approval → every `apply`. The bindings, the history and the audit are
/// untouched; a second run finds nothing and submits nothing. Called by the migration job.
///
/// # Errors
///
/// [`crate::RequestError`] on infrastructure failure.
pub async fn rename_permissions<S: crate::port::Store>(
    store: &S,
    max_operations: usize,
) -> Result<Option<crate::Outcome>, crate::RequestError> {
    let catalogue: Vec<&kartenn_core::authz::Permission> = kartenn_core::authz::PERMISSIONS
        .iter()
        .copied()
        .chain(store.modules().permissions())
        .collect();
    let retired = kartenn_core::authz::retired_names(&catalogue);
    let olds: Vec<&str> = retired.iter().map(|(old, _)| old.as_str()).collect();
    let mut tx = store.begin().await?;
    let roles = tx.custom_roles_citing(&olds).await?;
    tx.rollback().await?;
    let operations: Vec<Operation> = roles
        .into_iter()
        .map(|role| {
            let mut entries: Vec<String> = role
                .entries
                .into_iter()
                .flat_map(|e| {
                    kartenn_core::authz::retired(&e, &catalogue).unwrap_or_else(|| vec![e])
                })
                .collect();
            entries.sort();
            entries.dedup();
            Operation::SetRole {
                key: role.key,
                labels: role.labels,
                entries,
            }
        })
        .collect();
    if operations.is_empty() {
        return Ok(None);
    }
    let request = crate::ChangesetRequest {
        author: SYSTEM_AUTHOR,
        motive: RENAME_MOTIVE.into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
        apply_own: true,
    };
    crate::submit(store, &crate::authz::Unrestricted, &request, max_operations)
        .await
        .map(Some)
}

/// Synchronises the delivered roles with the code (K12): one system changeset setting
/// each role whose entries or label differ. Called by the migration job.
///
/// # Errors
///
/// [`crate::RequestError`] on infrastructure failure.
pub async fn sync_roles<S: crate::port::Store>(
    store: &S,
    max_operations: usize,
) -> Result<Option<crate::Outcome>, crate::RequestError> {
    let mut operations = Vec::new();
    // The delivered roles that read read the objects of the modules too (REQ-008-8).
    let modules = store.modules().permissions();
    for role in BUILTIN_ROLES {
        let mut tx = store.begin().await?;
        let stored = tx.role_entries(role.key).await?;
        let labels = tx.role_labels(role.key).await?;
        tx.rollback().await?;
        let mut wanted: Vec<String> = role
            .entries_with(&modules)
            .into_iter()
            .map(str::to_owned)
            .collect();
        wanted.sort();
        // Its French and English labels too (spec 018, REQ-018-13): a delivered role is
        // changed by this synchronisation only (`core.role.builtin`).
        let wanted_labels = role.labels.raw();
        if stored.as_ref() != Some(&wanted) || labels.as_ref() != Some(&wanted_labels) {
            operations.push(Operation::SetRole {
                key: role.key.to_owned(),
                labels: wanted_labels,
                entries: wanted,
            });
        }
    }
    if operations.is_empty() {
        return Ok(None);
    }
    let request = crate::ChangesetRequest {
        author: SYSTEM_AUTHOR,
        motive: "roles sync".into(),
        period: None,
        operations,
        simulate: false,
        idempotency_key: None,
        apply_own: true,
    };
    crate::submit(store, &crate::authz::Unrestricted, &request, max_operations)
        .await
        .map(Some)
}
