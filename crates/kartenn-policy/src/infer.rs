//! The inference of expected classifications in the store and the write pipeline (spec
//! 010, REQ-010-40, 41, 46, 50 to 52; task 010-15): the operations on the rules of
//! inference, their stored form, the rules read at a date in the transaction, the chain
//! of a node of the addressing tree, and the source of expected values `policy.inference`
//! that the core calls for suggestions and default values (contract 003 amended by the
//! 016: context, batch, several candidates).

use std::collections::{BTreeMap, BTreeSet};

use kartenn_core::addr::{Family, Prefix};
use kartenn_core::family::{IPV4, IPV6};
use kartenn_core::period::Instant;
use kartenn_core::rules::Violation;
use kartenn_service::expected::{Expected, ExpectedContext, ExpectedValueSource, Suggestion};
use kartenn_service::module::{BoxFuture, ModulePlan, ModuleTx};
use kartenn_service::operations::{ModuleOperation, RawInput, Ref};
use kartenn_service::port::{ObjectView, StoreError};
use serde_json::{Map, Value, json};
use sqlx::PgConnection;
use sqlx::types::time::OffsetDateTime;
use uuid::Uuid;

use crate::handler::{invalid, plan};
use crate::inference::{
    Criterion, Expectation, InferenceRule, NameRegex, Node, NodeKind, PropertyTest, expected,
};
use crate::model::{AxisKey, AxisValue, Key};
use crate::module::INFERENCE_RULE;
use crate::pg::store;
use crate::rules::{AXIS_INVALID, INFERENCE_CRITERION_INVALID, VALUE_UNKNOWN};

/// The identifier of the source, as the declaration of `policy.environment` names it.
pub const SOURCE: &str = "policy.inference";

/// The families a criterion may name.
const FAMILIES: [Family; 2] = [IPV4, IPV6];

fn criterion_invalid(why: impl std::fmt::Display) -> Violation {
    Violation::new(&INFERENCE_CRITERION_INVALID, why.to_string())
}

fn strings(value: &Value, name: &str) -> Result<Vec<String>, Violation> {
    value
        .as_array()
        .ok_or_else(|| criterion_invalid(format!("{name} takes a list")))?
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| criterion_invalid(format!("{name} takes texts")))
        })
        .collect()
}

fn prefixes(value: &Value, name: &str) -> Result<Vec<Prefix>, Violation> {
    strings(value, name)?
        .iter()
        .map(|p| {
            Prefix::parse_any(&FAMILIES, p)
                .map_err(|e| criterion_invalid(format!("{name}: {p}: {e}")))
        })
        .collect()
}

fn uuids(value: &Value, name: &str) -> Result<Vec<Uuid>, Violation> {
    strings(value, name)?
        .iter()
        .map(|u| Uuid::parse_str(u).map_err(|_| criterion_invalid(format!("{name}: {u} is no id"))))
        .collect()
}

fn values(value: &Value, name: &str) -> Result<BTreeSet<AxisValue>, Violation> {
    Ok(strings(value, name)?
        .into_iter()
        .map(AxisValue::new)
        .collect())
}

/// One criterion from its stored form, an object of one member (REQ-010-42):
/// `within`, `equals`, `family`, `domain`, `kind`, `site`, `entity`, `property`
/// (`{key, equals | in | present}`), `name`, `prefix_length`.
///
/// # Errors
/// `policy.inference.criterion_invalid`, or `policy.inference.regex_invalid` for a name.
pub fn criterion(value: &Value) -> Result<Criterion, Violation> {
    let (name, arg) = value
        .as_object()
        .filter(|o| o.len() == 1)
        .and_then(|o| o.iter().next())
        .ok_or_else(|| criterion_invalid("a criterion is an object of one member"))?;
    Ok(match name.as_str() {
        "within" => Criterion::Within(prefixes(arg, name)?),
        "equals" => Criterion::Equals(prefixes(arg, name)?),
        "family" => Criterion::Family(
            strings(arg, name)?
                .iter()
                .map(|f| {
                    FAMILIES
                        .into_iter()
                        .find(|x| x.id() == f)
                        .ok_or_else(|| criterion_invalid(format!("family {f} is unknown")))
                })
                .collect::<Result<_, _>>()?,
        ),
        "domain" => Criterion::Domain(uuids(arg, name)?),
        "site" => Criterion::Site(uuids(arg, name)?),
        "entity" => Criterion::Entity(uuids(arg, name)?),
        "kind" => Criterion::Kind(
            strings(arg, name)?
                .iter()
                .map(|k| match k.as_str() {
                    "domain" => Ok(NodeKind::Domain),
                    "container" => Ok(NodeKind::Container),
                    "network" => Ok(NodeKind::Network),
                    "address" => Ok(NodeKind::Address),
                    other => Err(criterion_invalid(format!("kind {other} is unknown"))),
                })
                .collect::<Result<_, _>>()?,
        ),
        "name" => Criterion::Name(
            strings(arg, name)?
                .iter()
                .map(|r| NameRegex::new(r))
                .collect::<Result<_, _>>()?,
        ),
        "prefix_length" => Criterion::PrefixLength(
            arg.as_array()
                .ok_or_else(|| criterion_invalid("prefix_length takes a list"))?
                .iter()
                .map(|l| {
                    l.as_u64()
                        .and_then(|l| u8::try_from(l).ok())
                        .filter(|l| *l <= 128)
                        .ok_or_else(|| criterion_invalid(format!("prefix length {l}")))
                })
                .collect::<Result<_, _>>()?,
        ),
        "property" => {
            let test = arg
                .as_object()
                .ok_or_else(|| criterion_invalid("property takes an object"))?;
            let key = test
                .get("key")
                .and_then(Value::as_str)
                .and_then(|k| AxisKey::parse(k).ok())
                .ok_or_else(|| criterion_invalid("property needs a property key"))?;
            let op = if let Some(v) = test.get("equals") {
                PropertyTest::Equals(values(v, "equals")?)
            } else if let Some(v) = test.get("in") {
                PropertyTest::In(values(v, "in")?)
            } else if test.get("present") == Some(&Value::Bool(true)) {
                PropertyTest::Present
            } else {
                return Err(criterion_invalid("property takes equals, in or present"));
            };
            Criterion::Property(key, op)
        }
        other => return Err(criterion_invalid(format!("criterion {other} is unknown"))),
    })
}

/// The criteria of a stored list.
fn criteria(value: &Value) -> Result<Vec<Criterion>, Violation> {
    value
        .as_array()
        .ok_or_else(|| criterion_invalid("criteria are a list"))?
        .iter()
        .map(criterion)
        .collect()
}

/// A stored rule of inference: id, key, axis, parent, priority, criteria, values, domain.
type StoredRule = (
    Uuid,
    String,
    String,
    Option<Uuid>,
    i32,
    Value,
    Vec<String>,
    Option<Uuid>,
);

/// A version a changeset wrote: id, start, axis, parent, values, criteria.
type WrittenRule = (
    Uuid,
    OffsetDateTime,
    String,
    Option<Uuid>,
    Vec<String>,
    Value,
);

/// A stored rule of inference, as read.
struct Row {
    id: Uuid,
    parent: Option<Uuid>,
    rule: InferenceRule,
}

/// The rules of inference valid at `at` (now when `None`) in the transaction, as trees of
/// exceptions under their roots; a rule that cites a domain applies only there (REQ-010-46).
/// A stored row that does not read back is left out.
///
/// # Errors
/// A failure of the database.
pub async fn rules_at(
    conn: &mut PgConnection,
    at: Option<OffsetDateTime>,
) -> Result<Vec<InferenceRule>, StoreError> {
    let rows: Vec<StoredRule> = sqlx::query_as(
        "SELECT id, key, axis_key, parent_id, priority, criteria, value_keys, domain_id
               FROM policy.policy_inference_rule
              WHERE valid_period @> coalesce($1::timestamptz, now())",
    )
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut read: Vec<Row> = Vec::new();
    for (id, key, axis, parent, priority, stored, keys, domain) in rows {
        let (Ok(key), Ok(axis), Ok(mut criteria)) =
            (Key::new(&key), AxisKey::parse(&axis), criteria(&stored))
        else {
            continue;
        };
        if let Some(domain) = domain {
            criteria.push(Criterion::Domain(vec![domain]));
        }
        read.push(Row {
            id,
            parent,
            rule: InferenceRule {
                key,
                axis,
                priority,
                criteria,
                values: keys.into_iter().map(AxisValue::new).collect(),
                exceptions: Vec::new(),
            },
        });
    }
    Ok(trees(read))
}

/// `row` with its exceptions, taken from `children`.
fn build(row: Row, children: &mut BTreeMap<Uuid, Vec<Row>>, depth: usize) -> InferenceRule {
    let mut rule = row.rule;
    // A cycle cannot be stored (a parent lives before its child); the bound is a guard.
    if depth <= crate::inference::MAX_DEPTH + 1 {
        for child in children.remove(&row.id).unwrap_or_default() {
            rule.exceptions.push(build(child, children, depth + 1));
        }
    }
    rule
}

/// The rows nested under their parents; a row whose parent is missing is a root.
fn trees(rows: Vec<Row>) -> Vec<InferenceRule> {
    let ids: BTreeSet<Uuid> = rows.iter().map(|r| r.id).collect();
    let mut children: BTreeMap<Uuid, Vec<Row>> = BTreeMap::new();
    let mut roots = Vec::new();
    for row in rows {
        match row.parent.filter(|p| ids.contains(p)) {
            Some(parent) => children.entry(parent).or_default().push(row),
            None => roots.push(row),
        }
    }
    roots
        .into_iter()
        .map(|r| build(r, &mut children, 1))
        .collect()
}

/// The closed-list values of the heritable or own properties `keys` resolved on `object`.
async fn resolved(
    conn: &mut PgConnection,
    object: Uuid,
    keys: &[String],
    at: Option<OffsetDateTime>,
) -> Result<BTreeMap<AxisKey, BTreeSet<AxisValue>>, StoreError> {
    if keys.is_empty() {
        return Ok(BTreeMap::new());
    }
    let rows: Vec<(String, Vec<String>)> = sqlx::query_as(
        "WITH t AS (SELECT coalesce($3::timestamptz, now()) AS at)
         SELECT d.key, coalesce(array_agg(e.key ORDER BY e.key) FILTER (WHERE e.key IS NOT NULL), '{}')
           FROM t, property_declaration d
           JOIN property_assignment a ON a.property_id = d.id
           LEFT JOIN property_enum_value e ON e.id = a.v_uuid OR e.id = ANY(a.v_uuids)
          WHERE d.key = ANY($2) AND d.valid_period @> t.at AND a.valid_period @> t.at
            AND a.kind = 'value'
            AND a.object_id = CASE WHEN d.inherit_tree IS NULL THEN $1 ELSE
                (SELECT o.origin_object_id FROM property_origin o
                  WHERE o.object_id = $1 AND o.property_id = d.id AND o.valid_period @> t.at)
                END
          GROUP BY d.key",
    )
    .bind(object)
    .bind(keys)
    .bind(at)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    Ok(rows
        .into_iter()
        .filter_map(|(k, v)| {
            Some((
                AxisKey::parse(&k).ok()?,
                v.into_iter().map(AxisValue::new).collect(),
            ))
        })
        .collect())
}

/// The resolved reference `key` (`core.site`, `core.owner`) of `object` and the ancestors
/// of what it designates in `table` (`site`, `entity`).
async fn lineage(
    conn: &mut PgConnection,
    object: Uuid,
    key: &str,
    table: &str,
    at: Option<OffsetDateTime>,
) -> Result<Vec<Uuid>, StoreError> {
    let sql = format!(
        "WITH RECURSIVE t AS (SELECT coalesce($3::timestamptz, now()) AS at),
         start AS (
           SELECT a.v_uuid AS id FROM t, property_declaration d
             JOIN property_assignment a ON a.property_id = d.id
            WHERE d.key = $2 AND d.valid_period @> t.at AND a.valid_period @> t.at
              AND a.kind = 'value' AND a.v_uuid IS NOT NULL
              AND a.object_id = coalesce(
                (SELECT o.origin_object_id FROM property_origin o
                  WHERE o.object_id = $1 AND o.property_id = d.id AND o.valid_period @> t.at), $1)),
         up(id, n) AS (
           SELECT id, 0 FROM start
           UNION ALL
           SELECT x.parent_id, up.n + 1 FROM up
             JOIN {table} x ON x.id = up.id
            WHERE x.valid_period @> (SELECT at FROM t)
              AND x.parent_id IS NOT NULL AND up.n < 64)
         SELECT id FROM up ORDER BY n"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(object)
        .bind(key)
        .bind(at)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| store(&e))
}

/// The keys of the properties the criteria of `rules` test.
fn tested(rules: &[InferenceRule]) -> Vec<String> {
    fn walk(rule: &InferenceRule, out: &mut BTreeSet<String>) {
        for c in &rule.criteria {
            if let Criterion::Property(key, _) = c {
                out.insert(key.to_string());
            }
        }
        for e in &rule.exceptions {
            walk(e, out);
        }
    }
    let mut out = BTreeSet::new();
    for r in rules {
        walk(r, &mut out);
    }
    out.into_iter().collect()
}

/// The chain of `view` in its tree, root first (REQ-010-41): its domain, the prefixes of
/// its domain that contain it, by length, then itself. A range is not a node.
async fn chain(
    conn: &mut PgConnection,
    view: &ObjectView,
    keys: &[String],
    at: Option<OffsetDateTime>,
) -> Result<Vec<Node>, StoreError> {
    let mut members: Vec<(Uuid, NodeKind, Option<String>, Option<String>)> = Vec::new();
    let domain: Option<String> = sqlx::query_scalar(
        "SELECT slug FROM addressing_domain
          WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
    )
    .bind(view.domain)
    .bind(at)
    .fetch_optional(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    members.push((view.domain, NodeKind::Domain, None, domain));
    if let Some(value) = &view.value {
        let host = match view.kind.as_str() {
            "prefix" => value.clone(),
            "address" if value.contains(':') => format!("{value}/128"),
            "address" => format!("{value}/32"),
            _ => return Ok(Vec::new()),
        };
        let above: Vec<(Uuid, String, String)> = sqlx::query_as(
            "SELECT id, prefix::text, nature FROM prefix
              WHERE domain_id = $1 AND prefix >>= $2::cidr AND id <> $3
                AND valid_period @> coalesce($4::timestamptz, now())
              ORDER BY masklen(prefix)",
        )
        .bind(view.domain)
        .bind(&host)
        .bind(view.id)
        .bind(at)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        for (id, prefix, nature) in above {
            let kind = if nature == "network" {
                NodeKind::Network
            } else {
                NodeKind::Container
            };
            members.push((id, kind, Some(prefix), None));
        }
        let own = if view.kind == "address" {
            NodeKind::Address
        } else {
            {
                let nature: Option<String> = sqlx::query_scalar(
                    "SELECT nature FROM prefix
                      WHERE id = $1 AND valid_period @> coalesce($2::timestamptz, now())",
                )
                .bind(view.id)
                .bind(at)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| store(&e))?;
                if nature.as_deref() == Some("network") {
                    NodeKind::Network
                } else {
                    NodeKind::Container
                }
            }
        };
        members.push((view.id, own, Some(host), None));
    } else if view.kind != "domain" {
        return Ok(Vec::new());
    }
    let mut nodes = Vec::with_capacity(members.len());
    for (id, kind, prefix, name) in members {
        nodes.push(Node {
            id,
            kind,
            prefix: prefix.and_then(|p| Prefix::parse_any(&FAMILIES, &p).ok()),
            domain: view.domain,
            sites: lineage(conn, id, "core.site", "site", at).await?,
            entities: lineage(conn, id, "core.owner", "entity", at).await?,
            name,
            properties: resolved(conn, id, keys, at).await?,
        });
    }
    Ok(nodes)
}

/// An instant of the core as PostgreSQL reads it.
fn instant(i: Instant) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(i.0) * 1_000).expect("an instant")
}

/// The suggestion of one expected set: its values, why, and the rule (the `#<rule>` of
/// the provenance, REQ-010-50).
fn suggestion(e: &crate::inference::Expected) -> Suggestion {
    use std::fmt::Write as _;
    let mut why = format!("règle {}", e.rule);
    if !e.exceptions.is_empty() {
        let keys: Vec<String> = e.exceptions.iter().map(ToString::to_string).collect();
        let _ = write!(why, ", exceptions {}", keys.join(" › "));
    }
    if !e.observations.is_empty() {
        let _ = write!(why, " : {}", e.observations.join(", "));
    }
    let detail = e.exceptions.last().unwrap_or(&e.rule).to_string();
    Suggestion {
        values: e
            .values
            .iter()
            .map(|v| RawInput::Text(v.as_str().to_owned()))
            .collect(),
        explanation: why,
        detail: Some(detail),
    }
}

/// What the inference expects, as the contract of the core reads it (REQ-016-5).
fn answer(expectation: &Expectation) -> Expected {
    match expectation {
        Expectation::None => Expected::None,
        Expectation::Single(e) => Expected::One(suggestion(e)),
        Expectation::Multiple(c) => Expected::Candidates(c.iter().map(suggestion).collect()),
    }
}

fn connection(tx: &mut dyn ModuleTx) -> Result<&mut PgConnection, StoreError> {
    tx.connection()
        .downcast_mut::<PgConnection>()
        .ok_or_else(|| StoreError("the policy needs a PostgreSQL transaction".into()))
}

/// The source `policy.inference` (REQ-010-40, 50 to 52): the rules read in the
/// transaction at the date of the evaluation, its revision the counter of the rules.
#[derive(Debug, Clone, Copy, Default)]
pub struct InferenceSource;

impl ExpectedValueSource for InferenceSource {
    fn id(&self) -> &str {
        SOURCE
    }

    /// A node that inherits a value receives no default (REQ-010-52).
    fn defaults_when_inheriting(&self) -> bool {
        false
    }

    fn revision<'a>(
        &'a self,
        _ctx: ExpectedContext,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<String, StoreError>> {
        Box::pin(async move {
            let n: i64 = sqlx::query_scalar("SELECT inference FROM policy.policy_revision")
                .fetch_one(connection(tx)?)
                .await
                .map_err(|e| store(&e))?;
            Ok(n.to_string())
        })
    }

    fn expected<'a>(
        &'a self,
        property: &'a str,
        object: &'a ObjectView,
        ctx: ExpectedContext,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Expected, StoreError>> {
        Box::pin(async move {
            Ok(self
                .expected_many(property, std::slice::from_ref(object), ctx, tx)
                .await?
                .pop()
                .unwrap_or_default())
        })
    }

    fn expected_many<'a>(
        &'a self,
        property: &'a str,
        objects: &'a [ObjectView],
        ctx: ExpectedContext,
        tx: &'a mut dyn ModuleTx,
    ) -> BoxFuture<'a, Result<Vec<Expected>, StoreError>> {
        Box::pin(async move {
            let Ok(axis) = AxisKey::parse(property) else {
                return Ok(vec![Expected::None; objects.len()]);
            };
            let at = ctx.valid_at.map(instant);
            let conn = connection(tx)?;
            let rules: Vec<InferenceRule> = rules_at(conn, at)
                .await?
                .into_iter()
                .filter(|r| r.axis == axis)
                .collect();
            if rules.is_empty() {
                return Ok(vec![Expected::None; objects.len()]);
            }
            let keys = tested(&rules);
            let mut out = Vec::with_capacity(objects.len());
            for view in objects {
                let nodes = chain(conn, view, &keys, at).await?;
                if nodes.is_empty() {
                    out.push(Expected::None);
                } else {
                    out.push(answer(&expected(&rules, &axis, &nodes)));
                }
            }
            Ok(out)
        })
    }
}

/// The plan of `policy.inference_rule.create` or `.update` (REQ-010-42 to 46, 50): key,
/// axis, parent (an exception), priority, criteria, values and, optionally, the domain it
/// cites (its scope for the rights, locked by the write).
pub(crate) fn plan_rule(
    operation: &ModuleOperation,
    object: Uuid,
    resolve: &dyn Fn(Ref) -> Option<Uuid>,
) -> Result<ModulePlan, Violation> {
    let body = &operation.body;
    let known = [
        "local",
        "id",
        "completed",
        "key",
        "axis",
        "parent",
        "priority",
        "criteria",
        "values",
        "domain",
    ];
    if let Some(field) = body.keys().find(|k| !known.contains(&k.as_str())) {
        return Err(invalid(operation, format!("unknown field {field}")));
    }
    let reference = |name: &str| -> Result<Option<Uuid>, Violation> {
        body.get(name)
            .filter(|v| !v.is_null())
            .map(|v| {
                let r: Ref = serde_json::from_value(v.clone())
                    .map_err(|_| invalid(operation, format!("{name} is a reference")))?;
                resolve(r).ok_or_else(|| invalid(operation, format!("{name}: unknown object")))
            })
            .transpose()
    };
    let mut columns = Map::new();
    if let Some(priority) = body.get("priority") {
        let priority = priority
            .as_i64()
            .and_then(|p| i32::try_from(p).ok())
            .ok_or_else(|| invalid(operation, "priority is an integer"))?;
        columns.insert("priority".into(), priority.into());
    }
    if let Some(list) = body.get("criteria") {
        criteria(list)?;
        columns.insert("criteria".into(), list.clone());
    }
    if let Some(list) = body.get("values") {
        let keys = strings(list, "values").map_err(|_| invalid(operation, "values is a list"))?;
        if keys.is_empty() {
            return Err(criterion_invalid(
                "a rule of inference expects at least one value",
            ));
        }
        columns.insert("value_keys".into(), keys.into());
    }
    match operation.name.as_str() {
        "inference_rule.create" => {
            let key = body
                .get("key")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(operation, "key is required"))?;
            Key::new(key)?;
            let axis = body
                .get("axis")
                .and_then(Value::as_str)
                .and_then(|a| AxisKey::parse(a).ok())
                .ok_or_else(|| invalid(operation, "axis is a property key"))?;
            if !columns.contains_key("value_keys") {
                return Err(invalid(operation, "values are required"));
            }
            let domain = reference("domain")?;
            let mut row = json!({
                "id": object, "key": key, "axis_key": axis.to_string(),
                "parent_id": reference("parent")?, "priority": 0, "criteria": [],
                "domain_id": domain,
            });
            if let Value::Object(row) = &mut row {
                row.extend(columns);
            }
            let mut planned = plan(INFERENCE_RULE, object, true, json!({ "create": row }));
            planned.domains = domain.into_iter().collect();
            Ok(planned)
        }
        "inference_rule.update" => {
            for fixed in ["key", "axis", "parent", "domain"] {
                if body.contains_key(fixed) {
                    return Err(invalid(operation, format!("{fixed} cannot change")));
                }
            }
            let id = reference("id")?.ok_or_else(|| invalid(operation, "id is required"))?;
            Ok(plan(
                INFERENCE_RULE,
                id,
                false,
                json!({ "update": columns }),
            ))
        }
        _ => Err(invalid(operation, "unknown operation")),
    }
}

/// The domain a stored rule cites, for its record in `module_object` (its scope).
pub(crate) async fn domain_of(
    conn: &mut PgConnection,
    rule: Uuid,
) -> Result<Option<Uuid>, StoreError> {
    sqlx::query_scalar(
        "SELECT domain_id FROM policy.policy_inference_rule WHERE id = $1
          ORDER BY upper(valid_period) DESC NULLS FIRST LIMIT 1",
    )
    .bind(rule)
    .fetch_optional(conn)
    .await
    .map(Option::flatten)
    .map_err(|e| store(&e))
}

/// The checks on the final state of the rules of inference a changeset wrote (REQ-010-40,
/// 42, 44): an axis designated for the inference, a parent on the same axis, living values
/// of the axis, criteria that read back, the depth of the exceptions.
pub(crate) async fn check(
    conn: &mut PgConnection,
    changeset: Uuid,
) -> Result<Vec<Violation>, StoreError> {
    let written: Vec<WrittenRule> = sqlx::query_as(
        "SELECT id, lower(valid_period), axis_key, parent_id, value_keys, criteria
               FROM policy.policy_inference_rule WHERE changeset_id = $1",
    )
    .bind(changeset)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| store(&e))?;
    let mut found = Vec::new();
    for (id, at, axis, parent, keys, stored) in &written {
        let inference: Option<bool> = sqlx::query_scalar(
            "SELECT inference FROM policy.policy_axis
              WHERE property_key = $1 AND valid_period @> $2",
        )
        .bind(axis)
        .bind(at)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        // `policy.environment` is the axis of the inference from the installation (I1).
        if axis != crate::module::ENVIRONMENT && inference != Some(true) {
            found.push(
                Violation::new(
                    &AXIS_INVALID,
                    format!("{axis} is not an axis designated for the inference"),
                )
                .with_subjects([*id]),
            );
        }
        if let Some(parent) = parent {
            let same: Option<String> = sqlx::query_scalar(
                "SELECT axis_key FROM policy.policy_inference_rule
                  WHERE id = $1 AND valid_period @> $2",
            )
            .bind(parent)
            .bind(at)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| store(&e))?;
            if same.as_deref() != Some(axis.as_str()) {
                found.push(
                    criterion_invalid("an exception is under a living rule of the same axis")
                        .with_subjects([*id]),
                );
            }
        }
        let living: Vec<String> = sqlx::query_scalar(
            "SELECT e.key FROM property_enum_value e
               JOIN property_declaration d ON d.id = e.property_id
              WHERE d.key = $1 AND e.key = ANY($2)
                AND e.valid_period @> $3 AND d.valid_period @> $3",
        )
        .bind(axis)
        .bind(keys)
        .bind(at)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| store(&e))?;
        for key in keys.iter().filter(|k| !living.contains(k)) {
            found.push(
                Violation::new(
                    &VALUE_UNKNOWN,
                    format!("{key} is not a living value of {axis}"),
                )
                .with_subjects([*id]),
            );
        }
        if let Err(v) = criteria(stored) {
            found.push(v.with_subjects([*id]));
        }
    }
    let mut seen = BTreeSet::new();
    for (id, at, ..) in &written {
        if !seen.insert(*at) {
            continue;
        }
        for root in rules_at(conn, Some(*at)).await? {
            if let Err(violations) = root.check() {
                found.extend(violations.into_iter().map(|v| v.with_subjects([*id])));
            }
        }
    }
    Ok(found)
}
