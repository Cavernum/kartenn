//! The changeset pipeline (004/design, § Pipeline d'un changeset).

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use kartenn_core::addr::{Addr, AddrError, Prefix};
use kartenn_core::family::ALL;
use kartenn_core::objects::{self, Address, IpRange, PrefixObject, RangeUsage, Slug};
use kartenn_core::period::{Instant, Period};
use kartenn_core::properties::Declaration;
use kartenn_core::registry::special_registry;
use kartenn_core::rules::{self, Severity, Violation};
use kartenn_core::values::Raw;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::RequestContext;
use crate::authz::Authorizer;
use crate::expected::Sources;
use crate::module::{ModuleHandler, ModulePlan, ModuleRegistry};
use crate::operations::{ChangesetRequest, ModuleKind, ObjectKind, Operation, PeriodSpec, Ref};
use crate::port::{
    ApprovalPolicy, Change, DiffCount, DiffItem, Journal, Pending, Replay, Store, StoreError,
    WriteError, WriteTx,
};

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
    /// An operation of a module the build lacks, or that its module does not have
    /// (REQ-008-4).
    #[error("operation {operation}: `{op}` is not an operation of this build")]
    UnknownOperation {
        /// Index of the operation.
        operation: usize,
        /// Its tag.
        op: String,
    },
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
    /// The validation, rejection or withdrawal of a pending changeset is refused; the
    /// changeset keeps its state (REQ-004-21, 23, 24, 42).
    #[error("{}: {}", .0.rule.id, .0.message)]
    Review(Violation),
    /// The idempotency key was already used with a different content (REQ-004-10).
    #[error("the idempotency key was already used for a different changeset")]
    IdempotencyConflict,
    /// The idempotency key is held by a submission in progress (006/REQ-006-15).
    #[error("a submission with this idempotency key is in progress")]
    IdempotencyInProgress,
    /// A filter that does not parse, or a duration out of bounds (REQ-012-27).
    #[error("invalid filter or duration")]
    InvalidOverride,
    /// The actor lacks this permission (refusal audited).
    #[error("permission {0} is required")]
    Forbidden(String),
    /// The audit journal could not record the action, which is therefore not done
    /// (REQ-012-11).
    #[error("the audit journal is unavailable: {0}")]
    Audit(StoreError),
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
        /// Differential of resolved values, counted; the detail is stored (REQ-003-19).
        diff: Vec<DiffCount>,
        /// Closures added by a cascade (REQ-004-26).
        cascaded: Vec<(ObjectKind, Uuid)>,
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
    /// Held for a decision, as a proposal or by an approval policy (REQ-004-20, 41);
    /// nothing written but the journal.
    Pending {
        /// Changeset identifier.
        changeset: Uuid,
        /// Proposal or approval.
        kind: PendingKind,
        /// The policy that holds it.
        policy: Option<Uuid>,
        /// Differential that applying it would produce.
        diff: Vec<DiffCount>,
    },
    /// A pending changeset validated but refused on the current state (REQ-004-22).
    FailedRevalidation {
        /// Changeset identifier.
        changeset: Uuid,
        /// All violations found.
        violations: Vec<Violation>,
    },
    /// Simulation: what would happen, nothing written (REQ-004-9).
    Simulated {
        /// All violations found (errors and warnings).
        violations: Vec<Violation>,
        /// Differential that applying would produce (REQ-003-20).
        diff: Vec<DiffCount>,
        /// Its first entries.
        sample: Vec<DiffItem>,
        /// Closures a cascade would add (REQ-004-26).
        cascaded: Vec<(ObjectKind, Uuid)>,
        /// The sections the modules would contribute, each with its first rows
        /// (REQ-008-26).
        sections: Vec<SectionSample>,
    },
}

/// A section of the differential in a simulation: its size and its first rows.
#[derive(Debug, Clone, PartialEq)]
pub struct SectionSample {
    /// Its name, `<module>.<name>`.
    pub name: String,
    /// Its rows.
    pub count: u64,
    /// Its first rows.
    pub rows: Vec<crate::module::DiffRow>,
}

impl SectionSample {
    fn of(section: crate::module::DiffSection) -> Self {
        let count = u64::try_from(section.rows.len()).unwrap_or(u64::MAX);
        let mut rows = section.rows;
        rows.truncate(usize::try_from(SIMULATION_SAMPLE).unwrap_or(usize::MAX));
        Self {
            name: section.name,
            count,
            rows,
        }
    }
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
#[derive(Debug, Clone)]
pub(crate) enum Planned {
    Domain {
        id: Uuid,
        slug: Slug,
    },
    Prefix {
        object: PrefixObject,
        public: bool,
    },
    Range {
        object: IpRange,
        public: bool,
    },
    Address {
        object: Address,
        public: bool,
    },
    /// An object or one of its assignments restored as before a changeset (REQ-004-34).
    Restore {
        kind: ObjectKind,
        id: Uuid,
        property: Option<Uuid>,
        before: Uuid,
    },
    /// An identifier of an object in a source.
    BindExternal {
        object: Uuid,
        kind: ObjectKind,
        source: String,
        external_id: String,
    },
    /// An approval policy; its identifier is the existing one of the same name, if any.
    Policy(ApprovalPolicy),
    /// An administration operation of the RBAC (005).
    Rbac(crate::rbac::RbacOp),
    /// An entity or a site.
    Anchor {
        kind: ObjectKind,
        id: Uuid,
        slug: Slug,
        parent: Option<Uuid>,
    },
    Close {
        kind: ObjectKind,
        id: Uuid,
        expected: Option<Uuid>,
        cascade: bool,
    },
    /// A guard on the version of an object: no write.
    Expect {
        kind: ObjectKind,
        id: Uuid,
        expected: Uuid,
    },
    Modify {
        kind: ObjectKind,
        id: Uuid,
        change: Change,
        expected: Option<Uuid>,
    },
    Declare {
        id: Uuid,
        declaration: Declaration,
    },
    UpdateDeclaration {
        id: Uuid,
        declaration: Declaration,
    },
    EnumValue {
        id: Uuid,
        property: Uuid,
        key: Slug,
    },
    /// `values` `None`: explicit "no value".
    Assign {
        object: Uuid,
        kind: ObjectKind,
        property: Uuid,
        values: Option<Vec<Raw>>,
    },
    Unassign {
        object: Uuid,
        kind: ObjectKind,
        property: Uuid,
    },
    /// An operation of a module, planned by its handler (REQ-008-4).
    Module {
        handler: Arc<dyn ModuleHandler>,
        plan: Arc<ModulePlan>,
    },
}

impl Planned {
    /// The object the operation creates or modifies.
    pub(crate) fn object(&self) -> Uuid {
        match self {
            Self::Module { plan, .. } => plan.object,
            Self::Domain { id, .. }
            | Self::Anchor { id, .. }
            | Self::Restore { id, .. }
            | Self::Modify { id, .. }
            | Self::Close { id, .. }
            | Self::Expect { id, .. }
            | Self::Declare { id, .. }
            | Self::UpdateDeclaration { id, .. }
            | Self::EnumValue { id, .. } => *id,
            Self::Assign { object, .. }
            | Self::Unassign { object, .. }
            | Self::BindExternal { object, .. } => *object,
            Self::Policy(policy) => policy.id,
            Self::Rbac(op) => op.object(),
            Self::Prefix { object, .. } => object.id,
            Self::Range { object, .. } => object.id,
            Self::Address { object, .. } => object.id,
        }
    }
}

/// Objects an operation designates, with the kind each must have (REQ-004-4).
pub(crate) fn references(op: &Operation) -> Vec<(Ref, ObjectKind)> {
    match op {
        Operation::CreatePrefix { domain, .. }
        | Operation::CreateRange { domain, .. }
        | Operation::CreateAddress { domain, .. } => {
            vec![(*domain, ObjectKind::Domain)]
        }
        Operation::BindExternal { object, kind, .. } => vec![(*object, *kind)],
        Operation::Close { kind, id, .. }
        | Operation::Expect { kind, id, .. }
        | Operation::Restore { kind, id, .. } => {
            vec![(*id, *kind)]
        }
        Operation::UpdatePrefix { id, .. } => vec![(*id, ObjectKind::Prefix)],
        Operation::UpdateAddress { id, .. } => vec![(*id, ObjectKind::Address)],
        Operation::UpdateRange { id, .. } => vec![(*id, ObjectKind::Range)],
        Operation::UpdateProperty { id, .. } => vec![(*id, ObjectKind::Property)],
        Operation::AssignProperty {
            object,
            kind,
            property,
            ..
        }
        | Operation::ClearProperty {
            object,
            kind,
            property,
        }
        | Operation::UnassignProperty {
            object,
            kind,
            property,
        } => vec![(*object, *kind), (*property, ObjectKind::Property)],
        Operation::AddEnumValue { property, .. }
        | Operation::SetSensitivity { property, .. }
        | Operation::AddObligation { property, .. }
        | Operation::RemoveObligation { property, .. } => {
            vec![(*property, ObjectKind::Property)]
        }
        Operation::CreateEntity { parent, .. } => {
            parent.iter().map(|p| (*p, ObjectKind::Entity)).collect()
        }
        Operation::CreateSite { parent, .. } => {
            parent.iter().map(|p| (*p, ObjectKind::Site)).collect()
        }
        Operation::MoveEntity { id, parent, .. } => std::iter::once(id)
            .chain(parent)
            .map(|r| (*r, ObjectKind::Entity))
            .collect(),
        Operation::MoveSite { id, parent, .. } => std::iter::once(id)
            .chain(parent)
            .map(|r| (*r, ObjectKind::Site))
            .collect(),
        Operation::Rename { kind, id, .. } => vec![(*id, kind.kind())],
        Operation::Bind { scope, .. } => match scope {
            crate::operations::BindingScope::Global => vec![],
            crate::operations::BindingScope::Entity { id } => vec![(*id, ObjectKind::Entity)],
            crate::operations::BindingScope::Site { id } => vec![(*id, ObjectKind::Site)],
            crate::operations::BindingScope::Domain { id } => vec![(*id, ObjectKind::Domain)],
            crate::operations::BindingScope::Prefix { id } => vec![(*id, ObjectKind::Prefix)],
        },
        Operation::AttachIssuer { entity, .. } | Operation::DetachIssuer { entity, .. } => {
            entity.iter().map(|r| (*r, ObjectKind::Entity)).collect()
        }
        Operation::CreateDomain { .. }
        | Operation::DeclareProperty { .. }
        | Operation::SetApprovalPolicy { .. }
        | Operation::SetRole { .. }
        | Operation::CloseRole { .. }
        | Operation::Unbind { .. }
        | Operation::SetIdentityState { .. }
        | Operation::CreateServiceAccount { .. }
        | Operation::AddServiceSecret { .. }
        | Operation::RevokeServiceSecret { .. }
        // A module resolves its own references when it plans (`ModuleHandler::plan`).
        | Operation::Module(_)
        | Operation::Within { .. } => vec![],
    }
}
/// Checks the shape of the request (REQ-004-1 to 5): size, motive, period, references,
/// and that each operation of a module is one of the build (REQ-008-4).
fn check_request(
    req: &ChangesetRequest,
    max: usize,
    modules: &ModuleRegistry,
) -> Result<Option<Period>, RequestError> {
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
        to_period(op.own_period())?;
        let op = op.inner();
        let uses = references(op);
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
        let defined_module = match op {
            Operation::Module(m) => {
                let Some((operation, _)) = modules.operation(&m.module, &m.name) else {
                    return Err(RequestError::UnknownOperation {
                        operation: i,
                        op: m.tag(),
                    });
                };
                operation.defines.and_then(ModuleKind::parse)
            }
            _ => None,
        };
        if let Some(local) = op.defines() {
            let kind = match op {
                Operation::Module(_) => {
                    defined_module.map_or(ObjectKind::Address, ObjectKind::Module)
                }
                Operation::CreateDomain { .. } => ObjectKind::Domain,
                Operation::CreatePrefix { .. } => ObjectKind::Prefix,
                Operation::CreateRange { .. } => ObjectKind::Range,
                Operation::DeclareProperty { .. } => ObjectKind::Property,
                Operation::AddEnumValue { .. } => ObjectKind::EnumValue,
                Operation::CreateEntity { .. } => ObjectKind::Entity,
                Operation::CreateSite { .. } => ObjectKind::Site,
                _ => ObjectKind::Address,
            };
            if defined.insert(local, kind).is_some() {
                return Err(RequestError::DuplicateLocal(local));
            }
        }
    }
    Ok(period)
}

/// Parses and checks a range locally (REQ-002-21 bounds, 31, 35): the range is planned
/// unless a violation is an error; warnings are returned either way.
fn plan_range(
    id: Uuid,
    domain: Uuid,
    first: &str,
    last: &str,
    usage: RangeUsage,
) -> (Option<Planned>, Vec<Violation>) {
    let (first, last) = match (Addr::parse_any(&ALL, first), Addr::parse_any(&ALL, last)) {
        (Ok(first), Ok(last)) => (first, last),
        (first, last) => {
            let violations = [first.err(), last.err()]
                .iter()
                .flatten()
                .map(addr_violation)
                .collect();
            return (None, violations);
        }
    };
    let object = IpRange {
        id,
        domain,
        first,
        last,
        usage,
    };
    let violations = objects::check_range(&object);
    if has_errors(&violations) {
        return (None, violations);
    }
    // Classified by its bounds; a valid range lies in one network, hence in one block of
    // the special-purpose registry or none.
    let public = [first, last]
        .into_iter()
        .any(|a| special_registry(a.family()).is_none_or(|r| r.is_public(Prefix::host(a))));
    (Some(Planned::Range { object, public }), violations)
}

/// Plans a modification of an addressing object (004-8); a new prefix value is parsed and
/// classified like a creation.
fn plan_update(op: &Operation, id: Uuid) -> Result<Option<Planned>, Violation> {
    // REQ-002-12: an object never changes domain.
    if let Operation::UpdatePrefix {
        domain: Some(_), ..
    }
    | Operation::UpdateAddress {
        domain: Some(_), ..
    }
    | Operation::UpdateRange {
        domain: Some(_), ..
    } = op
    {
        return Err(Violation::new(
            &rules::DOMAIN_IMMUTABLE,
            format!("{id} cannot change domain: close it and create it in the other domain"),
        )
        .with_subjects([id]));
    }
    let (kind, change, expected) = match op {
        Operation::UpdatePrefix {
            prefix,
            nature,
            point_to_point,
            anycast,
            expected,
            ..
        } => {
            let value = match prefix.as_deref().map(|p| Prefix::parse_any(&ALL, p)) {
                None => None,
                Some(Ok(value)) => Some((
                    value,
                    special_registry(value.family()).is_none_or(|r| r.is_public(value)),
                )),
                Some(Err(e)) => return Err(addr_violation(&e)),
            };
            let change = Change::Prefix {
                value,
                nature: *nature,
                point_to_point: *point_to_point,
                anycast: *anycast,
            };
            (ObjectKind::Prefix, change, *expected)
        }
        Operation::UpdateAddress {
            usage, expected, ..
        } => (ObjectKind::Address, Change::Address(*usage), *expected),
        Operation::UpdateRange {
            usage, expected, ..
        } => (ObjectKind::Range, Change::Range(*usage), *expected),
        _ => return Ok(None),
    };
    Ok(Some(Planned::Modify {
        kind,
        id,
        change,
        expected,
    }))
}

/// Parses and checks a new prefix or address locally, and classifies it in the public
/// space.
fn plan_prefix(
    op: &Operation,
    id: Uuid,
    domain: Uuid,
) -> Result<(Option<Planned>, Vec<Violation>), Violation> {
    if let Operation::CreateAddress { address, usage, .. } = op {
        let value = Addr::parse_any(&ALL, address).map_err(|e| addr_violation(&e))?;
        let object = Address {
            id,
            domain,
            value,
            usage: *usage,
        };
        let violations = objects::check_address(&object);
        let host = Prefix::host(value);
        let public = special_registry(value.family()).is_none_or(|r| r.is_public(host));
        return Ok((Some(Planned::Address { object, public }), violations));
    }
    let Operation::CreatePrefix {
        prefix,
        nature,
        point_to_point,
        anycast,
        ..
    } = op
    else {
        return Ok((None, Vec::new()));
    };
    let value = Prefix::parse_any(&ALL, prefix).map_err(|e| addr_violation(&e))?;
    let object = PrefixObject {
        id,
        domain,
        value,
        nature: *nature,
        point_to_point: *point_to_point,
        anycast: *anycast,
    };
    let violations = objects::check_prefix(&object);
    let public = special_registry(value.family()).is_none_or(|r| r.is_public(value));
    Ok((Some(Planned::Prefix { object, public }), violations))
}

/// Plans the creation or the move of an entity or a site, an approval policy, or the
/// modification of an addressing object.
fn plan_anchor(
    op: &Operation,
    id: Uuid,
    resolve: impl Fn(Ref) -> Uuid,
) -> Result<Option<Planned>, Violation> {
    Ok(Some(match op {
        Operation::CreateEntity { slug, parent, .. }
        | Operation::CreateSite { slug, parent, .. } => Planned::Anchor {
            kind: if matches!(op, Operation::CreateEntity { .. }) {
                ObjectKind::Entity
            } else {
                ObjectKind::Site
            },
            id,
            slug: Slug::parse(slug)?,
            parent: parent.map(&resolve),
        },
        Operation::MoveEntity {
            id: target,
            parent,
            expected,
        }
        | Operation::MoveSite {
            id: target,
            parent,
            expected,
        } => {
            let kind = if matches!(op, Operation::MoveEntity { .. }) {
                ObjectKind::Entity
            } else {
                ObjectKind::Site
            };
            let (id, parent) = (resolve(*target), parent.map(&resolve));
            // The shortest cycle, refused before any write.
            if parent == Some(id) {
                return Err(Violation::new(
                    &rules::ANCHOR_CYCLE,
                    format!("{} {id} cannot be its own parent", kind.name()),
                )
                .with_subjects([id]));
            }
            Planned::Modify {
                kind,
                id,
                change: Change::Parent(parent),
                expected: *expected,
            }
        }
        Operation::Rename {
            kind,
            id: target,
            slug,
            expected,
        } => Planned::Modify {
            kind: kind.kind(),
            id: resolve(*target),
            change: Change::Slug(Slug::parse(slug)?),
            expected: *expected,
        },
        Operation::SetSensitivity {
            property,
            sensitive,
        } => Planned::Modify {
            kind: ObjectKind::Property,
            id: resolve(*property),
            change: Change::Sensitive(*sensitive),
            expected: None,
        },
        Operation::AddObligation { property, scope }
        | Operation::RemoveObligation { property, scope } => Planned::Modify {
            kind: ObjectKind::Property,
            id: resolve(*property),
            change: Change::Obligation {
                scope: crate::declarations::scope(scope),
                required: matches!(op, Operation::AddObligation { .. }),
            },
            expected: None,
        },
        Operation::SetApprovalPolicy { .. } => plan_policy(op, id, &resolve)?,
        Operation::BindExternal {
            object,
            kind,
            source,
            external_id,
        } => Planned::BindExternal {
            object: resolve(*object),
            kind: *kind,
            source: source.clone(),
            external_id: external_id.clone(),
        },
        Operation::Restore {
            kind,
            id: target,
            property,
            before,
        } => Planned::Restore {
            kind: *kind,
            id: resolve(*target),
            property: *property,
            before: *before,
        },
        Operation::UpdatePrefix { id: target, .. }
        | Operation::UpdateAddress { id: target, .. }
        | Operation::UpdateRange { id: target, .. } => return plan_update(op, resolve(*target)),
        _ => return Ok(None),
    }))
}

/// Plans an approval policy: a name and at least one trigger (REQ-004-40).
fn plan_policy(
    op: &Operation,
    id: Uuid,
    resolve: &impl Fn(Ref) -> Uuid,
) -> Result<Planned, Violation> {
    let Operation::SetApprovalPolicy {
        name,
        min_operations,
        min_diff_objects,
        property_keys,
        domains,
        authors,
        scopes,
        author_roles,
        author_groups,
        bound_roles,
        module_triggers,
        enabled,
    } = op
    else {
        return Err(Violation::new(
            &rules::POLICY_INVALID,
            "not a policy".to_owned(),
        ));
    };
    let policy = ApprovalPolicy {
        id,
        name: name.trim().to_owned(),
        min_operations: *min_operations,
        min_diff_objects: *min_diff_objects,
        property_keys: property_keys.clone(),
        domains: domains.clone(),
        authors: authors.clone(),
        scopes: Vec::new(),
        author_roles: author_roles.clone(),
        author_groups: author_groups.clone(),
        bound_roles: bound_roles.iter().map(|r| r.trim().to_owned()).collect(),
        module_triggers: module_triggers
            .iter()
            .map(|t| (t.measure.clone(), t.threshold))
            .collect(),
        enabled: *enabled,
    };
    let mut policy = policy;
    for scope in scopes {
        match scope {
            crate::operations::BindingScope::Entity { id } => {
                policy.scopes.push(("entity".into(), resolve(*id)));
            }
            crate::operations::BindingScope::Site { id } => {
                policy.scopes.push(("site".into(), resolve(*id)));
            }
            crate::operations::BindingScope::Prefix { id } => {
                policy.scopes.push(("prefix".into(), resolve(*id)));
            }
            crate::operations::BindingScope::Domain { id } => policy.domains.push(resolve(*id)),
            crate::operations::BindingScope::Global => {
                return Err(Violation::new(
                    &rules::POLICY_INVALID,
                    format!(
                        "the policy '{}': every changeset is in the global scope",
                        policy.name
                    ),
                ));
            }
        }
    }
    let triggers = policy.min_operations.is_some()
        || policy.min_diff_objects.is_some()
        || !policy.property_keys.is_empty()
        || !policy.domains.is_empty()
        || !policy.scopes.is_empty()
        || !policy.authors.is_empty()
        || !policy.author_roles.is_empty()
        || !policy.author_groups.is_empty()
        || !policy.bound_roles.is_empty()
        || !policy.module_triggers.is_empty();
    if policy.name.is_empty() || !triggers {
        return Err(Violation::new(
            &rules::POLICY_INVALID,
            format!(
                "the policy '{}' needs a name and at least one trigger",
                policy.name
            ),
        ));
    }
    Ok(Planned::Policy(policy))
}

/// Lets each operation of a module complete itself on the state of the transaction
/// ([`ModuleHandler::prepare`]); the request itself when it has none.
async fn prepare<'r, T: WriteTx>(
    tx: &mut T,
    req: &'r ChangesetRequest,
    modules: &ModuleRegistry,
    period: Option<Period>,
) -> Result<std::borrow::Cow<'r, ChangesetRequest>, StoreError> {
    if !req
        .operations
        .iter()
        .any(|op| matches!(op.inner(), Operation::Module(_)))
    {
        return Ok(std::borrow::Cow::Borrowed(req));
    }
    let mut prepared = req.clone();
    for op in &mut prepared.operations {
        let own = to_period(op.own_period()).ok().flatten().or(period);
        if let Operation::Module(operation) = op.inner_mut()
            && let Some((_, handler)) = modules.operation(&operation.module, &operation.name)
        {
            handler.prepare(tx.module_tx(), operation, own).await?;
        }
    }
    Ok(std::borrow::Cow::Owned(prepared))
}

/// Parses every operation and runs the local rules (no database): the planned writes, the
/// index of the operation each comes from, and the violations found, each ascribed to its
/// operation.
#[allow(clippy::too_many_lines)] // one arm per kind of operation
fn plan(
    req: &ChangesetRequest,
    modules: &ModuleRegistry,
) -> (Vec<Planned>, Vec<usize>, Vec<Violation>) {
    let mut ids: HashMap<u32, Uuid> = HashMap::new();
    let resolve = |r: Ref, ids: &HashMap<u32, Uuid>| match r {
        Ref::Id(id) => id,
        Ref::Local(n) => ids[&n],
    };
    let mut planned = Vec::with_capacity(req.operations.len());
    let mut origins = Vec::with_capacity(req.operations.len());
    let mut violations = Vec::new();
    for (index, requested) in req.operations.iter().enumerate() {
        let found = violations.len();
        // The operation, then the core operations a module operation adds (ADR 0026,
        // clause 6), planned in turn under the same index.
        let mut queue = std::collections::VecDeque::from([requested.clone()]);
        while let Some(owned) = queue.pop_front() {
            let id = Uuid::now_v7();
            if let Some(local) = owned.defines() {
                ids.insert(local, id);
            }
            let op = owned.inner();
            // REQ-016-7: a policy names the measures of the modules of the build only.
            if let Operation::SetApprovalPolicy {
                name,
                module_triggers,
                ..
            } = op
            {
                for trigger in module_triggers {
                    if !modules.declares_measure(&trigger.measure) {
                        violations.push(Violation::new(
                            &rules::POLICY_TRIGGER,
                            format!(
                                "the policy '{name}': no module of the build measures '{}'",
                                trigger.measure
                            ),
                        ));
                    }
                }
            }
            match op {
                Operation::Module(operation) => {
                    let Some((_, handler)) = modules.operation(&operation.module, &operation.name)
                    else {
                        // Refused by `check_request` already.
                        continue;
                    };
                    let found = handler.plan(operation, id, &|r| match r {
                        Ref::Id(id) => Some(id),
                        Ref::Local(n) => ids.get(&n).copied(),
                    });
                    match found {
                        Ok(module_plan) => {
                            queue.extend(module_plan.core.iter().cloned());
                            planned.push(Planned::Module {
                                handler,
                                plan: Arc::new(module_plan),
                            });
                        }
                        Err(v) => violations.push(v),
                    }
                }
                Operation::CreateDomain { slug, .. } => match Slug::parse(slug) {
                    Ok(slug) => planned.push(Planned::Domain { id, slug }),
                    Err(v) => violations.push(v),
                },
                Operation::CreatePrefix { domain, .. }
                | Operation::CreateAddress { domain, .. } => {
                    match plan_prefix(op, id, resolve(*domain, &ids)) {
                        Ok((p, found)) => {
                            planned.extend(p);
                            violations.extend(found);
                        }
                        Err(v) => violations.push(v),
                    }
                }
                Operation::CreateRange {
                    domain,
                    first,
                    last,
                    usage,
                    ..
                } => {
                    let domain = resolve(*domain, &ids);
                    let (range, found) = plan_range(id, domain, first, last, *usage);
                    planned.extend(range);
                    violations.extend(found);
                }
                Operation::CreateEntity { .. }
                | Operation::CreateSite { .. }
                | Operation::MoveEntity { .. }
                | Operation::MoveSite { .. }
                | Operation::Rename { .. }
                | Operation::SetSensitivity { .. }
                | Operation::AddObligation { .. }
                | Operation::RemoveObligation { .. }
                | Operation::SetApprovalPolicy { .. }
                | Operation::Restore { .. }
                | Operation::BindExternal { .. }
                | Operation::UpdatePrefix { .. }
                | Operation::UpdateAddress { .. }
                | Operation::UpdateRange { .. } => {
                    match plan_anchor(op, id, |r| resolve(r, &ids)) {
                        Ok(p) => planned.extend(p),
                        Err(v) => violations.push(v),
                    }
                }
                Operation::Close {
                    kind,
                    id: target,
                    expected,
                    cascade,
                } => {
                    planned.push(Planned::Close {
                        kind: *kind,
                        id: resolve(*target, &ids),
                        expected: *expected,
                        cascade: *cascade,
                    });
                }
                Operation::Expect {
                    kind,
                    id: target,
                    expected,
                } => planned.push(Planned::Expect {
                    kind: *kind,
                    id: resolve(*target, &ids),
                    expected: *expected,
                }),
                Operation::DeclareProperty { .. }
                | Operation::UpdateProperty { .. }
                | Operation::AddEnumValue { .. } => {
                    match crate::declarations::plan(op, id, |r| resolve(r, &ids), modules) {
                        Ok(p) => planned.push(p),
                        Err(v) => violations.push(v),
                    }
                }
                Operation::SetRole { .. }
                | Operation::CloseRole { .. }
                | Operation::Bind { .. }
                | Operation::Unbind { .. }
                | Operation::AttachIssuer { .. }
                | Operation::DetachIssuer { .. }
                | Operation::SetIdentityState { .. }
                | Operation::CreateServiceAccount { .. }
                | Operation::AddServiceSecret { .. }
                | Operation::RevokeServiceSecret { .. } => {
                    match crate::rbac::plan(op, id, |r| resolve(r, &ids)) {
                        Some(Ok(p)) => planned.push(p),
                        Some(Err(v)) => violations.push(v),
                        None => {}
                    }
                }
                // Unwrapped above.
                Operation::Within { .. } => {}
                Operation::AssignProperty { .. }
                | Operation::ClearProperty { .. }
                | Operation::UnassignProperty { .. } => {
                    planned.push(crate::resolution::plan(op, |r| resolve(r, &ids)));
                }
            }
        }
        origins.resize(planned.len(), index);
        ascribe(&mut violations[found..], index);
    }
    (planned, origins, violations)
}

/// Ascribes `violations` to operation `index`, those not ascribed already.
fn ascribe(violations: &mut [Violation], index: usize) {
    for v in violations {
        v.operation.get_or_insert(index);
    }
}

fn has_errors(violations: &[Violation]) -> bool {
    violations
        .iter()
        .any(|v| v.rule.severity == Severity::Error)
}

/// Lock key of the tree of entities or of sites, taken like a domain's (ADR 0015).
fn tree_lock(kind: ObjectKind) -> Uuid {
    let tree = if kind == ObjectKind::Entity { 1 } else { 2 };
    Uuid::from_u64_pair(u64::MAX - tree, 0)
}

/// Domains to lock and whether public space is written (ADR 0015).
#[allow(clippy::too_many_lines)] // one arm per kind of planned operation
async fn lock_targets<T: WriteTx>(
    tx: &mut T,
    planned: &[Planned],
) -> Result<(Vec<Uuid>, bool, Vec<String>), StoreError> {
    let mut domains = BTreeSet::new();
    let mut public = false;
    let mut keys = BTreeSet::new();
    for p in planned {
        if let Planned::Module { plan, .. } = p {
            domains.extend(plan.domains.iter().copied());
            keys.extend(plan.keys.iter().cloned());
            continue;
        }
        match p {
            Planned::Prefix {
                object,
                public: is_public,
            } => {
                domains.insert(object.domain);
                public |= is_public;
            }
            Planned::Range {
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
                kind:
                    kind @ (ObjectKind::Prefix
                    | ObjectKind::Range
                    | ObjectKind::Address
                    | ObjectKind::Module(_)),
                id,
                ..
            } => {
                if let Some(d) = tx.domain_of(*kind, *id).await? {
                    domains.insert(d);
                }
            }
            // The trees of entities and of sites are serialised as a whole (cycles).
            Planned::Anchor { kind, .. }
            | Planned::Modify {
                kind: kind @ (ObjectKind::Entity | ObjectKind::Site),
                ..
            }
            | Planned::Close {
                kind: kind @ (ObjectKind::Entity | ObjectKind::Site),
                ..
            }
            | Planned::Restore {
                kind: kind @ (ObjectKind::Entity | ObjectKind::Site),
                property: None,
                ..
            } => {
                domains.insert(tree_lock(*kind));
            }
            Planned::Restore {
                kind, id, property, ..
            } => {
                if let Some(d) = tx.domain_of(*kind, *id).await? {
                    domains.insert(d);
                }
                public |= property.is_none();
            }
            // A closed or renamed domain: its own lock only.
            Planned::Modify {
                kind: ObjectKind::Domain,
                id,
                ..
            }
            | Planned::Close {
                kind: ObjectKind::Domain,
                id,
                ..
            } => {
                domains.insert(*id);
            }
            // A new domain is invisible to others until commit: nothing to serialise.
            // Declarations are serialised by their constraints (unique key, versions).
            Planned::Domain { .. }
            | Planned::Close {
                kind: ObjectKind::Property | ObjectKind::EnumValue,
                ..
            }
            | Planned::Modify {
                kind: ObjectKind::Property,
                ..
            }
            | Planned::Declare { .. }
            | Planned::UpdateDeclaration { .. }
            | Planned::EnumValue { .. }
            | Planned::BindExternal { .. }
            | Planned::Rbac(_)
            | Planned::Policy(_)
            // A guard locks the version row of its object instead (`WriteTx::version`).
            // A module operation: taken above.
            | Planned::Expect { .. } | Planned::Module { .. } => {}
            // A modified prefix or address may enter or leave public space.
            Planned::Modify { kind, id, .. } => {
                if let Some(d) = tx.domain_of(*kind, *id).await? {
                    domains.insert(d);
                }
                public |= *kind != ObjectKind::Range;
            }
            // An assignment is written under the lock of its object's domain.
            Planned::Assign { object, kind, .. } | Planned::Unassign { object, kind, .. } => {
                if let Some(d) = tx.domain_of(*kind, *object).await? {
                    domains.insert(d);
                }
            }
        }
    }
    Ok((
        domains.into_iter().collect(),
        public,
        keys.into_iter().collect(),
    ))
}

/// Writes the planned operations in order, collecting every violation (REQ-004-7).
/// A conflict when the version of `id` at the start of `period` was not written by
/// `expected` (REQ-004-11).
async fn conflict<T: WriteTx>(
    tx: &mut T,
    id: Uuid,
    expected: Option<Uuid>,
) -> Result<Option<Violation>, StoreError> {
    let Some(expected) = expected else {
        return Ok(None);
    };
    let current = tx.version(id).await?;
    if current == Some(expected) {
        return Ok(None);
    }
    // REQ-004-11: the conflict names the current version.
    let current = current.map_or_else(|| "none".to_owned(), |c| c.to_string());
    Ok(Some(
        Violation::new(
            &rules::CHANGESET_CONFLICT,
            format!("{id} was modified since changeset {expected}: current version {current}"),
        )
        .with_subjects([id]),
    ))
}

/// Writes a policy under the identifier of the existing one of the same name, if any.
async fn write_policy<T: WriteTx>(tx: &mut T, policy: &ApprovalPolicy) -> Result<(), WriteError> {
    let id = tx.policy_named(&policy.name).await?.unwrap_or(policy.id);
    tx.set_policy(&ApprovalPolicy {
        id,
        ..policy.clone()
    })
    .await
}

/// Writes or removes an assignment typed by the user (`input` provenance).
async fn assignment<T: WriteTx>(
    tx: &mut T,
    p: &Planned,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    match p {
        Planned::Assign {
            object,
            kind,
            property,
            values,
        } => {
            let values = values.as_deref();
            crate::resolution::assign(
                tx, *object, *kind, *property, values, "input", period, changeset,
            )
            .await
        }
        Planned::Unassign {
            object,
            kind,
            property,
        } => crate::resolution::unassign(tx, *object, *kind, *property, period, changeset).await,
        _ => Ok(()),
    }
}

/// The rules of each module on the final state, with the objects of the module the
/// changeset wrote and the sections of the differential (ADR 0026, clause 7).
async fn check_modules<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    changeset: Uuid,
    planned: &[Planned],
    touched: &[(ObjectKind, Uuid)],
    sections: &[crate::module::DiffSection],
    violations: &mut Vec<Violation>,
) -> Result<ModuleFindings, StoreError> {
    // Each module once, in the order of the registry: a module that wrote nothing may
    // still have rules on the sections.
    let mut reevaluated = Vec::new();
    let mut measures = std::collections::BTreeMap::new();
    for module in modules.modules() {
        let Some(handler) = module.handler() else {
            continue;
        };
        let mut objects: Vec<(ModuleKind, Uuid)> = Vec::new();
        for p in planned {
            let object = match p {
                Planned::Module { plan, .. } => (plan.kind, plan.object),
                // Closed or restored by the generic operations (REQ-008-5).
                Planned::Close {
                    kind: ObjectKind::Module(kind),
                    id,
                    ..
                }
                | Planned::Restore {
                    kind: ObjectKind::Module(kind),
                    id,
                    property: None,
                    ..
                } => (*kind, *id),
                _ => continue,
            };
            if object.0.module() == module.name() {
                objects.push(object);
            }
        }
        let context = crate::module::CheckContext {
            changeset,
            touched: &objects,
            objects: touched,
            sections,
        };
        let findings = handler.check(tx.module_tx(), &context).await?;
        violations.extend(findings.violations);
        reevaluated.extend(findings.reevaluated);
        // Its measures, for the approval policies (REQ-016-7).
        for (name, value) in handler.measure(tx.module_tx(), &context).await? {
            if module.measures().contains(&name) {
                measures.insert(format!("{}.{name}", module.name()), value);
            } else {
                return Err(StoreError(format!(
                    "module `{}` measures `{name}`, which it does not declare",
                    module.name()
                )));
            }
        }
    }
    Ok(ModuleFindings {
        reevaluated,
        measures,
    })
}

/// What the modules found on a changeset: objects re-evaluated, and their measures.
struct ModuleFindings {
    reevaluated: Vec<(Uuid, &'static str)>,
    measures: std::collections::BTreeMap<String, u64>,
}

/// A write's refusal becomes a violation; an infrastructure failure stops everything.
fn record(
    written: Result<(), WriteError>,
    violations: &mut Vec<Violation>,
) -> Result<(), StoreError> {
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
    Ok(())
}

/// Restores an object or an assignment as before a changeset, noting the former extent
/// of a prefix.
async fn restore<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    (kind, id, property, before): (ObjectKind, Uuid, Option<Uuid>, Uuid),
    period: Option<Period>,
    changeset: Uuid,
    former: &mut Vec<(Uuid, Prefix)>,
) -> Result<(), WriteError> {
    if kind == ObjectKind::Prefix && property.is_none() {
        for (old, _) in tx.prefix_versions(id, period).await? {
            former.push((old.domain, old.value));
        }
    }
    tx.restore(kind, id, property, before, period, changeset)
        .await?;
    // An object of a module: its record, then the module's own rows (REQ-008-3).
    if let (ObjectKind::Module(kind), None) = (kind, property)
        && let Some(handler) = modules.handler_of(kind)
    {
        handler
            .restore(tx.module_tx(), kind, id, before, period, changeset)
            .await?;
    }
    Ok(())
}

/// Writes a modification, noting the former extent of a resized prefix.
async fn modify<T: WriteTx>(
    tx: &mut T,
    kind: ObjectKind,
    id: Uuid,
    change: &Change,
    period: Option<Period>,
    changeset: Uuid,
    former: &mut Vec<(Uuid, Prefix)>,
) -> Result<(), WriteError> {
    if let Change::Prefix { value: Some(_), .. } = change {
        for (old, _) in tx.prefix_versions(id, period).await? {
            former.push((old.domain, old.value));
        }
    }
    // REQ-008-28: an administrator's obligation, checked like those of a declaration.
    if let Change::Obligation {
        scope,
        required: true,
    } = change
    {
        let declaration =
            crate::declarations::check_admin_obligation(tx, id, scope, period).await?;
        tx.modify(kind, id, change, period, changeset).await?;
        return crate::declarations::admin_obligation_held(tx, id, &declaration, scope, period)
            .await;
    }
    tx.modify(kind, id, change, period, changeset).await
}

/// Ends an object with what ends with it: a property's closed-list values (its
/// components), an addressing object's assignments (003/design).
async fn close<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    kind: ObjectKind,
    id: Uuid,
    period: Option<Period>,
    changeset: Uuid,
) -> Result<(), WriteError> {
    tx.close(kind, id, period, changeset).await?;
    if kind == ObjectKind::Property {
        tx.close_enum_values_of(id, period, changeset).await?;
    } else if kind.is_addressing() {
        tx.unassign(id, None, period, changeset).await?;
    } else if let ObjectKind::Module(kind) = kind {
        // Its record ended, the module's own rows and its assignments end with it
        // (REQ-008-5).
        if let Some(handler) = modules.handler_of(kind) {
            handler
                .close(tx.module_tx(), kind, id, period, changeset)
                .await?;
        }
        tx.unassign(id, None, period, changeset).await?;
    }
    Ok(())
}

/// Applies the planned writes; returns the former domain and value of each prefix whose
/// value it changes, whose content must be checked again (REQ-002-18).
#[allow(clippy::too_many_lines)] // one arm per kind of planned operation
async fn apply<T: WriteTx>(
    tx: &mut T,
    modules: &ModuleRegistry,
    planned: &[Planned],
    origins: &[usize],
    periods: &[Option<Period>],
    changeset: Uuid,
    violations: &mut Vec<Violation>,
) -> Result<Vec<(Uuid, Prefix)>, StoreError> {
    let mut former = Vec::new();
    for ((p, &period), &origin) in planned.iter().zip(periods).zip(origins) {
        let found = violations.len();
        let written = match p {
            Planned::Domain { id, slug } => {
                tx.insert_domain(*id, slug.as_str(), period, changeset)
                    .await
            }
            Planned::Prefix { object, public } => {
                tx.insert_prefix(object, *public, period, changeset).await
            }
            Planned::Range { object, public } => {
                tx.insert_range(object, *public, period, changeset).await
            }
            Planned::Address { object, public } => {
                tx.insert_address(object, *public, period, changeset).await
            }
            Planned::Policy(policy) => write_policy(tx, policy).await,
            Planned::Rbac(op) => tx.rbac_write(op, period, changeset).await,
            Planned::BindExternal {
                object,
                kind,
                source,
                external_id,
            } => tx.bind_external(*object, *kind, source, external_id).await,
            Planned::Restore {
                kind,
                id,
                property,
                before,
            } => {
                let key = (*kind, *id, *property, *before);
                restore(tx, modules, key, period, changeset, &mut former).await
            }
            Planned::Anchor {
                kind,
                id,
                slug,
                parent,
            } => {
                tx.insert_anchor(*kind, *id, slug.as_str(), *parent, period, changeset)
                    .await
            }
            Planned::Modify {
                kind,
                id,
                change,
                expected,
            } => {
                if let Some(v) = conflict(tx, *id, *expected).await? {
                    violations.push(v.at(origin));
                    continue;
                }
                modify(tx, *kind, *id, change, period, changeset, &mut former).await
            }
            Planned::Close {
                kind, id, expected, ..
            } => {
                if let Some(v) = conflict(tx, *id, *expected).await? {
                    violations.push(v.at(origin));
                    continue;
                }
                close(tx, modules, *kind, *id, period, changeset).await
            }
            Planned::Declare { id, declaration } => {
                crate::declarations::declare(tx, *id, declaration, period, changeset).await
            }
            Planned::UpdateDeclaration { id, declaration } => {
                crate::declarations::update(tx, *id, declaration, period, changeset, violations)
                    .await
            }
            Planned::EnumValue { id, property, key } => {
                crate::declarations::add_enum_value(tx, *id, *property, key, period, changeset)
                    .await
            }
            Planned::Expect { id, expected, .. } => {
                if let Some(v) = conflict(tx, *id, Some(*expected)).await? {
                    violations.push(v.at(origin));
                }
                continue;
            }
            Planned::Assign { .. } | Planned::Unassign { .. } => {
                assignment(tx, p, period, changeset).await
            }
            Planned::Module { handler, plan } => {
                handler.apply(tx.module_tx(), plan, period, changeset).await
            }
        };
        // The version of what is written, its properties included (006-16).
        if written.is_ok()
            && !matches!(
                p,
                Planned::Policy(_) | Planned::Rbac(_) | Planned::BindExternal { .. }
            )
        {
            tx.touch(p.object(), changeset).await?;
        }
        record(written, violations)?;
        ascribe(&mut violations[found..], origin);
    }
    Ok(former)
}

/// Submits a changeset (REQ-004-1 to 12): checks the request, honours its idempotency key,
/// plans and validates the operations locally, then applies them atomically under domain
/// locks; the result is journaled whether applied or refused.
///
/// # Errors
///
/// [`RequestError`] for a malformed request, a reused idempotency key or an
/// infrastructure failure.
pub async fn submit<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    req: &ChangesetRequest,
    max_operations: usize,
) -> Result<Outcome, RequestError> {
    submit_with(store, authorizer, &Sources::none(), req, max_operations).await
}

/// [`submit`] with the sources of expected values of the deployment (REQ-003-21 to 24).
///
/// # Errors
///
/// As [`submit`].
pub async fn submit_with<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    sources: &Sources,
    req: &ChangesetRequest,
    max_operations: usize,
) -> Result<Outcome, RequestError> {
    // The system author (module synchronisation, 003/P12) acts as kartenn itself.
    let context = if req.author == crate::declarations::SYSTEM_AUTHOR {
        RequestContext::system()
    } else {
        RequestContext::person(req.author)
    };
    submit_in(store, authorizer, sources, &context, req, max_operations).await
}

/// [`submit_with`] in the context of a request, whose actor, source and correlation the
/// audit events take (spec 012, REQ-012-6, 9).
///
/// # Errors
///
/// As [`submit`].
pub async fn submit_in<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    sources: &Sources,
    context: &RequestContext,
    req: &ChangesetRequest,
    max_operations: usize,
) -> Result<Outcome, RequestError> {
    // The rights are the actor's when it is the author (with its issuer and groups),
    // the author's identity alone otherwise (system work on behalf of an author).
    let author = context
        .subject()
        .filter(|s| s.id == req.author)
        .unwrap_or_else(|| crate::authz::Subject::from(req.author));
    run(
        store,
        authorizer,
        sources,
        req,
        max_operations,
        Mode::Submission,
        context,
        &author,
    )
    .await
}

/// How a changeset runs through the pipeline.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Mode {
    /// A submission: the author's rights, with proposals and approval policies.
    Submission,
    /// The validation of the pending changeset `id` by `validator` (REQ-004-21, 22, 42).
    Validation {
        id: Uuid,
        validator: Uuid,
        kind: PendingKind,
    },
}

/// The pipeline of a changeset, in the span `pipeline.changeset`, followed by a technical
/// event named after its outcome (spec 012, REQ-012-5, 25): `pipeline.changeset.<state>`
/// with `state`, `operations`, `violations` and `duration_ms`, from which the telemetry
/// derives `kartenn.changeset.count` and `kartenn.pipeline.duration`. Neither the motive
/// nor the author is logged (REQ-012-29: the audit carries them).
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
pub(crate) async fn run<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    sources: &Sources,
    req: &ChangesetRequest,
    max_operations: usize,
    mode: Mode,
    context: &RequestContext,
    author: &crate::authz::Subject,
) -> Result<Outcome, RequestError> {
    use tracing::Instrument as _;
    let operations = req.operations.len();
    let span = tracing::info_span!(
        "pipeline.changeset",
        operations,
        simulate = req.simulate,
        validation = matches!(mode, Mode::Validation { .. })
    );
    let started = std::time::Instant::now();
    let result = stages(
        store,
        authorizer,
        sources,
        req,
        max_operations,
        mode,
        context,
        author,
    )
    .instrument(span.clone())
    .await;
    let _entered = span.enter();
    log_outcome(&result, operations, started.elapsed());
    result
}

/// The changeset of an outcome, and how many violations it carries.
fn outcome_counts(result: &Result<Outcome, RequestError>) -> (Option<Uuid>, usize) {
    match result {
        Ok(
            Outcome::Applied {
                changeset,
                warnings: v,
                ..
            }
            | Outcome::Refused {
                changeset,
                violations: v,
            }
            | Outcome::FailedRevalidation {
                changeset,
                violations: v,
            },
        ) => (Some(*changeset), v.len()),
        Ok(Outcome::Replayed { changeset, .. } | Outcome::Pending { changeset, .. }) => {
            (Some(*changeset), 0)
        }
        Ok(Outcome::Simulated { violations, .. }) => (None, violations.len()),
        Err(_) => (None, 0),
    }
}

fn log_outcome(
    result: &Result<Outcome, RequestError>,
    operations: usize,
    elapsed: std::time::Duration,
) {
    let duration_ms = elapsed.as_secs_f64() * 1000.0;
    let (changeset, violations) = outcome_counts(result);
    let changeset = changeset.map(|c| c.to_string());
    macro_rules! outcome {
        ($level:expr, $name:literal, $state:literal) => {
            tracing::event!(
                name: $name,
                $level,
                state = $state,
                changeset = changeset.as_deref(),
                operations,
                violations,
                duration_ms,
                "changeset {}",
                $state
            )
        };
    }
    match result {
        Ok(Outcome::Applied { .. }) => {
            outcome!(
                tracing::Level::INFO,
                "pipeline.changeset.applied",
                "applied"
            );
        }
        Ok(Outcome::Refused { .. }) => {
            outcome!(
                tracing::Level::INFO,
                "pipeline.changeset.refused",
                "refused"
            );
        }
        Ok(Outcome::Replayed { .. }) => {
            outcome!(
                tracing::Level::INFO,
                "pipeline.changeset.replayed",
                "replayed"
            );
        }
        Ok(Outcome::Pending { .. }) => {
            outcome!(
                tracing::Level::INFO,
                "pipeline.changeset.pending",
                "pending"
            );
        }
        Ok(Outcome::FailedRevalidation { .. }) => {
            outcome!(tracing::Level::INFO, "pipeline.changeset.stale", "stale");
        }
        Ok(Outcome::Simulated { .. }) => {
            outcome!(
                tracing::Level::INFO,
                "pipeline.changeset.simulated",
                "simulated"
            );
        }
        Err(RequestError::Audit(e)) => tracing::error!(
            name: "audit.write.failed",
            state = "failed",
            operations,
            duration_ms,
            error = %e,
            "the changeset could not be audited, so it was not made"
        ),
        Err(RequestError::Store(e)) => tracing::error!(
            name: "pipeline.changeset.failed",
            state = "failed",
            operations,
            duration_ms,
            error = %e,
            "changeset failed"
        ),
        Err(e) => tracing::warn!(
            name: "pipeline.changeset.rejected",
            state = "rejected",
            operations,
            duration_ms,
            error = %e,
            "changeset rejected"
        ),
    }
}

/// The stages of a changeset (004/design, § Pipeline d'un changeset).
#[allow(clippy::too_many_lines, clippy::too_many_arguments)] // one linear pipeline
async fn stages<S: Store, A: Authorizer>(
    store: &S,
    authorizer: &A,
    sources: &Sources,
    req: &ChangesetRequest,
    max_operations: usize,
    mode: Mode,
    context: &RequestContext,
    author: &crate::authz::Subject,
) -> Result<Outcome, RequestError> {
    let modules = store.modules();
    let period = check_request(req, max_operations, modules)?;
    let changeset = match mode {
        Mode::Submission => Uuid::now_v7(),
        Mode::Validation { id, .. } => id,
    };
    let mut tx = store.begin().await?;
    match mode {
        Mode::Submission if req.idempotency_key.is_some() && !req.simulate => {
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
                Replay::InProgress => {
                    tx.rollback().await?;
                    return Err(RequestError::IdempotencyInProgress);
                }
            }
        }
        Mode::Submission => {}
        // Validations of one changeset are serialised; only a pending one is decided.
        Mode::Validation { id, .. } => {
            if tx.lock_changeset(id).await?.as_deref() != Some("pending") {
                tx.rollback().await?;
                return Err(RequestError::Review(not_pending(id)));
            }
        }
    }
    let prepared = match prepare(&mut tx, req, modules, period).await {
        Ok(prepared) => prepared,
        Err(e) => {
            tx.rollback().await?;
            return Err(e.into());
        }
    };
    let req: &ChangesetRequest = &prepared;
    let (planned, origins, mut violations) = plan(req, modules);
    crate::expected::check_declared_sources(&planned, &origins, sources, &mut violations);
    if has_errors(&violations) {
        tx.rollback().await?;
        return refuse(store, req, changeset, violations, mode, context).await;
    }
    // REQ-004-26: closures in cascade, expanded before the locks.
    // REQ-004-2: each operation on its own period, or the changeset's; the property stage
    // covers them all.
    let periods: Vec<Option<Period>> = req
        .operations
        .iter()
        .map(|op| to_period(op.own_period()).map(|own| own.or(period)))
        .collect::<Result<_, _>>()?;
    let window = window(period, &periods);
    // One period per planned operation: that of the operation it comes from.
    let periods: Vec<Option<Period>> = origins.iter().map(|&o| periods[o]).collect();
    let (planned, origins, periods, cascaded) =
        crate::cascade::expand(&mut tx, modules, planned, origins, periods).await?;
    let (domains, public, keys) = lock_targets(&mut tx, &planned).await?;
    tx.lock(&domains, public).await?;
    tx.lock_keys(&keys).await?;
    // The policies in force at submission, before the changeset changes them.
    let policies = match mode {
        Mode::Submission => tx.approval_policies().await?,
        Mode::Validation { .. } => Vec::new(),
    };
    let validator = context
        .subject()
        .unwrap_or_else(|| crate::authz::Subject::from(req.author));
    let (subject, may_propose) = match mode {
        Mode::Submission => (author, true),
        Mode::Validation {
            kind: PendingKind::Proposal,
            ..
        } => (&validator, false),
        Mode::Validation {
            kind: PendingKind::Approval,
            ..
        } => (author, false),
    };
    // REQ-005-22: the objects the changeset closes or changes, before its writes.
    let missing_before = match crate::rights::check_objects(
        &mut tx,
        authorizer,
        subject,
        &planned,
        crate::rights::Phase::Before,
        may_propose,
        &mut violations,
    )
    .await
    {
        Ok(missing) => missing,
        Err(e) => {
            tx.rollback().await?;
            return Err(e.into());
        }
    };
    let former = match apply(
        &mut tx,
        modules,
        &planned,
        &origins,
        &periods,
        changeset,
        &mut violations,
    )
    .await
    {
        Ok(former) => former,
        Err(e) => {
            tx.rollback().await?;
            return Err(e.into());
        }
    };
    if !has_errors(&violations)
        && let Err(e) =
            crate::global::validate(&mut tx, &planned, &periods, &former, &mut violations).await
    {
        tx.rollback().await?;
        return Err(e.into());
    }
    // REQ-008-10: what the modules still reference is not closed.
    if !has_errors(&violations)
        && let Err(e) =
            crate::cascade::check_referrers(&mut tx, modules, &planned, &periods, &mut violations)
                .await
    {
        tx.rollback().await?;
        return Err(e.into());
    }
    if has_errors(&violations) {
        tx.rollback().await?;
        return refuse(store, req, changeset, violations, mode, context).await;
    }
    // Rights: the author's when submitting (a lack covered by `…propose` makes a
    // proposal); a proposal's validator's when validating it; the author's again for an
    // approval, whose validator needs the approval permission below.
    let props = properties(
        &mut tx,
        authorizer,
        sources,
        subject,
        may_propose,
        &planned,
        window,
        changeset,
        &mut violations,
    )
    .await;
    let Checked {
        diff,
        expected: checked,
        mut missing,
    } = match props {
        Ok(Some(found)) => found,
        Ok(None) => {
            tx.rollback().await?;
            // A validator without the rights leaves the proposal pending.
            if let Mode::Validation {
                validator,
                kind: PendingKind::Proposal,
                ..
            } = mode
                && let Some(denied) = only_rights(&violations)
            {
                return Err(RequestError::Review(forbidden(validator, denied)));
            }
            return refuse(store, req, changeset, violations, mode, context).await;
        }
        Err(e) => {
            tx.rollback().await?;
            return Err(e.into());
        }
    };
    missing.extend(missing_before);
    // REQ-008-26: the sections of the modules, then their rules on the final state.
    let touched = crate::sections::touched(&planned);
    let sections =
        match crate::sections::contribute(&mut tx, modules, changeset, &diff, &touched).await {
            Ok(sections) => sections,
            Err(e) => {
                tx.rollback().await?;
                return Err(e.into());
            }
        };
    let module_findings = match check_modules(
        &mut tx,
        modules,
        changeset,
        &planned,
        &touched,
        &sections,
        &mut violations,
    )
    .await
    {
        Ok(findings) => findings,
        Err(e) => {
            tx.rollback().await?;
            return Err(e.into());
        }
    };
    if has_errors(&violations) {
        tx.rollback().await?;
        return refuse(store, req, changeset, violations, mode, context).await;
    }
    if req.simulate {
        let sample = tx.diff_sample(SIMULATION_SAMPLE).await;
        tx.rollback().await?;
        return Ok(Outcome::Simulated {
            violations,
            diff,
            sample: sample?,
            cascaded,
            sections: sections.into_iter().map(SectionSample::of).collect(),
        });
    }
    let objects: Vec<Uuid> = planned.iter().map(Planned::object).collect();
    match mode {
        Mode::Submission => {
            // REQ-016-10: an operation the module requires a review of makes a proposal,
            // even for an author who may write it; its validator needs the permission.
            missing.extend(reviews(&planned));
            let kind = if missing.is_empty() {
                crate::review::holding_policy(
                    &mut tx,
                    &policies,
                    req,
                    author,
                    &planned,
                    &diff,
                    &domains,
                    &module_findings.measures,
                )
                .await?
                .map(|policy| (PendingKind::Approval, Some(policy)))
            } else {
                Some((PendingKind::Proposal, None))
            };
            if let Some((kind, policy)) = kind {
                let mut perimeter: BTreeSet<Uuid> = objects.iter().copied().collect();
                perimeter.extend(crate::rights::diff_objects(&mut tx, &diff).await?);
                tx.rollback().await?;
                let pending = Pending {
                    policy,
                    perimeter: perimeter.into_iter().collect(),
                    required: missing,
                    author: Some(author.clone()),
                };
                return hold(
                    store,
                    context,
                    req,
                    changeset,
                    kind,
                    &pending,
                    &violations,
                    &domains,
                    diff,
                )
                .await;
            }
        }
        Mode::Validation {
            validator,
            kind: PendingKind::Approval,
            ..
        } => {
            let mut perimeter: BTreeSet<Uuid> = objects.iter().copied().collect();
            perimeter.extend(crate::rights::diff_objects(&mut tx, &diff).await?);
            let perimeter: Vec<Uuid> = perimeter.into_iter().collect();
            let denied = authorizer
                .authorize_in(
                    &mut tx,
                    &context.subject().unwrap_or_else(|| validator.into()),
                    APPROVE_PERMISSION,
                    &perimeter,
                )
                .await?;
            if !denied.is_empty() {
                tx.rollback().await?;
                let error = RequestError::Review(forbidden(validator, denied));
                return Err(crate::review::audited(store, context, error, changeset).await);
            }
        }
        // REQ-016-11: the review permission the module names, on objects that now exist
        // in the transaction; the validator's write permissions were checked above.
        Mode::Validation {
            validator,
            kind: PendingKind::Proposal,
            ..
        } => {
            for (permission, objects) in reviews(&planned) {
                let denied = authorizer
                    .authorize_in(&mut tx, subject, &permission, &objects)
                    .await?;
                if !denied.is_empty() {
                    tx.rollback().await?;
                    let error = RequestError::Review(forbidden(validator, denied));
                    return Err(crate::review::audited(store, context, error, changeset).await);
                }
            }
        }
    }
    let mut reevaluated = reevaluated(&planned, &checked);
    reevaluated.extend(module_findings.reevaluated);
    let (kind, validator) = match mode {
        Mode::Submission => ("direct", None),
        Mode::Validation {
            validator, kind, ..
        } => (kind.as_str(), Some(validator)),
    };
    let journal = Journal {
        id: changeset,
        request: req,
        state: "applied",
        kind,
        validator,
        pending: None,
        violations: &violations,
        domains: &domains,
        keys: &keys,
        objects: &objects,
        reevaluated: &reevaluated,
        diff: &diff,
        cascaded: &cascaded,
        audit: &[],
        on_behalf_of: context.on_behalf_of,
    };
    let events = crate::audit::changeset_events(context, &journal, &planned)?;
    let journal = Journal {
        audit: &events,
        ..journal
    };
    tx.journal_applied(&journal).await?;
    tx.commit().await?;
    Ok(Outcome::Applied {
        changeset,
        warnings: violations,
        diff,
        cascaded,
    })
}

/// Permission to approve a changeset held by a policy, over its whole perimeter
/// (REQ-004-21).
pub const APPROVE_PERMISSION: &str = "core.changeset.approve";

/// The kind of a pending changeset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PendingKind {
    /// Held by an approval policy (REQ-004-20).
    Approval,
    /// Proposed by an author who may only propose (REQ-004-41).
    Proposal,
}

impl PendingKind {
    /// Stored form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approval => "approval",
            Self::Proposal => "proposal",
        }
    }
}

/// The review permissions the modules require on the objects of `planned` (REQ-016-10),
/// with the objects each covers.
fn reviews(planned: &[Planned]) -> crate::rights::Missing {
    let mut by: std::collections::BTreeMap<String, Vec<Uuid>> = std::collections::BTreeMap::new();
    for p in planned {
        if let Planned::Module { handler, plan } = p
            && let Some(permission) = handler.review(plan)
        {
            by.entry(permission.to_owned())
                .or_default()
                .push(plan.object);
        }
    }
    by.into_iter().collect()
}

fn not_pending(id: Uuid) -> Violation {
    Violation::new(
        &rules::CHANGESET_NOT_PENDING,
        format!("changeset {id} is not pending"),
    )
    .with_subjects([id])
}

fn forbidden(validator: Uuid, denied: Vec<Uuid>) -> Violation {
    Violation::new(
        &rules::APPROVAL_FORBIDDEN,
        format!(
            "{validator} lacks the right to validate on {} object(s)",
            denied.len()
        ),
    )
    .with_subjects(denied)
}

/// The objects refused when every error is a lack of right; `None` otherwise.
fn only_rights(violations: &[Violation]) -> Option<Vec<Uuid>> {
    let errors: Vec<&Violation> = violations
        .iter()
        .filter(|v| v.rule.severity == Severity::Error)
        .collect();
    errors
        .iter()
        .all(|v| {
            v.rule.id == rules::PROP_FORBIDDEN_EFFECT.id || v.rule.id == rules::OBJECT_FORBIDDEN.id
        })
        .then(|| errors.iter().flat_map(|v| v.subjects.clone()).collect())
}

/// Journals a pending changeset after the rollback of its writes, with its event.
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
async fn hold<S: Store>(
    store: &S,
    context: &RequestContext,
    req: &ChangesetRequest,
    changeset: Uuid,
    kind: PendingKind,
    pending: &Pending,
    violations: &[Violation],
    domains: &[Uuid],
    diff: Vec<DiffCount>,
) -> Result<Outcome, RequestError> {
    let journal = Journal {
        id: changeset,
        request: req,
        state: "pending",
        kind: kind.as_str(),
        validator: None,
        pending: Some(pending),
        violations,
        domains,
        keys: &[],
        objects: &[],
        reevaluated: &[],
        diff: &diff,
        cascaded: &[],
        audit: &[],
        on_behalf_of: context.on_behalf_of,
    };
    let events = crate::audit::changeset_events(context, &journal, &[])?;
    let journal = Journal {
        audit: &events,
        ..journal
    };
    store.journal_refused(&journal).await?;
    Ok(Outcome::Pending {
        changeset,
        kind,
        policy: pending.policy,
        diff,
    })
}

/// Entries of the differential detail returned by a simulation (003/design).
const SIMULATION_SAMPLE: i64 = 100;

/// What the properties stage found: the differential, the objects whose expected values
/// were checked, and the write permissions lacking but proposable.
struct Checked {
    diff: Vec<DiffCount>,
    expected: Vec<Uuid>,
    missing: crate::rights::Missing,
}

/// Properties after the writes (spec 003): origins, obligations, differential, then the
/// rights on the assignments and on their effects (REQ-003-17, 18). `None` when a rule
/// refuses the changeset; the violations say why.
#[allow(clippy::too_many_arguments)] // the context of a changeset, passed down unbundled
async fn properties<T: WriteTx + Send, A: Authorizer>(
    tx: &mut T,
    authorizer: &A,
    sources: &Sources,
    subject: &crate::authz::Subject,
    may_propose: bool,
    planned: &[Planned],
    period: Option<Period>,
    changeset: Uuid,
    violations: &mut Vec<Violation>,
) -> Result<Option<Checked>, StoreError> {
    // REQ-003-22: default values first, so that they are resolved, obliged and diffed like
    // any assignment.
    let defaults = match crate::expected::defaults(tx, sources, planned, period, changeset).await {
        Ok(defaults) => defaults,
        Err(WriteError::Store(e)) => return Err(e),
        Err(WriteError::Rule(v)) => {
            violations.push(v);
            return Ok(None);
        }
        Err(WriteError::NoVersion(id)) => {
            violations.push(
                Violation::new(
                    &rules::CHANGESET_NO_VERSION,
                    format!("{id} does not exist at the targeted time"),
                )
                .with_subjects([id]),
            );
            return Ok(None);
        }
    };
    let mut all = planned.to_vec();
    all.extend(defaults);
    let maintained = crate::resolution::maintain(tx, &all, period, changeset).await?;
    let refreshed = &maintained.affected;
    crate::resolution::check_obligations(tx, &all, refreshed, period, violations).await?;
    if has_errors(violations) {
        return Ok(None);
    }
    let touched = crate::resolution::touched(&all, refreshed);
    let diff = crate::resolution::diff(tx, &all, &maintained, period).await?;
    // Default values are not the author's choice: only their effects need rights.
    let missing = crate::rights::check(
        tx,
        authorizer,
        subject,
        planned,
        &diff,
        period,
        may_propose,
        violations,
    )
    .await?;
    if has_errors(violations) {
        return Ok(None);
    }
    let checked = crate::expected::check(tx, sources, &touched, period, violations).await?;
    Ok(Some(Checked {
        diff,
        expected: checked,
        missing,
    }))
}

/// Rules evaluated anew by the changeset, per object (REQ-002-35, 36): the core rules on
/// the objects it creates, every rule on those it closes, the expected values on the
/// objects checked.
fn reevaluated(planned: &[Planned], checked: &[Uuid]) -> Vec<(Uuid, &'static str)> {
    let mut out: Vec<(Uuid, &'static str)> = planned
        .iter()
        .filter_map(|p| match p {
            Planned::Domain { id, .. }
            | Planned::Modify { id, .. }
            | Planned::Restore {
                id, property: None, ..
            } => Some((*id, "core.")),
            Planned::Prefix { object, .. } => Some((object.id, "core.")),
            Planned::Range { object, .. } => Some((object.id, "core.")),
            Planned::Address { object, .. } => Some((object.id, "core.")),
            Planned::Close { kind, id, .. } if kind.is_addressing() => Some((*id, "")),
            _ => None,
        })
        .collect();
    out.extend(
        checked
            .iter()
            .map(|o| (*o, rules::PROP_EXPECTED_MISMATCH.id)),
    );
    out
}

/// Journals a changeset refused at submission, or a pending one refused on the current
/// state when validated (REQ-004-22).
async fn refuse<S: Store>(
    store: &S,
    req: &ChangesetRequest,
    changeset: Uuid,
    violations: Vec<Violation>,
    mode: Mode,
    context: &RequestContext,
) -> Result<Outcome, RequestError> {
    if req.simulate {
        return Ok(Outcome::Simulated {
            violations,
            diff: Vec::new(),
            sample: Vec::new(),
            cascaded: Vec::new(),
            sections: Vec::new(),
        });
    }
    let journal = Journal {
        id: changeset,
        request: req,
        state: match mode {
            Mode::Submission => "refused",
            Mode::Validation { .. } => "failed_revalidation",
        },
        kind: match mode {
            Mode::Submission => "direct",
            Mode::Validation { kind, .. } => kind.as_str(),
        },
        validator: match mode {
            Mode::Submission => None,
            Mode::Validation { validator, .. } => Some(validator),
        },
        pending: None,
        violations: &violations,
        domains: &[],
        keys: &[],
        objects: &[],
        reevaluated: &[],
        diff: &[],
        cascaded: &[],
        audit: &[],
        on_behalf_of: context.on_behalf_of,
    };
    let events = crate::audit::changeset_events(context, &journal, &[])?;
    let journal = Journal {
        audit: &events,
        ..journal
    };
    store.journal_refused(&journal).await?;
    Ok(match mode {
        Mode::Submission => Outcome::Refused {
            changeset,
            violations,
        },
        Mode::Validation { .. } => Outcome::FailedRevalidation {
            changeset,
            violations,
        },
    })
}

/// The period the property stage covers: from the earliest start of the operations (the
/// changeset's period when none has its own, REQ-004-2), unbounded.
fn window(period: Option<Period>, periods: &[Option<Period>]) -> Option<Period> {
    if periods.iter().all(|p| *p == period) {
        return period;
    }
    // An operation without a start runs from the transaction time: a minute earlier is a
    // safe superset whatever the clocks.
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros(),
    )
    .unwrap_or(i64::MAX)
        - 60_000_000;
    let start = periods
        .iter()
        .map(|p| p.map_or(now, |p| p.start().0))
        .chain(std::iter::once(period.map_or(now, |p| p.start().0)))
        .min()
        .unwrap_or(now);
    Period::new(Instant(start), None).ok()
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
            anycast: false,
        }
    }

    /// The checks of a build without modules.
    fn check_request(req: &ChangesetRequest, max: usize) -> Result<Option<Period>, RequestError> {
        super::check_request(req, max, ModuleRegistry::empty())
    }

    /// The planning of a build without modules.
    fn plan(req: &ChangesetRequest) -> (Vec<Planned>, Vec<usize>, Vec<Violation>) {
        super::plan(req, ModuleRegistry::empty())
    }

    #[test]
    fn module_operations_round_trip_through_json() {
        let json = serde_json::json!({"op": "vlan.create_vlan", "local": 3, "vid": 10});
        let op: Operation = serde_json::from_value(json.clone()).unwrap();
        let Operation::Module(m) = &op else {
            panic!("{op:?}")
        };
        assert_eq!(
            (m.module.as_str(), m.name.as_str()),
            ("vlan", "create_vlan")
        );
        assert_eq!(op.defines(), Some(3));
        assert_eq!(serde_json::to_value(&op).unwrap(), json);
        let core = serde_json::json!({"op": "create_domain", "local": 1, "slug": "lab"});
        let op: Operation = serde_json::from_value(core.clone()).unwrap();
        assert!(matches!(op, Operation::CreateDomain { .. }));
        assert_eq!(serde_json::to_value(&op).unwrap()["op"], "create_domain");
        let kind: ObjectKind = serde_json::from_value(serde_json::json!("vlan.vlan")).unwrap();
        assert_eq!(kind.name(), "vlan.vlan");
        assert!(serde_json::from_value::<ObjectKind>(serde_json::json!("vlan")).is_err());
        assert_eq!(
            serde_json::to_value(ObjectKind::Prefix).unwrap(),
            serde_json::json!("prefix")
        );
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
            anycast: false,
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
        let (planned, origins, violations) = plan(&req(vec![
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
        // Each violation and each planned write keeps the index of its operation.
        let at: Vec<Option<usize>> = violations.iter().map(|v| v.operation).collect();
        assert_eq!(at, vec![Some(0), Some(1), Some(2), Some(3)]);
        assert_eq!(origins, vec![2, 4]);
    }

    #[test]
    fn public_flag_comes_from_the_registry() {
        let d = Ref::Id(Uuid::now_v7());
        let (planned, _, _) = plan(&req(vec![
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
