//! Inheritance along the entity and site trees, and obligations scoped by entity (spec
//! 003, REQ-003-11, 16; task 003-18): examples, then the materialisation checked against
//! the naive resolver on random sequences of dated moves.

use std::collections::{BTreeMap, HashMap};

use kartenn_core::inheritance::{Assigned, ParentTree, resolve};
use kartenn_core::objects::Nature;
use kartenn_core::values::{Cardinality, Scalar, Value};
use kartenn_service::authz::Unrestricted;
use kartenn_service::declarations::{core_declarations, sync_modules};
use kartenn_service::operations::{
    DeclarationInput, ObligationInput, PeriodSpec, RawInput, TypeInput,
};
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::{ChangesetRequest, ObjectKind, Operation, Outcome, Ref, submit};
use kartenn_store_pg::explain::explain;
use kartenn_store_pg::write::PgStore;
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

/// 2030-01-01T00:00Z in microseconds; the tests live on days from there.
const T0: i64 = 1_893_456_000_000_000;
const DAY: i64 = 86_400_000_000;

fn day(d: i64) -> i64 {
    T0 + d * DAY
}

fn instant(micros: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(i128::from(micros) * 1_000).unwrap()
}

struct Fixture {
    pool: PgPool,
    store: PgStore,
    author: Uuid,
    /// `entity` or `site`.
    tree: &'static str,
    kind: ObjectKind,
    property: Uuid,
}

impl Fixture {
    /// `custom.zone` (closed list `a`, `b`), heritable along `tree` and carried by its
    /// objects; declared on day 0.
    async fn new(pool: &PgPool, tree: &'static str) -> Self {
        let author = sqlx::query_scalar(
            "INSERT INTO person (id, display_name) VALUES (gen_random_uuid(), 't') RETURNING id",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        let mut f = Self {
            pool: pool.clone(),
            store: PgStore::new(pool.clone()),
            author,
            tree,
            kind: if tree == "entity" {
                ObjectKind::Entity
            } else {
                ObjectKind::Site
            },
            property: Uuid::nil(),
        };
        f.applied(
            vec![
                Operation::DeclareProperty {
                    local: Some(1),
                    declaration: declaration(tree, &[tree], Vec::new()),
                },
                Operation::AddEnumValue {
                    local: None,
                    property: Ref::Local(1),
                    key: "a".into(),
                    labels: std::collections::BTreeMap::new(),
                },
                Operation::AddEnumValue {
                    local: None,
                    property: Ref::Local(1),
                    key: "b".into(),
                    labels: std::collections::BTreeMap::new(),
                },
            ],
            0,
            None,
        )
        .await;
        f.property =
            sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'custom.zone'")
                .fetch_one(pool)
                .await
                .unwrap();
        f
    }

    async fn submit(&self, operations: Vec<Operation>, start: i64, end: Option<i64>) -> Outcome {
        let request = ChangesetRequest {
            author: self.author,
            motive: "test".into(),
            period: Some(PeriodSpec {
                start: day(start),
                end: end.map(day),
            }),
            operations,
            simulate: false,
            idempotency_key: None,
            approval_self: true,
        };
        submit(&self.store, &Unrestricted, &request, MAX_OPERATIONS)
            .await
            .unwrap()
    }

    async fn applied(&self, operations: Vec<Operation>, start: i64, end: Option<i64>) {
        let outcome = self.submit(operations, start, end).await;
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }

    fn create(&self, slug: &str, parent: Option<Uuid>) -> Operation {
        let parent = parent.map(Ref::Id);
        if self.kind == ObjectKind::Entity {
            Operation::CreateEntity {
                local: None,
                slug: slug.into(),
                parent,
            }
        } else {
            Operation::CreateSite {
                local: None,
                slug: slug.into(),
                parent,
            }
        }
    }

    fn mv(&self, id: Uuid, parent: Option<Uuid>) -> Operation {
        let (id, parent) = (Ref::Id(id), parent.map(Ref::Id));
        if self.kind == ObjectKind::Entity {
            Operation::MoveEntity {
                id,
                parent,
                expected: None,
            }
        } else {
            Operation::MoveSite {
                id,
                parent,
                expected: None,
            }
        }
    }

    fn assign(&self, object: Uuid, value: &str) -> Operation {
        Operation::AssignProperty {
            object: Ref::Id(object),
            kind: self.kind,
            property: Ref::Id(self.property),
            values: vec![RawInput::Text(value.into())],
        }
    }

    async fn id(&self, slug: &str) -> Uuid {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {} WHERE slug = $1 LIMIT 1",
            self.tree
        )))
        .bind(slug)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    /// Stored origin of `object` at day `d`.
    async fn origin(&self, object: Uuid, d: i64) -> Option<Uuid> {
        sqlx::query_scalar(
            "SELECT origin_object_id FROM property_origin
             WHERE object_id = $1 AND property_id = $2 AND valid_period @> $3",
        )
        .bind(object)
        .bind(self.property)
        .bind(instant(day(d)))
        .fetch_optional(&self.pool)
        .await
        .unwrap()
    }

    /// Every object of the tree, whatever its period, by slug (so that a seed replays).
    async fn objects(&self) -> Vec<Uuid> {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT id FROM {} GROUP BY id ORDER BY min(slug)",
            self.tree
        )))
        .fetch_all(&self.pool)
        .await
        .unwrap()
    }

    /// The oracle at instant `t`: origin of every object of the tree.
    async fn oracle(&self, t: OffsetDateTime) -> BTreeMap<Uuid, Uuid> {
        let nodes: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT id, parent_id FROM {} WHERE valid_period @> $1",
            self.tree
        )))
        .bind(t)
        .fetch_all(&self.pool)
        .await
        .unwrap();
        let assigned: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT object_id, kind FROM property_assignment
             WHERE property_id = $1 AND valid_period @> $2",
        )
        .bind(self.property)
        .bind(t)
        .fetch_all(&self.pool)
        .await
        .unwrap();
        let tree = ParentTree::new(nodes.iter().filter_map(|(id, p)| Some((*id, (*p)?))));
        let assignments: HashMap<Uuid, Assigned> = assigned
            .into_iter()
            .map(|(id, kind)| {
                let a = if kind == "cleared" {
                    Assigned::Cleared
                } else {
                    Assigned::Value(Value::One(Scalar::Int(0)))
                };
                (id, a)
            })
            .collect();
        nodes
            .iter()
            .filter_map(|(o, _)| Some((*o, resolve(&tree, &assignments, *o, true).origin?)))
            .collect()
    }

    /// The materialisation at instant `t`.
    async fn stored(&self, t: OffsetDateTime) -> BTreeMap<Uuid, Uuid> {
        let rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT object_id, origin_object_id FROM property_origin
             WHERE property_id = $1 AND valid_period @> $2",
        )
        .bind(self.property)
        .bind(t)
        .fetch_all(&self.pool)
        .await
        .unwrap();
        rows.into_iter().collect()
    }
}

/// `custom.zone`, a closed list heritable along `tree`, carried by `carriers`.
fn declaration(
    tree: &str,
    carriers: &[&str],
    obligations: Vec<ObligationInput>,
) -> DeclarationInput {
    DeclarationInput {
        key: "custom.zone".into(),
        value_type: TypeInput::ENUM,
        cardinality: Cardinality::Single,
        carriers: carriers.iter().map(|c| (*c).to_owned()).collect(),
        inherit: Some(tree.into()),
        module: None,
        obligations,
        expected: None,
        labels: None,
    }
}

#[kartenn_testing::pg_test]
async fn entity_values_follow_the_parents_over_time(pool: PgPool) {
    follow_the_parents(&pool, "entity").await;
}

#[kartenn_testing::pg_test]
async fn site_values_follow_the_parents_over_time(pool: PgPool) {
    follow_the_parents(&pool, "site").await;
}

async fn follow_the_parents(pool: &PgPool, tree: &'static str) {
    let f = Fixture::new(pool, tree).await;
    let p = |s: &str| format!("{tree}-{s}");
    f.applied(vec![f.create(&p("group"), None)], 0, None).await;
    let group = f.id(&p("group")).await;
    f.applied(vec![f.create(&p("sub"), Some(group))], 0, None)
        .await;
    let sub = f.id(&p("sub")).await;
    f.applied(vec![f.create(&p("team"), Some(sub))], 0, None)
        .await;
    let team = f.id(&p("team")).await;
    f.applied(vec![f.create(&p("other"), None)], 0, None).await;
    let other = f.id(&p("other")).await;

    // REQ-003-11: assigned on the root, inherited two levels down.
    f.applied(vec![f.assign(group, "a")], 1, None).await;
    assert_eq!(f.origin(team, 0).await, None);
    assert_eq!(f.origin(team, 1).await, Some(group));
    assert_eq!(f.origin(sub, 1).await, Some(group));
    // The sub-entity moves under another root on day 3: its subtree follows.
    f.applied(vec![f.assign(other, "b")], 2, None).await;
    f.applied(vec![f.mv(sub, Some(other))], 3, None).await;
    assert_eq!(f.origin(team, 2).await, Some(group), "the past is kept");
    assert_eq!(f.origin(team, 3).await, Some(other));
    // A move over a closed period only: back to the first root afterwards.
    f.applied(vec![f.mv(team, None)], 5, Some(6)).await;
    assert_eq!(f.origin(team, 5).await, None);
    assert_eq!(f.origin(team, 6).await, Some(other));
    // REQ-003-14: the explained chain walks the parents of that instant.
    let explained = explain(pool, team, f.property, instant(day(4)), None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(explained.origin, Some(other));
    assert_eq!(explained.chain, vec![team, sub, other]);
    assert_eq!(explained.value.as_deref(), Some("\"b\""));
}

#[kartenn_testing::pg_test]
async fn heritable_carriers_belong_to_the_tree(pool: PgPool) {
    let f = Fixture::new(&pool, "entity").await;
    for (tree, carriers) in [
        ("entity", ["prefix"].as_slice()),
        ("site", ["entity"].as_slice()),
        ("addressing", ["entity", "prefix"].as_slice()),
    ] {
        let mut d = declaration(tree, carriers, Vec::new());
        d.key = "custom.other".into();
        match f
            .submit(
                vec![Operation::DeclareProperty {
                    local: None,
                    declaration: d,
                }],
                0,
                None,
            )
            .await
        {
            Outcome::Refused { violations, .. } => {
                assert_eq!(violations[0].rule.id, "prop.declaration.invalid", "{tree}");
            }
            other => panic!("{tree}: {other:?}"),
        }
    }
}

/// A small deterministic generator (xorshift64*), to replay a failing seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[usize::try_from(self.below(items.len() as u64)).unwrap()]
    }
}

/// One random operation on the tree, with a random period; objects are created from day
/// 0 on, so that most moves find their object and parent.
async fn random_step(
    f: &Fixture,
    rng: &mut Rng,
    n: &mut u32,
) -> (Vec<Operation>, i64, Option<i64>) {
    let start = i64::try_from(rng.below(10)).unwrap();
    let end = (rng.below(3) == 0).then(|| start + 1 + i64::try_from(rng.below(4)).unwrap());
    let pool = f.objects().await;
    // A parent other than `id`, or none.
    let parent = |rng: &mut Rng, id: Option<Uuid>| {
        let others: Vec<Uuid> = pool.iter().copied().filter(|o| Some(*o) != id).collect();
        (rng.below(4) != 0 && !others.is_empty()).then(|| *rng.pick(&others))
    };
    let op = match rng.below(12) {
        _ if pool.len() < 3 => {
            *n += 1;
            return (vec![f.create(&format!("n{n}"), None)], 0, None);
        }
        0 | 1 => {
            *n += 1;
            let parent = parent(rng, None);
            return (vec![f.create(&format!("n{n}"), parent)], 0, None);
        }
        2..=5 => {
            let id = *rng.pick(&pool);
            let parent = parent(rng, Some(id));
            f.mv(id, parent)
        }
        6 | 7 => {
            let id = *rng.pick(&pool);
            f.assign(id, rng.pick(&["a", "b"]))
        }
        8 => Operation::ClearProperty {
            object: Ref::Id(*rng.pick(&pool)),
            kind: f.kind,
            property: Ref::Id(f.property),
        },
        9 | 10 => Operation::UnassignProperty {
            object: Ref::Id(*rng.pick(&pool)),
            kind: f.kind,
            property: Ref::Id(f.property),
        },
        _ => Operation::Close {
            kind: f.kind,
            id: Ref::Id(*rng.pick(&pool)),
            expected: None,
            cascade: false,
        },
    };
    (vec![op], start, end)
}

#[kartenn_testing::pg_test]
async fn entity_origins_match_the_oracle(pool: PgPool) {
    matches_the_oracle(&pool, "entity").await;
}

#[kartenn_testing::pg_test]
async fn site_origins_match_the_oracle(pool: PgPool) {
    matches_the_oracle(&pool, "site").await;
}

async fn matches_the_oracle(pool: &PgPool, tree: &'static str) {
    let f = Fixture::new(pool, tree).await;
    let mut n = 0;
    for seed in 1..=4_u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let mut moves = 0;
        for step in 0..40 {
            let (operations, start, end) = random_step(&f, &mut rng, &mut n).await;
            let outcome = f.submit(operations.clone(), start, end).await;
            if matches!(outcome, Outcome::Applied { .. })
                && matches!(
                    operations[0],
                    Operation::MoveEntity { .. } | Operation::MoveSite { .. }
                )
            {
                moves += 1;
            }
            // Every half day of the horizon, boundaries included.
            for half in 0..30 {
                let t = instant(day(0) + i64::from(half) * DAY / 2);
                assert_eq!(
                    f.stored(t).await,
                    f.oracle(t).await,
                    "{tree}, seed {seed}, step {step} ({operations:?} from day {start} to {end:?}), at {t}"
                );
            }
        }
        assert!(
            moves > 3,
            "{tree}, seed {seed}: too few moves applied ({moves})"
        );
    }
    // Segments are maximal: no two touching rows with the same origin.
    let touching: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM property_origin a JOIN property_origin b
           ON a.object_id = b.object_id AND a.property_id = b.property_id
          AND upper(a.valid_period) = lower(b.valid_period)
          AND a.origin_object_id = b.origin_object_id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(touching, 0);
}

/// Refused: the rules and the objects of the first violation.
fn refusal(outcome: Outcome) -> (Vec<&'static str>, Vec<Uuid>) {
    match outcome {
        Outcome::Refused { violations, .. } => (
            violations.iter().map(|v| v.rule.id).collect(),
            violations[0].subjects.clone(),
        ),
        other => panic!("expected refused, got {other:?}"),
    }
}

#[kartenn_testing::pg_test]
async fn an_entity_obligation_follows_owners_and_moves(pool: PgPool) {
    let f = Fixture::new(&pool, "entity").await;
    for outcome in sync_modules(
        &f.store,
        &Unrestricted,
        &[core_declarations()],
        MAX_OPERATIONS,
    )
    .await
    .unwrap()
    {
        assert!(matches!(outcome, Outcome::Applied { .. }), "{outcome:?}");
    }
    let owner: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'core.owner'")
            .fetch_one(&pool)
            .await
            .unwrap();
    f.applied(vec![f.create("group", None)], 0, None).await;
    let group = f.id("group").await;
    f.applied(vec![f.create("branch", Some(group))], 0, None)
        .await;
    let branch = f.id("branch").await;
    f.applied(vec![f.create("outsider", None)], 0, None).await;
    let outsider = f.id("outsider").await;
    // Two domains: one owned by the branch, one by the outsider, each with a network.
    let domain = |local: u32, slug: &str, by: Uuid| {
        vec![
            Operation::CreateDomain {
                local: Some(local),
                slug: slug.into(),
            },
            Operation::AssignProperty {
                object: Ref::Local(local),
                kind: ObjectKind::Domain,
                property: Ref::Id(owner),
                values: vec![RawInput::Text(by.to_string())],
            },
            Operation::CreatePrefix {
                local: None,
                domain: Ref::Local(local),
                prefix: "10.1.2.0/24".into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
        ]
    };
    f.applied(domain(1, "lab", branch), 0, None).await;
    f.applied(domain(2, "edge", outsider), 0, None).await;
    let lab_network: Uuid = sqlx::query_scalar(
        "SELECT p.id FROM prefix p JOIN addressing_domain d ON d.id = p.domain_id WHERE d.slug = 'lab'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let edge_network: Uuid = sqlx::query_scalar(
        "SELECT p.id FROM prefix p JOIN addressing_domain d ON d.id = p.domain_id WHERE d.slug = 'edge'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // REQ-003-16: `custom.tier` mandatory for the objects owned within the group, and for
    // the entities of the group.
    let tier = |obligations| DeclarationInput {
        key: "custom.tier".into(),
        value_type: TypeInput::Int { min: 0, max: 9 },
        cardinality: Cardinality::Single,
        carriers: ["prefix".to_owned(), "entity".to_owned()].into(),
        inherit: None,
        module: None,
        obligations,
        expected: None,
        labels: None,
    };
    let scoped = vec![ObligationInput::Entity { id: group }];
    let (rules, objects) = refusal(
        f.submit(
            vec![Operation::DeclareProperty {
                local: None,
                declaration: tier(scoped.clone()),
            }],
            1,
            None,
        )
        .await,
    );
    assert_eq!(rules, vec!["prop.assignment.required"]);
    let mut expected = vec![group, branch, lab_network];
    expected.sort();
    assert_eq!(
        objects, expected,
        "the edge network and the outsider are out of scope"
    );

    // Declared without obligation, valued, then made mandatory.
    f.applied(
        vec![Operation::DeclareProperty {
            local: Some(1),
            declaration: tier(Vec::new()),
        }],
        1,
        None,
    )
    .await;
    let tier_id: Uuid =
        sqlx::query_scalar("SELECT id FROM property_declaration WHERE key = 'custom.tier'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let set = |object: Uuid, kind: ObjectKind| Operation::AssignProperty {
        object: Ref::Id(object),
        kind,
        property: Ref::Id(tier_id),
        values: vec![RawInput::Int(1)],
    };
    f.applied(
        vec![
            set(group, ObjectKind::Entity),
            set(branch, ObjectKind::Entity),
            set(lab_network, ObjectKind::Prefix),
            Operation::UpdateProperty {
                id: Ref::Id(tier_id),
                declaration: tier(scoped),
            },
        ],
        1,
        None,
    )
    .await;

    // A network created under the branch's domain needs its tier.
    let lab: Uuid = sqlx::query_scalar("SELECT id FROM addressing_domain WHERE slug = 'lab'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let network = |domain: Uuid, value: &str| Operation::CreatePrefix {
        local: Some(9),
        domain: Ref::Id(domain),
        prefix: value.into(),
        nature: Nature::Network,
        point_to_point: false,
        anycast: false,
    };
    let (rules, _) = refusal(f.submit(vec![network(lab, "10.1.3.0/24")], 2, None).await);
    assert_eq!(rules, vec!["prop.assignment.required"]);
    // The outsider moving under the group brings its unvalued network and itself in scope.
    let (rules, objects) = refusal(f.submit(vec![f.mv(outsider, Some(group))], 2, None).await);
    assert_eq!(rules, vec!["prop.assignment.required"]);
    let mut expected = vec![outsider, edge_network];
    expected.sort();
    assert_eq!(objects, expected);
    // Valued, it may move.
    f.applied(
        vec![
            set(outsider, ObjectKind::Entity),
            set(edge_network, ObjectKind::Prefix),
            f.mv(outsider, Some(group)),
        ],
        2,
        None,
    )
    .await;
    // Out of scope again from day 3: a network without tier is accepted there.
    f.applied(vec![f.mv(branch, None)], 3, None).await;
    f.applied(vec![network(lab, "10.1.3.0/24")], 3, None).await;
}
