//! The flow registry module as the core sees it (ADR 0026): its type of object,
//! permissions, rules and migrations. Its operations, references and handler in the
//! write pipeline come with the tasks of phase P2.

use std::sync::Arc;

use kartenn_core::authz::Permission;
use kartenn_core::rules::Rule;
use kartenn_service::module::{Module, ModuleHandler, ModuleMigration, ObjectType};

use crate::rules::FLOW_RULES;

/// The qualified type of a flow.
pub const FLOW: &str = "flow.flow";

static TYPES: &[ObjectType] = &[
    // A flow belongs to an entity: `core.owner`, required (design « Données »).
    ObjectType {
        kind: FLOW,
        collection: "flows",
        parent: None,
        owner_required: true,
        carries_site: false,
        domain_anchored: false,
    },
];

static MIGRATIONS: &[ModuleMigration] = &[ModuleMigration {
    version: 1,
    name: "flow",
    sql: include_str!("../migrations/0001_flow.sql"),
}];

macro_rules! permissions {
    ($list:ident; $($name:ident = $id:literal, $summary:literal;)*) => {
        $(
            static $name: Permission = Permission {
                name: $id,
                summary: $summary,
                global_only: false,
            };
        )*
        static $list: &[&Permission] = &[$(&$name),*];
    };
}

// REQ-011-17: all carried by the entity of the flow.
permissions! {
    PERMISSIONS;
    FLOW_READ = "flow.flow.read", "Lire les flux.";
    FLOW_REQUEST = "flow.flow.request",
        "Demander un flux sans lire ses extrémités, et lire ses propres demandes.";
    FLOW_REVIEW = "flow.flow.review", "Instruire une demande de flux.";
    FLOW_WAIVE = "flow.flow.waive", "Accorder une dérogation à un flux refusé.";
    FLOW_WRITE = "flow.flow.write",
        "Écrire directement un flux (maintenance), avec la lecture de ses deux extrémités.";
}

/// The flow registry module (spec 011).
#[derive(Debug, Clone, Copy, Default)]
pub struct Flows;

impl Module for Flows {
    fn name(&self) -> &'static str {
        "flow"
    }
    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
    fn object_types(&self) -> &'static [ObjectType] {
        TYPES
    }
    fn permissions(&self) -> &'static [&'static Permission] {
        PERMISSIONS
    }
    fn rules(&self) -> &'static [&'static Rule] {
        FLOW_RULES
    }
    fn migrations(&self) -> &'static [ModuleMigration] {
        MIGRATIONS
    }
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        None
    }
}

#[cfg(test)]
mod tests {
    use kartenn_service::module::ModuleRegistry;

    use super::*;

    #[test]
    fn the_module_registers() {
        let registry = ModuleRegistry::new(vec![Arc::new(Flows)]).expect("a valid module");
        assert_eq!(registry.object_types().count(), 1);
        assert!(
            registry
                .permissions()
                .iter()
                .all(|p| p.name.starts_with("flow."))
        );
    }
}
