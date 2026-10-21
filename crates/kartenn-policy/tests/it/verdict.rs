//! The query of compliance (spec 010, REQ-010-13, 14, 15, 25, 31, 38; task 010-13).

use std::collections::BTreeSet;

use kartenn_core::objects::{AddressUsage, Nature};
use kartenn_policy::cache::SnapshotCache;
use kartenn_policy::evaluate::{Decisive, Outcome, Verdict};
use kartenn_policy::model::{AxisKey, AxisValue, Key, Transport};
use kartenn_policy::verdict::{
    Access, Answer, Empty, EndpointKind, EndpointRef, FLOW_REQUEST, Object, Query, QueryError,
    Source, VERDICT_READ, query,
};
use kartenn_service::authz::{Authorizer, Grants, Subject};
use kartenn_service::operations::{ModuleKind, RawInput};
use kartenn_service::{ObjectKind, Operation, Ref};
use serde_json::{Value, json};
use sqlx::PgPool;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;

use crate::support::env::{Env, op};

fn machines(name: &str, fields: Value) -> Operation {
    let mut object = fields;
    object["op"] = format!("machines.{name}").into();
    serde_json::from_value(object).unwrap()
}

/// The environments, the flow types `admin` and `application`, the services `ssh`,
/// `https` and `https-admin`, and the rule « prod to dev for admin and application ».
async fn policy(env: &Env) {
    env.applied(vec![
        op("environment.create", json!({ "key": "prod" })),
        op("environment.create", json!({ "key": "dev" })),
        op(
            "flow_type.create",
            json!({ "local": 1, "key": "admin", "label": "Administration" }),
        ),
        op(
            "flow_type.create",
            json!({ "local": 2, "key": "application", "label": "Application" }),
        ),
        op(
            "service.create",
            json!({ "key": "ssh", "label": "SSH", "flow_type": { "local": 1 },
                    "entries": [{ "transport": "tcp", "ports": 22 }] }),
        ),
        op(
            "service.create",
            json!({ "key": "https", "label": "HTTPS", "flow_type": { "local": 2 },
                    "entries": [{ "transport": "tcp", "ports": 443 }] }),
        ),
        op(
            "service.create",
            json!({ "key": "https-admin", "label": "HTTPS d'administration",
                    "flow_type": { "local": 1 },
                    "entries": [{ "transport": "tcp", "ports": 443 }] }),
        ),
        op(
            "rule.create",
            json!({ "key": "prod-dev", "label": "Prod vers dev", "effect": "allow",
                    "flow_types": [{ "local": 1 }, { "local": 2 }],
                    "source": { "policy.environment": ["prod"] },
                    "destination": { "policy.environment": ["dev"] } }),
        ),
    ])
    .await;
}

impl Env {
    /// The network `prefix` of the domain, classified `environment`.
    async fn network(&self, prefix: &str, environment: &str) -> Uuid {
        let property = self.property("policy.environment").await;
        self.applied(vec![
            Operation::CreatePrefix {
                local: Some(1),
                domain: Ref::Id(self.domain),
                prefix: prefix.into(),
                nature: Nature::Network,
                point_to_point: false,
                anycast: false,
            },
            Operation::AssignProperty {
                object: Ref::Local(1),
                kind: ObjectKind::Prefix,
                property: Ref::Id(property),
                values: vec![RawInput::Text(environment.into())],
            },
        ])
        .await;
        sqlx::query_scalar("SELECT id FROM prefix WHERE prefix = $1::cidr")
            .bind(prefix)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn address(&self, value: &str) -> Uuid {
        self.address_of(value, AddressUsage::Host).await
    }

    /// The address `value`, of `usage` (a `vip` may be held by several machines).
    async fn address_of(&self, value: &str, usage: AddressUsage) -> Uuid {
        self.applied(vec![Operation::CreateAddress {
            local: None,
            domain: Ref::Id(self.domain),
            address: value.into(),
            usage,
        }])
        .await;
        sqlx::query_scalar("SELECT id FROM address WHERE address = $1::inet")
            .bind(value)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    /// The machine `name`, one interface holding `addresses`.
    async fn machine(&self, name: &str, addresses: &[Uuid]) -> Uuid {
        let acme = self.id("entity", "acme").await;
        let mut operations = vec![
            machines(
                "create_machine",
                json!({ "local": 1, "name": name, "kind": "physical", "owner": { "id": acme } }),
            ),
            machines(
                "create_interface",
                json!({ "local": 2, "machine": { "local": 1 }, "name": "eth0" }),
            ),
        ];
        for address in addresses {
            operations.push(machines(
                "attach_address",
                json!({ "interface": { "local": 2 }, "address": { "id": address } }),
            ));
        }
        self.applied(operations).await;
        sqlx::query_scalar(
            "SELECT id FROM machines.machine WHERE name = $1 AND valid_period @> now()",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn at() -> OffsetDateTime {
    OffsetDateTime::now_utc() + std::time::Duration::from_secs(60)
}

fn ask(source: EndpointRef, destination: EndpointRef, object: Object) -> Query {
    Query {
        source,
        destination,
        object,
        dossier: None,
        entity: None,
        valid_at: at(),
        as_of: None,
    }
}

fn of(kind: EndpointKind, id: Uuid) -> EndpointRef {
    EndpointRef { kind, id }
}

fn service(key: &str) -> Object {
    Object::Service(Key::new(key).unwrap())
}

async fn answer<A: Authorizer>(
    env: &Env,
    authorizer: &A,
    subject: Option<&Subject>,
    q: &Query,
) -> Result<Answer, QueryError> {
    let cache = SnapshotCache::new();
    let mut conn = env.pool.acquire().await.unwrap();
    query(&mut conn, &env.store, authorizer, &cache, subject, q).await
}

async fn system(env: &Env, q: &Query) -> Answer {
    answer(env, &Grants::new(), None, q).await.unwrap()
}

/// REQ-010-13: machine A (2 addresses) to machine B (3 addresses), `ssh`: 6 verdicts; a
/// machine without address: an empty answer, `no_address`.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_13(pool: PgPool) {
    let env = Env::new(&pool).await;
    policy(&env).await;
    env.network("10.1.0.0/24", "prod").await;
    env.network("10.2.0.0/24", "dev").await;
    let mut a_addresses = Vec::new();
    for value in ["10.1.0.1", "10.1.0.2"] {
        a_addresses.push(env.address(value).await);
    }
    let mut b_addresses = Vec::new();
    for value in ["10.2.0.1", "10.2.0.2", "10.2.0.3"] {
        b_addresses.push(env.address(value).await);
    }
    let a = env.machine("a", &a_addresses).await;
    let b = env.machine("b", &b_addresses).await;
    let found = system(
        &env,
        &ask(
            of(EndpointKind::Machine, a),
            of(EndpointKind::Machine, b),
            service("ssh"),
        ),
    )
    .await;
    assert_eq!(found.evaluations.len(), 6);
    assert!(
        found
            .evaluations
            .iter()
            .all(|e| e.evaluation.verdict == Verdict::Allowed)
    );
    let synthesis = found.synthesis.unwrap();
    assert_eq!((synthesis.allowed, synthesis.denied), (6, 0));
    assert_eq!(found.empty, None);

    let empty = env.machine("bare", &[]).await;
    let found = system(
        &env,
        &ask(
            of(EndpointKind::Machine, empty),
            of(EndpointKind::Machine, b),
            service("ssh"),
        ),
    )
    .await;
    assert!(found.evaluations.is_empty());
    assert_eq!(found.empty, Some(Empty::NoAddress));
    assert_eq!(found.synthesis, None);
}

/// REQ-010-14: `tcp/443` stands for `https` and `https-admin`, two verdicts; `tcp/9999`
/// no service covers: denied `unknown_service`, to be reviewed. REQ-010-15: the synthesis.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_14(pool: PgPool) {
    let env = Env::new(&pool).await;
    policy(&env).await;
    env.network("10.1.0.0/24", "prod").await;
    env.network("10.2.0.0/24", "dev").await;
    let a = env.address("10.1.0.1").await;
    let b = env.address("10.2.0.1").await;
    let ends = (of(EndpointKind::Address, a), of(EndpointKind::Address, b));
    let port = system(
        &env,
        &ask(ends.0, ends.1, Object::Port(Transport::Tcp, 443)),
    )
    .await;
    let services: Vec<String> = port
        .evaluations
        .iter()
        .map(|e| e.evaluation.service.as_ref().unwrap().to_string())
        .collect();
    assert_eq!(services, ["https", "https-admin"]);

    let unknown = system(
        &env,
        &ask(ends.0, ends.1, Object::Port(Transport::Tcp, 9999)),
    )
    .await;
    assert_eq!(unknown.evaluations.len(), 1);
    let evaluation = &unknown.evaluations[0].evaluation;
    assert_eq!(evaluation.verdict, Verdict::Denied);
    assert_eq!(evaluation.decisive, Decisive::UnknownService);
    assert_eq!(evaluation.outcome, Outcome::Review);
    assert_eq!(unknown.synthesis.unwrap().outcome, Outcome::Review);

    // Reversed, the rule does not apply: denied, not compliant.
    let reversed = system(&env, &ask(ends.1, ends.0, service("ssh"))).await;
    assert_eq!(reversed.evaluations[0].evaluation.verdict, Verdict::Denied);
    assert_eq!(reversed.synthesis.unwrap().outcome, Outcome::Noncompliant);
}

/// REQ-010-25: a service account with `policy.flow.request` on the entity, without reading
/// the tree, gets the verdict and the values of environment, not the container of origin;
/// `policy.verdict.read` and the reading of both endpoints give the whole explanation; any
/// other caller, or an endpoint it may not read, is answered as absent (404).
#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_25(pool: PgPool) {
    let env = Env::new(&pool).await;
    policy(&env).await;
    let network = env.network("10.1.0.0/24", "prod").await;
    env.network("10.2.0.0/24", "dev").await;
    let a = env.address("10.1.0.1").await;
    let b = env.address("10.2.0.1").await;
    let acme = env.id("entity", "acme").await;
    let mut q = ask(
        of(EndpointKind::Address, a),
        of(EndpointKind::Address, b),
        service("ssh"),
    );
    q.entity = Some(acme);
    let environment = AxisKey::parse("policy.environment").unwrap();

    let requester = Subject::from(Uuid::now_v7());
    let grants = Grants::new().on(requester.id, FLOW_REQUEST, [acme]);
    let reduced = answer(&env, &grants, Some(&requester), &q).await.unwrap();
    assert_eq!(reduced.access, Access::Reduced);
    assert_eq!(reduced.evaluations[0].evaluation.verdict, Verdict::Allowed);
    let point = &reduced.sources[0];
    assert_eq!(point.id, None, "no object of the referential");
    let axis = point.axes.iter().find(|r| r.axis == environment).unwrap();
    assert_eq!(axis.values, BTreeSet::from([AxisValue::new("prod")]));
    assert_eq!(
        axis.source,
        Source::Tree {
            origin: None,
            chain: Vec::new()
        },
        "not the container of origin"
    );

    let reader = Subject::from(Uuid::now_v7());
    let grants = Grants::new()
        .everywhere(reader.id, VERDICT_READ)
        .everywhere(reader.id, "core.address.read");
    let full = answer(&env, &grants, Some(&reader), &q).await.unwrap();
    assert_eq!(full.access, Access::Full);
    assert_eq!(full.sources[0].id, Some(a));
    let axis = full.sources[0]
        .axes
        .iter()
        .find(|r| r.axis == environment)
        .unwrap();
    assert!(
        matches!(&axis.source, Source::Tree { origin: Some(origin), .. } if *origin == network),
        "{:?}",
        axis.source
    );

    // The verdict alone, without reading the destination: as absent.
    let blind = Subject::from(Uuid::now_v7());
    let grants =
        Grants::new()
            .everywhere(blind.id, VERDICT_READ)
            .on(blind.id, "core.address.read", [a]);
    assert!(matches!(
        answer(&env, &grants, Some(&blind), &q).await,
        Err(QueryError::NotFound)
    ));
    // Nothing at all: as absent.
    let stranger = Subject::from(Uuid::now_v7());
    assert!(matches!(
        answer(&env, &Grants::new(), Some(&stranger), &q).await,
        Err(QueryError::NotFound)
    ));
    // An endpoint that does not exist, or not of the kind named: as absent.
    let mut wrong = q.clone();
    wrong.destination = of(EndpointKind::Prefix, b);
    assert!(matches!(
        system_err(&env, &wrong).await,
        QueryError::NotFound
    ));
    wrong.destination = of(EndpointKind::Address, Uuid::now_v7());
    assert!(matches!(
        system_err(&env, &wrong).await,
        QueryError::NotFound
    ));
}

async fn system_err(env: &Env, q: &Query) -> QueryError {
    answer(env, &Grants::new(), None, q).await.unwrap_err()
}

/// REQ-010-31: the network `{interne}` on `policy.sensitivity`, the machine `db-1`
/// `{confidentiel}` attached to `10.1.0.5`: the address resolves `{confidentiel}` (source:
/// machine); `10.1.0.6` without machine resolves `{interne}` (source: tree); a VIP
/// attached to two machines `{interne}` and `{confidentiel}` resolves the union.
#[kartenn_testing::pg_test]
async fn acceptance_of_req_010_31(pool: PgPool) {
    let env = Env::new(&pool).await;
    policy(&env).await;
    env.applied(vec![op("examples.install", json!({}))]).await;
    let network = env.network("10.1.0.0/24", "prod").await;
    let sensitivity_property = env.property("policy.sensitivity").await;
    let sensitive = |object: Uuid, kind: ObjectKind, value: &str| Operation::AssignProperty {
        object: Ref::Id(object),
        kind,
        property: Ref::Id(sensitivity_property),
        values: vec![RawInput::Text(value.into())],
    };
    env.applied(vec![sensitive(network, ObjectKind::Prefix, "interne")])
        .await;
    let held = env.address("10.1.0.5").await;
    let free = env.address("10.1.0.6").await;
    let vip = env.address_of("10.1.0.7", AddressUsage::Vip).await;
    let machine = ObjectKind::Module(ModuleKind::parse("machines.machine").unwrap());
    let db1 = env.machine("db-1", &[held, vip]).await;
    let web = env.machine("web-1", &[vip]).await;
    env.applied(vec![
        sensitive(db1, machine, "confidentiel"),
        sensitive(web, machine, "interne"),
    ])
    .await;
    let sensitivity = AxisKey::parse("policy.sensitivity").unwrap();
    let of_address = async |address: Uuid| {
        let found = system(
            &env,
            &ask(
                of(EndpointKind::Address, address),
                of(EndpointKind::Address, address),
                service("ssh"),
            ),
        )
        .await;
        found.sources[0]
            .axes
            .iter()
            .find(|r| r.axis == sensitivity)
            .unwrap()
            .clone()
    };
    let resolution = of_address(held).await;
    assert_eq!(
        resolution.values,
        BTreeSet::from([AxisValue::new("confidentiel")])
    );
    assert_eq!(
        resolution.source,
        Source::Machine {
            machines: vec![db1]
        }
    );
    let resolution = of_address(free).await;
    assert_eq!(
        resolution.values,
        BTreeSet::from([AxisValue::new("interne")])
    );
    assert!(matches!(resolution.source, Source::Tree { origin: Some(o), .. } if o == network));
    let resolution = of_address(vip).await;
    assert_eq!(
        resolution.values,
        BTreeSet::from([AxisValue::new("confidentiel"), AxisValue::new("interne")])
    );
    let Source::Machine { mut machines } = resolution.source else {
        panic!("from the machines");
    };
    machines.sort();
    let mut expected = vec![db1, web];
    expected.sort();
    assert_eq!(machines, expected);
}

/// The bounds: an endpoint of more than 64 addresses is refused (`policy.query.too_large`).
#[kartenn_testing::pg_test]
async fn a_machine_of_too_many_addresses_is_refused(pool: PgPool) {
    let env = Env::new(&pool).await;
    policy(&env).await;
    env.network("10.1.0.0/24", "prod").await;
    let mut addresses = Vec::new();
    for i in 1..=65 {
        addresses.push(env.address(&format!("10.1.0.{i}")).await);
    }
    let big = env.machine("big", &addresses).await;
    let q = ask(
        of(EndpointKind::Machine, big),
        of(EndpointKind::Address, addresses[0]),
        service("ssh"),
    );
    assert!(matches!(
        system_err(&env, &q).await,
        QueryError::TooLarge(_)
    ));
}
