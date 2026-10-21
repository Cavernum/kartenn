//! The policy module as the core sees it (ADR 0026): its types of objects, permissions,
//! rules, operations, references and migrations, its property declarations, and its
//! handler in the write pipeline (feature `pg`).

use std::collections::BTreeSet;
use std::sync::Arc;

use kartenn_core::authz::Permission;
use kartenn_core::properties::{Declarant, Declaration, InheritTree, PropertyKey, TypeSpec};
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

static TYPES: &[ObjectType] = &[
    ObjectType {
        kind: AXIS,
        collection: "policy-axes",
        parent: None,
        owner_required: false,
        carries_site: false,
        domain_anchored: false,
    },
    ObjectType {
        kind: VALUE,
        collection: "policy-values",
        parent: None,
        owner_required: false,
        carries_site: false,
        domain_anchored: false,
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
];

/// The references of the module (D33): an axis keeps its property; the attributes of a
/// value go with it when the value is closed with the cascade (REQ-010-2).
static REFERENCES: &[Reference] = &[
    Reference {
        from: AXIS,
        to: "property",
        on_close: OnClose::Refuse,
    },
    Reference {
        from: VALUE,
        to: "enum_value",
        on_close: OnClose::Cascade,
    },
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

// The objects of the policy have no anchor (S15): their writes are granted at the global
// scope only; reading them is a right of the delivered reader roles.
permissions! {
    PERMISSIONS;
    AXIS_READ = "policy.axis.read", false, "Lire les axes de la politique.";
    AXIS_WRITE = "policy.axis.write", true, "Désigner, modifier ou retirer un axe de la politique.";
    AXIS_PROPOSE = "policy.axis.propose", true, "Proposer un changement d'un axe de la politique.";
    VALUE_READ = "policy.value.read", false, "Lire les attributs des valeurs des axes.";
    VALUE_WRITE = "policy.value.write", true, "Décrire une valeur d'axe, créer ou clore un environnement.";
    VALUE_PROPOSE = "policy.value.propose", true, "Proposer un changement des attributs d'une valeur d'axe.";
}

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
                declaration(
                    ENVIRONMENT,
                    TypeSpec::ENUM,
                    &TREE,
                    Some(InheritTree::Addressing),
                ),
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
