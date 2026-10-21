//! Two modules of test for the contributions to the differential (spec 008, REQ-008-26;
//! task 008-19): `alpha` lists every object the changeset writes in its section
//! `alpha.objects`; `beta`, which depends on it, reads that section into its own,
//! `beta.echo`, and its rule refuses a changeset whose section `alpha.objects` names a
//! prefix. Neither has objects of its own.

use std::sync::Arc;

use kartenn_core::rules::{Rule, Severity, Violation};
use kartenn_service::module::{
    BoxFuture, CheckContext, DiffContext, DiffRow, DiffSection, Findings, Module, ModuleHandler,
    ModulePlan, ModuleRegistry, ModuleTx,
};
use kartenn_service::operations::{ModuleOperation, Ref};
use kartenn_service::port::StoreError;
use serde_json::json;
use uuid::Uuid;

/// No prefix in the section of `alpha` (a rule of `beta` on another module's section).
pub(crate) static NO_PREFIX: Rule = Rule {
    id: "beta.alpha.prefix",
    severity: Severity::Error,
    summary: "The section alpha.objects names no prefix.",
    requirements: &["REQ-008-26"],
    source: "008/design",
};

static BETA_RULES: &[&Rule] = &[&NO_PREFIX];

/// Neither module accepts operations.
fn no_operation(operation: &ModuleOperation) -> Violation {
    Violation::new(&NO_PREFIX, format!("no operation {}", operation.name))
}

#[derive(Debug)]
struct AlphaHandler;

impl ModuleHandler for AlphaHandler {
    fn plan(
        &self,
        operation: &ModuleOperation,
        _object: Uuid,
        _resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        Err(no_operation(operation))
    }

    fn apply<'a>(
        &'a self,
        _tx: &'a mut dyn ModuleTx,
        _plan: &'a ModulePlan,
        _period: Option<kartenn_core::period::Period>,
        _changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), kartenn_service::port::WriteError>> {
        Box::pin(std::future::ready(Ok(())))
    }

    fn diff<'a>(
        &'a self,
        _tx: &'a mut dyn ModuleTx,
        context: &'a DiffContext<'a>,
    ) -> BoxFuture<'a, Result<Option<DiffSection>, StoreError>> {
        let rows = context
            .touched
            .iter()
            .map(|(kind, id)| DiffRow {
                object: *id,
                kind: kind.name().to_owned(),
                before: None,
                after: Some(json!({ "kind": kind.name() })),
                period: None,
            })
            .collect();
        Box::pin(std::future::ready(Ok(Some(DiffSection {
            name: "alpha.objects".into(),
            rows,
        }))))
    }
}

#[derive(Debug)]
struct BetaHandler;

impl ModuleHandler for BetaHandler {
    fn plan(
        &self,
        operation: &ModuleOperation,
        _object: Uuid,
        _resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        Err(no_operation(operation))
    }

    fn apply<'a>(
        &'a self,
        _tx: &'a mut dyn ModuleTx,
        _plan: &'a ModulePlan,
        _period: Option<kartenn_core::period::Period>,
        _changeset: Uuid,
    ) -> BoxFuture<'a, Result<(), kartenn_service::port::WriteError>> {
        Box::pin(std::future::ready(Ok(())))
    }

    fn diff<'a>(
        &'a self,
        _tx: &'a mut dyn ModuleTx,
        context: &'a DiffContext<'a>,
    ) -> BoxFuture<'a, Result<Option<DiffSection>, StoreError>> {
        let seen = context.section("alpha.objects").map_or(0, |s| s.rows.len());
        Box::pin(std::future::ready(Ok(Some(DiffSection {
            name: "beta.echo".into(),
            rows: vec![DiffRow {
                object: context.changeset,
                kind: "changeset".into(),
                before: None,
                after: Some(json!({ "alpha_rows": seen })),
                period: None,
            }],
        }))))
    }

    fn check<'a>(
        &'a self,
        _tx: &'a mut dyn ModuleTx,
        context: &'a CheckContext<'a>,
    ) -> BoxFuture<'a, Result<Findings, StoreError>> {
        let prefixes: Vec<Uuid> = context
            .section("alpha.objects")
            .map(|s| {
                s.rows
                    .iter()
                    .filter(|r| r.kind == "prefix")
                    .map(|r| r.object)
                    .collect()
            })
            .unwrap_or_default();
        let violations = if prefixes.is_empty() {
            Vec::new()
        } else {
            vec![Violation::new(&NO_PREFIX, "alpha.objects names a prefix").with_subjects(prefixes)]
        };
        Box::pin(std::future::ready(Ok(violations.into())))
    }
}

#[derive(Debug)]
struct Alpha;

impl Module for Alpha {
    fn name(&self) -> &'static str {
        "alpha"
    }
    fn version(&self) -> &'static str {
        "0.0.1"
    }
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        Some(Arc::new(AlphaHandler))
    }
}

#[derive(Debug)]
struct Beta;

impl Module for Beta {
    fn name(&self) -> &'static str {
        "beta"
    }
    fn version(&self) -> &'static str {
        "0.0.1"
    }
    fn depends_on(&self) -> &'static [&'static str] {
        &["alpha"]
    }
    fn rules(&self) -> &'static [&'static Rule] {
        BETA_RULES
    }
    fn handler(&self) -> Option<Arc<dyn ModuleHandler>> {
        Some(Arc::new(BetaHandler))
    }
}

/// The registry of a build with both modules, given in the reverse of their order.
pub(crate) fn registry() -> Arc<ModuleRegistry> {
    Arc::new(ModuleRegistry::new(vec![Arc::new(Beta), Arc::new(Alpha)]).expect("a valid registry"))
}
