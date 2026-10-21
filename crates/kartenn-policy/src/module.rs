//! The policy module as the core sees it (ADR 0026): its types of objects, permissions,
//! rules, operations, references and migrations, its property declarations, and its
//! handler in the write pipeline (feature `pg`).

use std::collections::BTreeSet;
use std::sync::Arc;

use kartenn_core::authz::{Permission, TypePermissions};
use kartenn_core::properties::{
    Declarant, Declaration, Expected, ExpectedMode, InheritTree, PropertyKey, TypeSpec,
};
use kartenn_core::rules::Rule;
use kartenn_core::values::Cardinality;
use kartenn_service::declarations::ModuleDeclarations;
use kartenn_service::module::{
    Module, ModuleHandler, ModuleMigration, ObjectType, OnClose, OperationType, Reference,
};

use crate::rules::POLICY_RULES;

/// The property of the environments (REQ-010-1, S2): the one imposed axis (S22).
pub const ENVIRONMENT: &str = "policy.environment";
/// The environment a machine declares (REQ-010-3, S3): never read by a verdict.
pub const DECLARED_ENVIRONMENT: &str = "policy.declared_environment";

/// The qualified type of a designated axis (REQ-010-29).
pub const AXIS: &str = "policy.axis";
/// The qualified type of the attributes of a value of an axis (REQ-010-2, 30).
pub const VALUE: &str = "policy.value";
/// The qualified type of a flow type (REQ-010-4).
pub const FLOW_TYPE: &str = "policy.flow_type";
/// The qualified type of a service (REQ-010-5).
pub const SERVICE: &str = "policy.service";
/// The qualified type of an architecture pattern (REQ-010-9).
pub const PATTERN: &str = "policy.pattern";
/// The qualified type of a rule (REQ-010-8).
pub const RULE: &str = "policy.rule";
/// The qualified type of an admissibility condition (REQ-010-36).
pub const CONDITION: &str = "policy.condition";
/// The qualified type of a signal (REQ-010-37).
pub const SIGNAL: &str = "policy.signal";
/// The qualified type of a role of interface exempt from S10.
pub const EXEMPT_ROLE: &str = "policy.exempt_role";

/// The qualified type of a rule of inference (REQ-010-42 to 46): an exception when it has
/// a parent; anchored in the domain it cites, if any (its scope for the rights).
pub const INFERENCE_RULE: &str = "policy.inference_rule";

/// Every type of the module, with its collection under `/v1` (REQ-010-27).
pub(crate) const KINDS: &[(&str, &str)] = &[
    (AXIS, "policy-axes"),
    (VALUE, "policy-values"),
    (FLOW_TYPE, "flow-types"),
    (SERVICE, "services"),
    (PATTERN, "policy-patterns"),
    (RULE, "policy-rules"),
    (CONDITION, "admissibility-conditions"),
    (SIGNAL, "risk-signals"),
    (EXEMPT_ROLE, "policy-exempt-roles"),
    (INFERENCE_RULE, "inference-rules"),
];

/// The axes the module declares as examples, designated only by
/// `policy.examples.install` (REQ-010-29): carried by the tree and the machine (S27),
/// closed lists with their values, or external references (N26).
pub(crate) const EXAMPLES: &[(&str, &[&str])] = &[
    (
        "policy.exposure",
        &["internet", "partenaires", "interne", "administration"],
    ),
    (
        "policy.sensitivity",
        &["public", "interne", "confidentiel", "restreint"],
    ),
    ("policy.criticality", &["standard", "importante", "vitale"]),
    (
        "policy.conduit_role",
        &["stockage-objets", "passerelle-api", "bastion"],
    ),
    ("policy.application", &[]),
    ("policy.owner", &[]),
];

/// The indicator of personal data that goes with `policy.sensitivity` (REQ-010-29).
pub(crate) const PERSONAL_DATA: &str = "policy.personal_data";

/// The carriers of the addressing tree (REQ-010-1): a range carries no classification.
const TREE: [&str; 3] = ["domain", "prefix", "address"];
/// The type of the machines (009).
pub(crate) const MACHINE: &str = "machines.machine";

/// A type of the module: no anchor, no parent (S15).
const fn global(kind: &'static str, collection: &'static str) -> ObjectType {
    ObjectType {
        kind,
        collection,
        parent: None,
        owner_required: false,
        carries_site: false,
        domain_anchored: false,
    }
}

static TYPES: &[ObjectType] = &[
    global(KINDS[0].0, KINDS[0].1),
    global(KINDS[1].0, KINDS[1].1),
    global(KINDS[2].0, KINDS[2].1),
    global(KINDS[3].0, KINDS[3].1),
    global(KINDS[4].0, KINDS[4].1),
    global(KINDS[5].0, KINDS[5].1),
    global(KINDS[6].0, KINDS[6].1),
    global(KINDS[7].0, KINDS[7].1),
    global(KINDS[8].0, KINDS[8].1),
    // A rule that cites a domain is written under `policy.inference.admin` on that domain,
    // under the whole referential otherwise (REQ-010-46).
    ObjectType {
        domain_anchored: true,
        ..global(KINDS[9].0, KINDS[9].1)
    },
];

static OPERATIONS: &[OperationType] = &[
    OperationType {
        name: "axis.designate",
        defines: Some(AXIS),
    },
    OperationType {
        name: "axis.update",
        defines: None,
    },
    OperationType {
        name: "value.describe",
        defines: Some(VALUE),
    },
    OperationType {
        name: "environment.create",
        defines: Some(VALUE),
    },
    OperationType {
        name: "environment.close",
        defines: None,
    },
    OperationType {
        name: "examples.install",
        defines: Some(AXIS),
    },
    OperationType {
        name: "flow_type.create",
        defines: Some(FLOW_TYPE),
    },
    OperationType {
        name: "flow_type.update",
        defines: None,
    },
    OperationType {
        name: "service.create",
        defines: Some(SERVICE),
    },
    OperationType {
        name: "service.update",
        defines: None,
    },
    OperationType {
        name: "pattern.create",
        defines: Some(PATTERN),
    },
    OperationType {
        name: "pattern.update",
        defines: None,
    },
    OperationType {
        name: "rule.create",
        defines: Some(RULE),
    },
    OperationType {
        name: "rule.update",
        defines: None,
    },
    // A cell of the matrix view (REQ-010-34): creates, updates or closes its rule.
    OperationType {
        name: "matrix.set",
        defines: Some(RULE),
    },
    OperationType {
        name: "condition.create",
        defines: Some(CONDITION),
    },
    OperationType {
        name: "condition.update",
        defines: None,
    },
    OperationType {
        name: "signal.create",
        defines: Some(SIGNAL),
    },
    OperationType {
        name: "signal.update",
        defines: None,
    },
    OperationType {
        name: "exempt_role.create",
        defines: Some(EXEMPT_ROLE),
    },
    OperationType {
        name: "inference_rule.create",
        defines: Some(INFERENCE_RULE),
    },
    OperationType {
        name: "inference_rule.update",
        defines: None,
    },
];

/// A reference refused while its source lives (D33).
const fn refuse(from: &'static str, to: &'static str) -> Reference {
    Reference {
        from,
        to,
        on_close: OnClose::Refuse,
    }
}

/// The references of the module (D33): an axis keeps its property; the attributes of a
/// value go with it when the value is closed with the cascade (REQ-010-2); what a service,
/// a rule or a condition cites, and the value a rule admits, cannot be closed under it.
static REFERENCES: &[Reference] = &[
    refuse(AXIS, "property"),
    Reference {
        from: VALUE,
        to: "enum_value",
        on_close: OnClose::Cascade,
    },
    refuse(SERVICE, FLOW_TYPE),
    refuse(RULE, FLOW_TYPE),
    refuse(RULE, SERVICE),
    refuse(RULE, PATTERN),
    refuse(RULE, "enum_value"),
    refuse(CONDITION, RULE),
    refuse(CONDITION, FLOW_TYPE),
];

static MIGRATIONS: &[ModuleMigration] = &[ModuleMigration {
    version: 1,
    name: "policy",
    sql: include_str!("../migrations/0001_policy.sql"),
}];

macro_rules! permissions {
    ($list:ident; $($name:ident = $id:literal, $global:literal, $summary:literal;)*) => {
        $(
            static $name: Permission = Permission {
                name: $id,
                summary: $summary,
                global_only: $global,
            };
        )*
        static $list: &[&Permission] = &[$(&$name),*];
    };
}

// The rights of the policy (S15, amended by Q17; REQ-010-24). Its objects have no anchor:
// administering them is granted at the global scope only, reading them is a right of the
// delivered reader roles. `policy.inference.admin` is scoped by domain (REQ-010-46),
// `policy.flow.request` by entity; the write of each axis is the property permission of
// its key, on the scope of the node or the machine (REQ-003-17).
permissions! {
    PERMISSIONS;
    READ = "policy.catalogue.read", false, "Lire la politique : axes, valeurs, types de flux, services, règles, motifs, conditions et signaux.";
    VERDICT_READ = "policy.verdict.read", false, "Demander un verdict de conformité entre deux extrémités dont on détient la lecture.";
    FLOW_REQUEST = "policy.flow.request", false, "Demander un flux pour une entité : verdict de toute paire, explication réduite aux valeurs, règles, motifs et conditions.";
    AXIS_ADMIN = "policy.axis.admin", true, "Désigner, modifier ou retirer un axe, décrire ses valeurs.";
    ENVIRONMENT_ADMIN_PERMISSION = "policy.environment.admin", true, "Créer, décrire ou clore un environnement.";
    FLOW_TYPE_ADMIN = "policy.flow_type.admin", true, "Créer, modifier ou clore un type de flux.";
    SERVICE_ADMIN = "policy.service.admin", true, "Créer, modifier ou clore un service.";
    RULE_ADMIN = "policy.rule.admin", true, "Créer, modifier ou clore une règle, un motif, une condition d'admissibilité, un signal de risque ou un rôle exempté.";
    INFERENCE_ADMIN = "policy.inference.admin", false, "Écrire les règles d'inférence qui citent un domaine du périmètre.";
}

/// The permission an environment is written under, besides the write of the values (S15).
pub(crate) const ENVIRONMENT_ADMIN: &str = "policy.environment.admin";

/// The types of the module under the permissions of S15: one read for all, one
/// administration by family (rules, patterns, conditions, signals and exempt roles under
/// `policy.rule.admin`).
static TYPE_PERMISSIONS: &[TypePermissions] = &[
    typed(AXIS, "policy.axis.admin"),
    typed(VALUE, "policy.axis.admin"),
    typed(FLOW_TYPE, "policy.flow_type.admin"),
    typed(SERVICE, "policy.service.admin"),
    typed(PATTERN, "policy.rule.admin"),
    typed(RULE, "policy.rule.admin"),
    typed(CONDITION, "policy.rule.admin"),
    typed(SIGNAL, "policy.rule.admin"),
    typed(EXEMPT_ROLE, "policy.rule.admin"),
    typed(INFERENCE_RULE, "policy.inference.admin"),
];

const fn typed(kind: &'static str, write: &'static str) -> TypePermissions {
    TypePermissions {
        kind,
        read: "policy.catalogue.read",
        write,
    }
}

/// The events of the module (N15, ADR 0026): a new revision of the policy.
static EVENTS: &[&str] = &["revised"];

/// A declaration of the module, from its key, type, carriers and inheritance.
fn declaration(
    key: &str,
    spec: TypeSpec,
    carriers: &[&str],
    inherit: Option<InheritTree>,
) -> Declaration {
    Declaration {
        key: PropertyKey::parse(key).expect("a valid key"),
        declarant: Declarant::Module("policy".into()),
        spec,
        cardinality: Cardinality::Set,
        carriers: carriers.iter().map(|&c| c.to_owned()).collect(),
        inherit,
        obligations: BTreeSet::new(),
        expected: None,
    }
}

/// The policy module (spec 010).
#[derive(Debug, Clone, Copy, Default)]
pub struct Policy;

impl Module for Policy {
    fn name(&self) -> &'static str {
        "policy"
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    /// The machines carry the axes of S27 and the declared environment (REQ-010-3).
    fn depends_on(&self) -> &'static [&'static str] {
        &["machines"]
    }
    fn object_types(&self) -> &'static [ObjectType] {
        TYPES
    }
    /// `policy.environment` (REQ-010-1), `policy.declared_environment` sharing its values
    /// (REQ-010-3, N7), and the axes of the examples (REQ-010-29), declared but designated
    /// as axes only on request.
    fn declarations(&self) -> Option<ModuleDeclarations> {
        let mut tree_and_machine: Vec<&str> = TREE.to_vec();
        tree_and_machine.push(MACHINE);
        let mut properties = vec![
            (
                // The inference proposes its expected environments and writes the single
                // one as a default value at creation (REQ-010-51, 52).
                Declaration {
                    expected: Some(Expected {
                        source: "policy.inference".into(),
                        mode: ExpectedMode::Default,
                    }),
                    ..declaration(
                        ENVIRONMENT,
                        TypeSpec::ENUM,
                        &TREE,
                        Some(InheritTree::Addressing),
                    )
                },
                Vec::new(),
            ),
            (
                declaration(
                    DECLARED_ENVIRONMENT,
                    TypeSpec::Enum {
                        values_of: Some(PropertyKey::parse(ENVIRONMENT).expect("a valid key")),
                    },
                    &[MACHINE],
                    None,
                ),
                Vec::new(),
            ),
            (
                Declaration {
                    cardinality: Cardinality::Single,
                    ..declaration(
                        PERSONAL_DATA,
                        TypeSpec::Bool,
                        &tree_and_machine,
                        Some(InheritTree::Addressing),
                    )
                },
                Vec::new(),
            ),
        ];
        for (key, values) in EXAMPLES {
            let spec = if values.is_empty() {
                TypeSpec::ExternalRef
            } else {
                TypeSpec::ENUM
            };
            properties.push((
                declaration(key, spec, &tree_and_machine, Some(InheritTree::Addressing)),
                values.iter().map(|&v| v.to_owned()).collect(),
            ));
        }
        Some(ModuleDeclarations {
            module: "policy".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            properties,
        })
    }
    fn permissions(&self) -> &'static [&'static Permission] {
        PERMISSIONS
    }
    fn type_permissions(&self) -> &'static [TypePermissions] {
        TYPE_PERMISSIONS
    }
    fn rules(&self) -> &'static [&'static Rule] {
        POLICY_RULES
    }
    fn operations(&self) -> &'static [OperationType] {
        OPERATIONS
    }
    fn references(&self) -> &'static [Reference] {
        REFERENCES
    }
    fn migrations(&self) -> &'static [ModuleMigration] {
        MIGRATIONS
    }
    fn events(&self) -> &'static [&'static str] {
        EVENTS
    }
    /// The approval triggers of the policy (REQ-010-20, S12), `policy.<name>` once
    /// qualified: none applies by default.
    fn measures(&self) -> &'static [&'static str] {
        &["write", "opens", "closes", "reclassify"]
    }
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        #[cfg(feature = "pg")]
        {
            Some(Arc::new(crate::handler::Handler))
        }
        #[cfg(not(feature = "pg"))]
        {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use kartenn_service::module::ModuleRegistry;

    use super::*;

    /// The module needs the machines (and the VLAN they need): registered alone, it is
    /// refused; with them, its permissions stay in its namespace.
    #[test]
    fn the_module_registers_after_the_machines() {
        assert!(ModuleRegistry::new(vec![Arc::new(Policy)]).is_err());
        let registry = ModuleRegistry::new(vec![
            Arc::new(kartenn_service_fakes::Named("vlan", &[])),
            Arc::new(kartenn_service_fakes::Named("machines", &["vlan"])),
            Arc::new(Policy),
        ])
        .expect("a valid module");
        assert!(
            registry
                .permissions()
                .iter()
                .filter(|p| p.name.starts_with("policy."))
                .count()
                >= 6
        );
    }

    /// Stand-ins for the modules the policy depends on, in this crate's unit tests.
    mod kartenn_service_fakes {
        use kartenn_service::module::Module;

        #[derive(Debug)]
        pub(super) struct Named(pub &'static str, pub &'static [&'static str]);

        impl Module for Named {
            fn name(&self) -> &'static str {
                self.0
            }
            fn version(&self) -> &'static str {
                "0"
            }
            fn depends_on(&self) -> &'static [&'static str] {
                self.1
            }
        }
    }
}
