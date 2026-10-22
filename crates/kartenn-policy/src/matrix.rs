//! The matrix view of the rules (spec 010, REQ-010-34, S28, N36): environment of the source
//! × environment of the destination × flow type, three states. A cell is a rule that cites
//! one value of `policy.environment` on each side and one flow type, nothing else; the
//! view reads them from a snapshot and writes them by `policy.matrix.set`, translated into
//! the creation, the update or the closure of that rule. Every other rule is out of the
//! view, and counted.

use std::collections::BTreeSet;

use crate::model::{AxisKey, AxisValue, Effect, Key, Rule};
use crate::module::ENVIRONMENT;
use crate::snapshot::PolicySnapshot;

/// The state of a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CellState {
    /// An authorisation.
    Allow,
    /// A refusal.
    Deny,
    /// An authorisation that requires one of its patterns.
    Conditional,
}

impl CellState {
    /// Its name in the contract.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Conditional => "conditional",
        }
    }
}

/// One cell of the view: the rules that make it, and their combined state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// Environment of the source.
    pub source: AxisValue,
    /// Environment of the destination.
    pub destination: AxisValue,
    /// Combined state: a deny wins, then an unconditional allow (D28, S23).
    pub state: CellState,
    /// The patterns one of which a conditional cell requires.
    pub patterns: BTreeSet<Key>,
    /// The rules of the cell (usually one).
    pub rules: Vec<Key>,
}

/// The view of one flow type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatrixView {
    /// The flow type.
    pub flow_type: Key,
    /// Its cells, by source then destination.
    pub cells: Vec<Cell>,
    /// The rules that apply to the flow type but are out of the view (multi-axis,
    /// several values, a service, « same value », a pattern member).
    pub outside: usize,
}

/// The one value of the environment a selector admits, when it admits nothing else.
fn single_environment(selector: &crate::model::Selector) -> Option<&AxisValue> {
    let environment = AxisKey::parse(ENVIRONMENT).ok()?;
    if selector.len() != 1 {
        return None;
    }
    let values = selector.get(&environment)?;
    if values.len() == 1 {
        values.iter().next()
    } else {
        None
    }
}

/// The cell `rule` makes for `flow_type`, if it is a rule of the view.
#[must_use]
pub fn cell_of(rule: &Rule, flow_type: &Key) -> Option<(AxisValue, AxisValue)> {
    let view = rule.same.is_empty()
        && rule.pattern.is_none()
        && rule.flows.services.is_empty()
        && rule.flows.flow_types.len() == 1
        && rule.flows.flow_types.contains(flow_type);
    if !view {
        return None;
    }
    Some((
        single_environment(&rule.source)?.clone(),
        single_environment(&rule.destination)?.clone(),
    ))
}

/// Whether `rule` applies to flows of `flow_type`, by the type or by one of its services.
fn applies(snapshot: &PolicySnapshot, rule: &Rule, flow_type: &Key) -> bool {
    rule.flows.flow_types.contains(flow_type)
        || rule.flows.services.iter().any(|s| {
            snapshot
                .service(s)
                .is_some_and(|s| &s.flow_type == flow_type)
        })
}

/// The matrix view of `flow_type` in `snapshot` (REQ-010-34).
#[must_use]
pub fn view(snapshot: &PolicySnapshot, flow_type: &Key) -> MatrixView {
    let mut cells: Vec<Cell> = Vec::new();
    let mut outside = 0;
    for rule in snapshot.rules() {
        let Some((source, destination)) = cell_of(rule, flow_type) else {
            if applies(snapshot, rule, flow_type) {
                outside += 1;
            }
            continue;
        };
        let state = match rule.effect {
            Effect::Deny => CellState::Deny,
            Effect::Allow if rule.required_patterns.is_empty() => CellState::Allow,
            Effect::Allow => CellState::Conditional,
        };
        if let Some(cell) = cells
            .iter_mut()
            .find(|c| c.source == source && c.destination == destination)
        {
            cell.rules.push(rule.key.clone());
            // A deny wins; an unconditional allow wins over a conditional one.
            match (cell.state, state) {
                (CellState::Deny, _) | (CellState::Allow, CellState::Conditional) => {}
                (CellState::Conditional, CellState::Conditional) => {
                    cell.patterns.extend(rule.required_patterns.iter().cloned());
                }
                _ => {
                    cell.state = state;
                    cell.patterns = if state == CellState::Conditional {
                        rule.required_patterns.clone()
                    } else {
                        BTreeSet::new()
                    };
                }
            }
        } else {
            cells.push(Cell {
                source,
                destination,
                state,
                patterns: if state == CellState::Conditional {
                    rule.required_patterns.clone()
                } else {
                    BTreeSet::new()
                },
                rules: vec![rule.key.clone()],
            });
        }
    }
    cells.sort_by(|a, b| {
        (a.source.as_str(), a.destination.as_str())
            .cmp(&(b.source.as_str(), b.destination.as_str()))
    });
    MatrixView {
        flow_type: flow_type.clone(),
        cells,
        outside,
    }
}

/// The fingerprint of `view`, the strong `ETag` of its matrix (S35): 32 hexadecimal digits of
/// the SHA-256 of its members in order, which fix the bytes the route serves.
#[cfg(feature = "pg")]
#[must_use]
pub fn fingerprint(view: &MatrixView) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;
    let cells: Vec<serde_json::Value> = view
        .cells
        .iter()
        .map(|c| {
            serde_json::json!([
                c.source.as_str(),
                c.destination.as_str(),
                c.state.as_str(),
                c.patterns.iter().map(Key::as_str).collect::<Vec<_>>(),
                c.rules.iter().map(Key::as_str).collect::<Vec<_>>(),
            ])
        })
        .collect();
    let canonical = serde_json::json!([view.flow_type.as_str(), view.outside, cells]);
    let digest = Sha256::digest(canonical.to_string().as_bytes());
    digest[..16].iter().fold(String::new(), |mut hex, b| {
        let _ = write!(hex, "{b:02x}");
        hex
    })
}

#[cfg(feature = "pg")]
pub(crate) use write::{check_set, plan_set, prepare_set};

/// `policy.matrix.set`: the write of a cell, translated into an operation on its rule.
#[cfg(feature = "pg")]
mod write {
    use kartenn_core::rules::Violation;
    use kartenn_service::module::ModulePlan;
    use kartenn_service::operations::{ModuleOperation, Ref};
    use kartenn_service::port::WriteError;
    use kartenn_service::{ObjectKind, Operation};
    use serde::{Deserialize, Serialize};
    use serde_json::{Map, Value, json};
    use sqlx::PgConnection;
    use sqlx::types::time::OffsetDateTime;
    use uuid::Uuid;

    use super::Key;
    use crate::codec;
    use crate::handler::{invalid, plan};
    use crate::model::Label;
    use crate::module::{ENVIRONMENT, RULE};
    use crate::rules::{MATRIX_STALE, RULE_CONDITIONAL_DENY};

    /// What the transaction tells of the cell before the plan.
    #[derive(Debug, Default, Serialize, Deserialize)]
    struct Completed {
        /// The rule of the cell, when one lives at the date.
        existing: Option<Uuid>,
        /// Under `if_match`: the revision of the policy the matrix was compared at, which
        /// the write confirms under the key `policy` (S35).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        revision: Option<u64>,
        /// Under `if_match`: the matrix of the flow type no longer has that tag.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        stale: bool,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Set {
        #[serde(default)]
        local: Option<u32>,
        source: String,
        destination: String,
        flow_type: Ref,
        state: String,
        #[serde(default)]
        patterns: Vec<Ref>,
        #[serde(default)]
        texts: Option<Value>,
        #[serde(default)]
        label: Option<String>,
        /// The `ETag` of the matrix of the flow type read at present (S35); none: no check.
        #[serde(default)]
        if_match: Option<String>,
        #[serde(default)]
        completed: Completed,
    }

    fn decode(operation: &ModuleOperation) -> Result<Set, Violation> {
        serde_json::from_value(Value::Object(operation.body.clone()))
            .map_err(|e| invalid(operation, e))
    }

    /// The rule of the cell living at `at`: the first by key when several make it.
    async fn existing(
        conn: &mut PgConnection,
        source: &str,
        destination: &str,
        flow_type: Uuid,
        at: Option<OffsetDateTime>,
    ) -> Result<Option<Uuid>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT id FROM policy.policy_rule
             WHERE valid_period @> coalesce($1::timestamptz, now())
               AND source = jsonb_build_object($2::text, jsonb_build_array($3::text))
               AND destination = jsonb_build_object($2::text, jsonb_build_array($4::text))
               AND same = '{}' AND service_ids = '{}' AND pattern_id IS NULL
               AND flow_type_ids = ARRAY[$5::uuid]
             ORDER BY key LIMIT 1",
        )
        .bind(at)
        .bind(ENVIRONMENT)
        .bind(source)
        .bind(destination)
        .bind(flow_type)
        .fetch_optional(conn)
        .await
    }

    /// Whether the matrix of `flow_type` at present has the tag `if_match` (S35), and the
    /// revision of the policy read before it. Every operation of the changeset is
    /// prepared before the first is applied: the matrix compared is the one before the
    /// changeset. A flow type absent at present has no matrix: the tag is stale.
    async fn compared(
        conn: &mut PgConnection,
        flow_type: Option<Ref>,
        if_match: &str,
    ) -> Result<(u64, bool), kartenn_service::StoreError> {
        let revision = crate::read::revision(conn).await?;
        let Some(Ref::Id(flow_type)) = flow_type else {
            return Ok((revision, true));
        };
        let found: Option<(String, OffsetDateTime)> = sqlx::query_as(
            "SELECT key, now() FROM policy.policy_flow_type
             WHERE id = $1 AND valid_period @> now()",
        )
        .bind(flow_type)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| crate::pg::store(&e))?;
        let Some((key, now)) = found else {
            return Ok((revision, true));
        };
        let Ok(key) = Key::new(&key) else {
            return Ok((revision, true));
        };
        let snapshot = crate::cache::shared().snapshot(conn, now, None).await?;
        let tag = if_match
            .strip_prefix('"')
            .and_then(|t| t.strip_suffix('"'))
            .unwrap_or(if_match);
        Ok((
            revision,
            super::fingerprint(&super::view(&snapshot, &key)) != tag,
        ))
    }

    /// Completes `policy.matrix.set` with the rule of its cell, read in the transaction,
    /// and, under `if_match`, with the comparison of the matrix (S35). A flow type created
    /// in the same changeset has no rule yet.
    pub(crate) async fn prepare_set(
        conn: &mut PgConnection,
        operation: &mut ModuleOperation,
        at: Option<OffsetDateTime>,
    ) -> Result<(), kartenn_service::StoreError> {
        let text = |name: &str| {
            operation
                .body
                .get(name)
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        let flow_type = operation
            .body
            .get("flow_type")
            .and_then(|v| serde_json::from_value::<Ref>(v.clone()).ok());
        let mut completed = Completed::default();
        if let Some(if_match) = text("if_match") {
            let (revision, stale) = compared(conn, flow_type, &if_match).await?;
            completed.revision = Some(revision);
            completed.stale = stale;
        }
        if let (Some(source), Some(destination), Some(Ref::Id(flow_type))) =
            (text("source"), text("destination"), flow_type)
        {
            completed.existing = existing(conn, &source, &destination, flow_type, at)
                .await
                .map_err(|e| crate::pg::store(&e))?;
        }
        operation.body.insert(
            "completed".into(),
            serde_json::to_value(&completed).expect("a JSON object"),
        );
        Ok(())
    }

    /// The plan of `policy.matrix.set` (REQ-010-34): the creation, the update or the
    /// closure of the rule of the cell.
    pub(crate) fn plan_set(
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
    ) -> Result<ModulePlan, Violation> {
        let set = decode(operation)?;
        let _ = (set.local, &set.if_match);
        if set.completed.stale {
            return Err(stale(operation));
        }
        let mut planned = planned_set(operation, object, resolve, set)?;
        if let (Some(revision), Value::Object(data)) = (set_revision(operation), &mut planned.data)
        {
            data.insert("revision".into(), revision.into());
        }
        Ok(planned)
    }

    /// `policy.matrix.stale` for `operation`.
    fn stale(operation: &ModuleOperation) -> Violation {
        Violation::new(
            &MATRIX_STALE,
            format!(
                "{}: the matrix of the flow type changed since the ETag of if_match",
                operation.tag()
            ),
        )
    }

    /// The revision of the policy the matrix of `operation` was compared at.
    fn set_revision(operation: &ModuleOperation) -> Option<u64> {
        operation
            .body
            .get("completed")
            .and_then(|c| c.get("revision"))
            .and_then(Value::as_u64)
    }

    /// Under the key `policy`, before the write of the cell: the policy has the revision
    /// its matrix was compared at, less the one this changeset raised (S35). Another
    /// changeset that wrote the policy in between makes the tag stale.
    ///
    /// # Errors
    /// `policy.matrix.stale`; a failure of the database.
    pub(crate) async fn check_set(
        conn: &mut PgConnection,
        data: &Value,
        changeset: Uuid,
    ) -> Result<(), WriteError> {
        let Some(compared) = data.get("revision").and_then(Value::as_u64) else {
            return Ok(());
        };
        let before: i64 = sqlx::query_scalar(
            "SELECT policy - CASE WHEN policy_by = $1 THEN 1 ELSE 0 END
               FROM policy.policy_revision",
        )
        .bind(changeset)
        .fetch_one(conn)
        .await
        .map_err(|e| WriteError::Store(crate::pg::store(&e)))?;
        if u64::try_from(before).ok() == Some(compared) {
            Ok(())
        } else {
            Err(WriteError::Rule(Violation::new(
                &MATRIX_STALE,
                "the matrix of the flow type changed since the ETag of if_match",
            )))
        }
    }

    /// The plan of the cell itself, once its tag is checked.
    fn planned_set(
        operation: &ModuleOperation,
        object: Uuid,
        resolve: &dyn Fn(Ref) -> Option<Uuid>,
        set: Set,
    ) -> Result<ModulePlan, Violation> {
        for value in [&set.source, &set.destination] {
            if value.is_empty() {
                return Err(invalid(operation, "an environment is a value key"));
            }
        }
        let flow_type =
            resolve(set.flow_type).ok_or_else(|| invalid(operation, "unknown flow type"))?;
        let patterns = set
            .patterns
            .into_iter()
            .map(|r| {
                resolve(r)
                    .map(|id| id.to_string())
                    .ok_or_else(|| invalid(operation, "unknown pattern"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let effect = match set.state.as_str() {
            "allow" | "conditional" => "allow",
            "deny" => "deny",
            "none" => {
                // The cell emptied: its rule closed by the core (D33 checks its referrers).
                let existing = set
                    .completed
                    .existing
                    .ok_or_else(|| invalid(operation, "the cell is empty"))?;
                let mut planned = plan(RULE, existing, false, json!({ "noop": true }));
                planned.core.push(Operation::Close {
                    kind: ObjectKind::from_name(RULE).expect("a module type"),
                    id: Ref::Id(existing),
                    expected: None,
                    cascade: false,
                });
                return Ok(planned);
            }
            _ => {
                return Err(invalid(
                    operation,
                    "state is allow, deny, conditional or none",
                ));
            }
        };
        match (set.state.as_str(), patterns.is_empty()) {
            ("conditional", true) => {
                return Err(invalid(operation, "a conditional cell requires patterns"));
            }
            ("deny", false) => {
                return Err(Violation::new(
                    &RULE_CONDITIONAL_DENY,
                    format!("{}: a deny requires no pattern", operation.tag()),
                ));
            }
            ("allow", false) => {
                return Err(invalid(operation, "patterns make a conditional cell"));
            }
            _ => {}
        }
        let mut columns = Map::new();
        columns.insert("effect".into(), effect.into());
        columns.insert("required_pattern_ids".into(), patterns.into());
        if let Some(texts) = &set.texts {
            columns.insert("texts".into(), codec::texts(texts)?.1);
        }
        if let Some(existing) = set.completed.existing {
            return Ok(plan(RULE, existing, false, json!({ "update": columns })));
        }
        let label = set
            .label
            .unwrap_or_else(|| format!("{} → {}", set.source, set.destination));
        let label = Label::new(&label)?.as_str().to_owned();
        let mut row = json!({
            "id": object,
            "key": format!("matrix-{}", object.simple()),
            "label": label,
            "source": { ENVIRONMENT: [set.source] },
            "destination": { ENVIRONMENT: [set.destination] },
            "same": [],
            "flow_type_ids": [flow_type.to_string()],
            "service_ids": [],
            "pattern_id": null,
            "texts": {},
        });
        if let Value::Object(row) = &mut row {
            row.extend(columns);
        }
        Ok(plan(RULE, object, true, json!({ "create": row })))
    }
}
