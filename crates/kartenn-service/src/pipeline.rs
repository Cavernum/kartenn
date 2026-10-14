//! The changeset pipeline (004/design, § Pipeline d'un changeset).

use std::collections::{BTreeSet, HashMap};

use kartenn_core::addr::{Addr, AddrError, Prefix};
use kartenn_core::family::ALL;
use kartenn_core::objects::{self, Address, PrefixObject, Slug};
use kartenn_core::period::{Instant, Period};
use kartenn_core::registry::special_registry;
use kartenn_core::rules::{self, Severity, Violation};
use uuid::Uuid;

use crate::operations::{ChangesetRequest, ObjectKind, Operation, PeriodSpec, Ref};
use crate::port::{Journal, Replay, Store, StoreError, WriteError, WriteTx};

/// Default maximum number of operations of a changeset (004/C3).
pub const MAX_OPERATIONS: usize = 10_000;

/// A request that cannot even be considered (malformed); it is not journaled.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RequestError {
    /// No operation.
    #[error("the changeset has no operation")]
    Empty,
    /// Empty motive (REQ-004-1).
    #[error("the changeset needs a motive")]
    MotiveRequired,
    /// Too many operations (REQ-004-5).
    #[error("the changeset has {count} operations, more than the maximum of {max}")]
    TooManyOperations {
        /// Operations submitted.
        count: usize,
        /// Configured maximum.
        max: usize,
    },
    /// Empty or inverted period (REQ-004-13).
    #[error("the period of the changeset is empty or inverted")]
    InvalidPeriod,
    /// A local reference is used before (or without) being defined (REQ-004-4).
    #[error("operation {operation} uses ${local}, which no earlier operation defines")]
    UnknownReference {
        /// Index of the operation.
        operation: usize,
        /// Local name.
        local: u32,
    },
    /// A local name is defined twice.
    #[error("${0} is defined twice")]
    DuplicateLocal(u32),
    /// A local reference designates an object of another kind.
    #[error("operation {operation} expects a {expected:?} for ${local}")]
    WrongReferenceKind {
        /// Index of the operation.
        operation: usize,
        /// Local name.
        local: u32,
        /// Kind expected.
        expected: ObjectKind,
    },
    /// The idempotency key was already used with a different content (REQ-004-10).
    #[error("the idempotency key was already used for a different changeset")]
    IdempotencyConflict,
    /// Infrastructure failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Result of a submitted changeset.
#[derive(Debug)]
#[non_exhaustive]
pub enum Outcome {
    /// Applied; warnings are returned as anomalies.
    Applied {
        /// Changeset identifier.
        changeset: Uuid,
        /// Warnings.
        warnings: Vec<Violation>,
    },
    /// Refused by at least one error rule; nothing was written but the journal.
    Refused {
        /// Changeset identifier.
        changeset: Uuid,
        /// All violations found.
        violations: Vec<Violation>,
    },
    /// Same idempotency key and content as an earlier submission: its result (REQ-004-10).
    Replayed {
        /// The earlier changeset.
        changeset: Uuid,
        /// Its state.
        state: String,
    },
    /// Simulation: what would happen, nothing written (REQ-004-9).
    Simulated {
        /// All violations found (errors and warnings).
        violations: Vec<Violation>,
    },
}

fn addr_violation(err: &AddrError) -> Violation {
    let rule = match err {
        AddrError::Zone { .. } => &rules::ADDR_ZONE,
        AddrError::PrefixSyntax { .. } | AddrError::LengthOutOfRange { .. } => {
            &rules::PREFIX_SYNTAX
        }
        AddrError::HostBitsSet { .. } => &rules::PREFIX_UNALIGNED,
        _ => &rules::ADDR_SYNTAX,
    };
    Violation::new(rule, err.to_string())
}

fn to_period(spec: Option<PeriodSpec>) -> Result<Option<Period>, RequestError> {
    spec.map(|s| {
        Period::new(Instant(s.start), s.end.map(Instant)).map_err(|_| RequestError::InvalidPeriod)
    })
    .transpose()
}

/// An operation ready to be written: parsed, locally validated, references resolved.
#[derive(Debug)]
enum Planned {
    Domain {
        id: Uuid,
        slug: Slug,
    },
    Prefix {
        object: PrefixObject,
        public: bool,
    },
    Address {
        object: Address,
        public: bool,
    },
    Close {
        kind: ObjectKind,
        id: Uuid,
        expected: Option<Uuid>,
    },
}

/// Checks the shape of the request (REQ-004-1 to 5): size, motive, period, references.
fn check_request(req: &ChangesetRequest, max: usize) -> Result<Option<Period>, RequestError> {
    if req.operations.is_empty() {
        return Err(RequestError::Empty);
    }
    if req.motive.trim().is_empty() {
        return Err(RequestError::MotiveRequired);
    }
    if req.operations.len() > max {
        return Err(RequestError::TooManyOperations {
            count: req.operations.len(),
            max,
        });
    }
    let period = to_period(req.period)?;
    let mut defined: HashMap<u32, ObjectKind> = HashMap::new();
    for (i, op) in req.operations.iter().enumerate() {
        let uses: Vec<(Ref, ObjectKind)> = match op {
            Operation::CreatePrefix { domain, .. } | Operation::CreateAddress { domain, .. } => {
                vec![(*domain, ObjectKind::Domain)]
            }
            Operation::Close { kind, id, .. } => vec![(*id, *kind)],
            Operation::CreateDomain { .. } => vec![],
        };
        for (r, expected) in uses {
            if let Ref::Local(local) = r {
                match defined.get(&local) {
                    None => {
                        return Err(RequestError::UnknownReference {
                            operation: i,
                            local,
                        });
                    }
                    Some(kind) if *kind != expected => {
                        return Err(RequestError::WrongReferenceKind {
                            operation: i,
                            local,
                            expected,
                        });
                    }
                    Some(_) => {}
                }
            }
        }
        if let Some(local) = op.defines() {
            let kind = match op {
                Operation::CreateDomain { .. } => ObjectKind::Domain,
                Operation::CreatePrefix { .. } => ObjectKind::Prefix,
                _ => ObjectKind::Address,
            };
            if defined.insert(local, kind).is_some() {
                return Err(RequestError::DuplicateLocal(local));
            }
        }
    }
    Ok(period)
}

/// Parses every operation and runs the local rules (no database).
fn plan(req: &ChangesetRequest) -> (Vec<Planned>, Vec<Violation>) {
    let mut ids: HashMap<u32, Uuid> = HashMap::new();
    let resolve = |r: Ref, ids: &HashMap<u32, Uuid>| match r {
        Ref::Id(id) => id,
        Ref::Local(n) => ids[&n],
    };
    let mut planned = Vec::with_capacity(req.operations.len());
    let mut violations = Vec::new();
    for op in &req.operations {
        let id = Uuid::now_v7();
        if let Some(local) = op.defines() {
            ids.insert(local, id);
        }
        match op {
            Operation::CreateDomain { slug, .. } => match Slug::parse(slug) {
                Ok(slug) => planned.push(Planned::Domain { id, slug }),
                Err(v) => violations.push(v),
            },
            Operation::CreatePrefix {
                domain,
                prefix,
                nature,
                point_to_point,
                ..
            } => match Prefix::parse_any(&ALL, prefix) {
                Ok(value) => {
                    let object = PrefixObject {
                        id,
                        domain: resolve(*domain, &ids),
                        value,
                        nature: *nature,
                        point_to_point: *point_to_point,
                    };
                    violations.extend(objects::check_prefix(&object));
                    let public =
                        special_registry(value.family()).is_none_or(|r| r.is_public(value));
                    planned.push(Planned::Prefix { object, public });
                }
                Err(e) => violations.push(addr_violation(&e)),
            },
            Operation::CreateAddress {
                domain,
                address,
                usage,
                ..
            } => match Addr::parse_any(&ALL, address) {
                Ok(value) => {
                    let object = Address {
                        id,
                        domain: resolve(*domain, &ids),
                        value,
                        usage: *usage,
                    };
                    violations.extend(objects::check_address(&object));
                    let host = Prefix::host(value);
                    let public = special_registry(value.family()).is_none_or(|r| r.is_public(host));
                    planned.push(Planned::Address { object, public });
                }
                Err(e) => violations.push(addr_violation(&e)),
            },
            Operation::Close {
                kind,
                id: target,
                expected,
            } => {
                planned.push(Planned::Close {
                    kind: *kind,
                    id: resolve(*target, &ids),
                    expected: *expected,
                });
            }
        }
    }
    (planned, violations)
}

fn has_errors(violations: &[Violation]) -> bool {
    violations
        .iter()
        .any(|v| v.rule.severity == Severity::Error)
}

/// Domains to lock and whether public space is written (ADR 0015).
async fn lock_targets<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
) -> Result<(Vec<Uuid>, bool), StoreError> {
    let mut domains = BTreeSet::new();
    let mut public = false;
    for p in planned {
        match p {
            Planned::Prefix {
                object,
                public: is_public,
            } => {
                domains.insert(object.domain);
                public |= is_public;
            }
            Planned::Address {
                object,
                public: is_public,
            } => {
                domains.insert(object.domain);
                public |= is_public;
            }
            Planned::Close {
                kind: kind @ (ObjectKind::Prefix | ObjectKind::Address),
                id,
                ..
            } => {
                if let Some(d) = tx.domain_of(*kind, *id).await? {
                    domains.insert(d);
                }
            }
            Planned::Domain { .. } | Planned::Close { .. } => {}
        }
    }
    Ok((domains.into_iter().collect(), public))
}

/// Writes the planned operations in order, collecting every violation (REQ-004-7).
async fn apply<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
    period: Option<Period>,
    changeset: Uuid,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
    for p in planned {
        let written = match p {
            Planned::Domain { id, slug } => {
                tx.insert_domain(*id, slug.as_str(), period, changeset)
                    .await
            }
            Planned::Prefix { object, public } => {
                tx.insert_prefix(object, *public, period, changeset).await
            }
            Planned::Address { object, public } => {
                tx.insert_address(object, *public, period, changeset).await
            }
            Planned::Close { kind, id, expected } => {
                if let Some(expected) = expected
                    && tx.current_changeset(*kind, *id, period).await? != Some(*expected)
                {
                    violations.push(
                        Violation::new(
                            &rules::CHANGESET_CONFLICT,
                            format!("{id} was modified since changeset {expected}"),
                        )
                        .with_subjects([*id]),
                    );
                    continue;
                }
                tx.close(*kind, *id, period, changeset).await
            }
        };
        match written {
            Ok(()) => {}
            Err(WriteError::Rule(v)) => violations.push(v),
            Err(WriteError::NoVersion(id)) => violations.push(
                Violation::new(
                    &rules::CHANGESET_NO_VERSION,
                    format!("{id} does not exist at the targeted time"),
                )
                .with_subjects([id]),
            ),
            Err(WriteError::Store(e)) => return Err(e),
        }
    }
    Ok(())
}

/// Submits a changeset (REQ-004-1 to 12): checks the request, honours its idempotency key,
/// plans and validates the operations locally, then applies them atomically under domain
/// locks; the result is journaled whether applied or refused.
///
/// # Errors
///
/// [`RequestError`] for a malformed request, a reused idempotency key or an
/// infrastructure failure.
pub async fn submit<S: Store>(
    store: &S,
    req: &ChangesetRequest,
    max_operations: usize,
) -> Result<Outcome, RequestError> {
    let period = check_request(req, max_operations)?;
    let changeset = Uuid::now_v7();
    let mut tx = store.begin().await?;
    if req.idempotency_key.is_some() && !req.simulate {
        match tx.replay(req).await? {
            Replay::Fresh => {}
            Replay::Same { changeset, state } => {
                tx.rollback().await?;
                return Ok(Outcome::Replayed { changeset, state });
            }
            Replay::Conflict => {
                tx.rollback().await?;
                return Err(RequestError::IdempotencyConflict);
            }
        }
    }
    let (planned, mut violations) = plan(req);
    if has_errors(&violations) {
        tx.rollback().await?;
        return refuse(store, req, changeset, violations).await;
    }
    let (domains, public) = lock_targets(&mut tx, &planned).await?;
    tx.lock(&domains, public).await?;
    if let Err(e) = apply(&mut tx, &planned, period, changeset, &mut violations).await {
        tx.rollback().await?;
        return Err(e.into());
    }
    if has_errors(&violations) {
        tx.rollback().await?;
        return refuse(store, req, changeset, violations).await;
    }
    if req.simulate {
        tx.rollback().await?;
        return Ok(Outcome::Simulated { violations });
    }
    let journal = Journal {
        id: changeset,
        request: req,
        state: "applied",
        violations: &violations,
    };
    tx.journal_applied(&journal).await?;
    tx.commit().await?;
    Ok(Outcome::Applied {
        changeset,
        warnings: violations,
    })
}

async fn refuse<S: Store>(
    store: &S,
    req: &ChangesetRequest,
    changeset: Uuid,
    violations: Vec<Violation>,
) -> Result<Outcome, RequestError> {
    if req.simulate {
        return Ok(Outcome::Simulated { violations });
    }
    let journal = Journal {
        id: changeset,
        request: req,
        state: "refused",
        violations: &violations,
    };
    store.journal_refused(&journal).await?;
    Ok(Outcome::Refused {
        changeset,
        violations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kartenn_core::objects::{AddressUsage, Nature};

    fn req(operations: Vec<Operation>) -> ChangesetRequest {
        ChangesetRequest {
            author: Uuid::nil(),
            motive: "test".into(),
            period: None,
            operations,
            simulate: false,
            idempotency_key: None,
        }
    }

    fn domain(local: u32) -> Operation {
        Operation::CreateDomain {
            local: Some(local),
            slug: "prod".into(),
        }
    }

    fn prefix(domain: Ref, prefix: &str) -> Operation {
        Operation::CreatePrefix {
            local: None,
            domain,
            prefix: prefix.into(),
            nature: Nature::Network,
            point_to_point: false,
        }
    }

    #[test]
    fn request_shape() {
        assert!(matches!(
            check_request(&req(vec![]), 10),
            Err(RequestError::Empty)
        ));
        let mut r = req(vec![domain(1)]);
        r.motive = "  ".into();
        assert!(matches!(
            check_request(&r, 10),
            Err(RequestError::MotiveRequired)
        ));
        assert!(matches!(
            check_request(&req(vec![domain(1), domain(2)]), 1),
            Err(RequestError::TooManyOperations { count: 2, max: 1 })
        ));
        let mut r = req(vec![domain(1)]);
        r.period = Some(PeriodSpec {
            start: 10,
            end: Some(10),
        });
        assert!(matches!(
            check_request(&r, 10),
            Err(RequestError::InvalidPeriod)
        ));
    }

    #[test]
    fn local_references() {
        // REQ-004-4: defined before use, once, with the right kind.
        assert!(
            check_request(
                &req(vec![domain(1), prefix(Ref::Local(1), "10.0.0.0/8")]),
                10
            )
            .is_ok()
        );
        assert!(matches!(
            check_request(
                &req(vec![prefix(Ref::Local(1), "10.0.0.0/8"), domain(1)]),
                10
            ),
            Err(RequestError::UnknownReference {
                operation: 0,
                local: 1
            })
        ));
        assert!(matches!(
            check_request(&req(vec![domain(1), domain(1)]), 10),
            Err(RequestError::DuplicateLocal(1))
        ));
        let p = Operation::CreatePrefix {
            local: Some(2),
            domain: Ref::Local(1),
            prefix: "10.0.0.0/8".into(),
            nature: Nature::Container,
            point_to_point: false,
        };
        let a = Operation::CreateAddress {
            local: None,
            domain: Ref::Local(2),
            address: "10.0.0.1".into(),
            usage: AddressUsage::Host,
        };
        assert!(matches!(
            check_request(&req(vec![domain(1), p, a]), 10),
            Err(RequestError::WrongReferenceKind {
                local: 2,
                expected: ObjectKind::Domain,
                ..
            })
        ));
    }

    #[test]
    fn planning_collects_every_local_violation() {
        // REQ-004-7: all violations, not only the first.
        let d = Ref::Id(Uuid::now_v7());
        let (planned, violations) = plan(&req(vec![
            prefix(d, "10.1.2.5/24"),
            prefix(d, "fe80::%2/64"),
            Operation::CreateAddress {
                local: None,
                domain: d,
                address: "127.0.0.1".into(),
                usage: AddressUsage::Host,
            },
            Operation::CreateDomain {
                local: None,
                slug: "Bad_Slug".into(),
            },
            prefix(d, "10.1.2.0/24"),
        ]));
        let ids: Vec<&str> = violations.iter().map(|v| v.rule.id).collect();
        assert_eq!(
            ids,
            vec![
                "core.prefix.unaligned",
                "core.addr.zone",
                "core.address.not_assignable",
                "core.slug.syntax"
            ]
        );
        assert_eq!(
            planned.len(),
            2,
            "the valid prefix and the parsed address are planned"
        );
    }

    #[test]
    fn public_flag_comes_from_the_registry() {
        let d = Ref::Id(Uuid::now_v7());
        let (planned, _) = plan(&req(vec![
            prefix(d, "20.0.0.0/24"),
            prefix(d, "10.0.0.0/24"),
        ]));
        let flags: Vec<bool> = planned
            .iter()
            .map(|p| matches!(p, Planned::Prefix { public: true, .. }))
            .collect();
        assert_eq!(flags, vec![true, false]);
    }
}
