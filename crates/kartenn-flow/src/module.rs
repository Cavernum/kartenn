//! The flow registry module as the core sees it (ADR 0026): its type of object,
//! permissions, rules, migrations and the references of its endpoints (REQ-011-3). Its
//! operations come with task 011-3.

use std::sync::Arc;

use kartenn_core::authz::Permission;
use kartenn_core::rules::Rule;
use kartenn_service::module::{
    Module, ModuleHandler, ModuleMigration, ObjectType, OnClose, OperationType, Reference,
};

use crate::handler::Handler;
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

// REQ-011-3: an endpoint of a living flow is not closed; the cascade closes the flow
// (D33). The interface and the machine come with the module Machines, a dependency of the
// policy this module depends on.
static REFERENCES: &[Reference] = &[
    Reference {
        from: FLOW,
        to: "address",
        on_close: OnClose::Cascade,
    },
    Reference {
        from: FLOW,
        to: "prefix",
        on_close: OnClose::Cascade,
    },
    Reference {
        from: FLOW,
        to: "machines.interface",
        on_close: OnClose::Cascade,
    },
    Reference {
        from: FLOW,
        to: "machines.machine",
        on_close: OnClose::Cascade,
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
    fn depends_on(&self) -> &'static [&'static str] {
        &["policy", "machines"]
    }
    fn object_types(&self) -> &'static [ObjectType] {
        TYPES
    }
    fn references(&self) -> &'static [Reference] {
        REFERENCES
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
    fn operations(&self) -> &'static [OperationType] {
        OPERATIONS
    }
    fn review_operations(&self) -> &'static [&'static str] {
        // The validator of a request joins its approval and, for a denied flow, its waiver
        // (REQ-011-22, REQ-016-11).
        &["approve_review", "waive"]
    }
    fn operation_permissions(&self) -> &'static [(&'static str, &'static str)] {
        // REQ-011-17, F12: requesting needs no read of the endpoints; the direct write of
        // maintenance (`register`, `update`) is `flow.flow.write`, the default.
        &[
            ("request", "flow.flow.request"),
            ("waive", "flow.flow.waive"),
            ("approve_review", "flow.flow.review"),
        ]
    }
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        Some(Arc::new(Handler))
    }
}

// The operations of the module (design 011 « Opérations et règles »); a flow is closed by
// the generic `Close` of the core, its references by the cascade (REQ-011-3).
static OPERATIONS: &[OperationType] = &[
    OperationType {
        name: "request",
        defines: Some(FLOW),
    },
    OperationType {
        name: "register",
        defines: Some(FLOW),
    },
    OperationType {
        name: "update",
        defines: None,
    },
    OperationType {
        name: "waive",
        defines: None,
    },
    OperationType {
        name: "approve_review",
        defines: None,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    // The registry with the modules it depends on is checked by the integration tests.
    #[test]
    fn the_module_declares_its_namespace() {
        assert_eq!(Flows.object_types().len(), 1);
        assert!(
            Flows
                .permissions()
                .iter()
                .all(|p| p.name.starts_with("flow."))
        );
        assert!(Flows.references().iter().all(|r| r.from == FLOW));
    }
}
