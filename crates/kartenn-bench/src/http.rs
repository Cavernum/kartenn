//! Budgets of the core through HTTP (spec 006, REQ-006-39; task 006-25): the dataset and
//! the scenarios of `bench-core`, with the person of `bench-rbac` (50 groups, 1 000
//! bindings), played through the whole router served on a local port by
//! `kartenn_server::http::serve`, in HTTP/1.1 behind a trusted proxy that terminated TLS.
//! Half of the 200 users prove themselves by an access token (`Bearer`, ES256), the
//! other half by a session cookie; every response is compressed.
//!
//! Each budget (INV-30 to 34) is measured on the same draws through the service, with
//! the access control of the same person, and through HTTP, alternately: each p95
//! through HTTP must hold its budget, the overhead is reported (target < 10 ms at the
//! p95). Then a load (INV-36): the 200 users read with a think time, 55 of them write
//! once a second as well; at least 50 writes per second must be applied, without a 5xx
//! but the shedding.

use std::fmt::Write as _;
use std::future::Future;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow, bail};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::client::conn::http1;
use hyper::http::{Request, StatusCode, header};
use hyper_util::rt::TokioIo;
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use kartenn_auth::http::HttpClient;
use kartenn_auth::issuer::IssuerConfig;
use kartenn_auth::keys::KeyRing;
use kartenn_auth::oidc::{LoggedIn, Oidc, Session};
use kartenn_auth::secrets::FailureLimiter;
use kartenn_auth::tokens::{KeySource, TokenError, TokenVerifier};
use kartenn_core::objects::AddressUsage;
use kartenn_rest::authn::Authn;
use kartenn_rest::health::Readiness;
use kartenn_rest::quota::{Quotas, Rates};
use kartenn_rest::transport::Settings;
use kartenn_service::aggregate::FailureAggregator;
use kartenn_service::audit::{Instance, RequestContext};
use kartenn_service::authn::{Identities, StateCache};
use kartenn_service::authz::{Authorizer, Subject};
use kartenn_service::expected::Sources;
use kartenn_service::pipeline::MAX_OPERATIONS;
use kartenn_service::reads::ReadAudit;
use kartenn_service::{ChangesetRequest, Operation, Outcome, Ref, submit_in};
use kartenn_store_pg::audit_journal::PgAuditJournal;
use kartenn_store_pg::filter::{Condition, filter_in};
use kartenn_store_pg::rbac::PgAuthorizer;
use kartenn_store_pg::read::object;
use kartenn_store_pg::tree::{children_as_of, next_free_address, occupancy_as_of};
use kartenn_store_pg::write::PgStore;
use p256::pkcs8::EncodePrivateKey;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::json;
use sqlx::PgPool;
use sqlx::postgres::PgConnectOptions;
use sqlx::types::Uuid;
use sqlx::types::time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use crate::core_budgets::{Prepared, prepare};
use crate::measures::{Bench, Measure, Selection};
use crate::props::{p95, progress};
use crate::rbac::{GROUPS, ISSUER, bind};
use crate::{Dataset, Nature};

const READ: &str = "http.read-object";
const READ_PAST: &str = "http.read-past";
const FILTERED_PAGE: &str = "http.filtered-page";
const SUBTREE: &str = "http.subtree";
const UNITARY: &str = "http.unitary-changeset";
const LABELS: &str = "http.labels";
const LOAD_TEST: &str = "http.load";

/// The measures of `bench-http` (ADR 0041). The budget of the read in the past is twice
/// the current read through HTTP.
pub(crate) static BENCH: Bench = Bench {
    name: "http",
    measures: &[
        Measure::new(READ),
        Measure::new(READ_PAST).after(&[READ]),
        Measure::new(FILTERED_PAGE),
        Measure::new(SUBTREE),
        Measure::new(UNITARY),
        Measure::new(LABELS),
        Measure::new(LOAD_TEST),
    ],
};

const USERS: usize = 200;
/// Users who also write once a second during the load: 55 offered, 50 at least applied.
const WRITERS: usize = 55;
const LOAD: Duration = Duration::from_secs(30);
/// Longest write transaction, as the default of the server (`[http]`).
const WRITE_BOUND: Duration = Duration::from_secs(60);
/// Target of the overhead of HTTP at the p95 (REQ-006-39), reported.
const TARGET: Duration = Duration::from_millis(10);
/// What a browser accepts.
const ACCEPT_ENCODING: &str = "gzip, deflate, br, zstd";
/// A write applied by its author, who may (spec 024, REQ-024-13), as the browser sends it:
/// without `approval=self`, the changeset is held as a proposal.
const CHANGESETS: &str = "/v1/changesets?approval=self";

/// The keys the issuer publishes.
struct Published(Vec<Jwk>);

impl KeySource for Published {
    fn keys(&self, _: &IssuerConfig) -> impl Future<Output = Result<JwkSet, TokenError>> + Send {
        std::future::ready(Ok(JwkSet {
            keys: self.0.clone(),
        }))
    }
}

struct Ready;

impl Readiness for Ready {
    fn ready(&self) -> std::pin::Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(std::future::ready(true))
    }
}

fn issuer() -> IssuerConfig {
    IssuerConfig {
        id: "main".into(),
        issuer: ISSUER.into(),
        client_id: "kartenn".into(),
        client_secret: "bench".into(),
        audience: "kartenn".into(),
        groups_claim: "groups".into(),
        algorithms: vec![Algorithm::ES256],
        native: None,
    }
}

fn groups() -> Vec<String> {
    (0..GROUPS).map(|g| format!("g{g:02}")).collect()
}

/// One HTTP/1.1 connection, kept alive.
struct Client {
    address: SocketAddr,
    sender: http1::SendRequest<Full<Bytes>>,
}

impl Client {
    async fn connect(address: SocketAddr) -> anyhow::Result<Self> {
        let tcp = TcpStream::connect(address).await?;
        tcp.set_nodelay(true)?;
        let (sender, connection) = http1::handshake(TokioIo::new(tcp)).await?;
        tokio::spawn(connection);
        Ok(Self { address, sender })
    }

    /// Ready to send: as a browser does, a new connection when the server closed the
    /// idle one (hyper's `header_read_timeout`, 30 s), outside the measure.
    async fn ready(&mut self) -> anyhow::Result<()> {
        if self.sender.is_closed() || self.sender.ready().await.is_err() {
            *self = Self::connect(self.address).await?;
        }
        Ok(())
    }

    /// Sends `request` once [`Self::ready`] and reads the whole response, still
    /// compressed.
    async fn send(&mut self, request: Request<Full<Bytes>>) -> anyhow::Result<(StatusCode, Bytes)> {
        let response = self
            .sender
            .send_request(request)
            .await
            .context("sending a request")?;
        let status = response.status();
        Ok((status, response.into_body().collect().await?.to_bytes()))
    }
}

/// A person of the bench, with its connection.
struct User {
    subject: Subject,
    /// `Authorization` or `Cookie`, with its value.
    proof: (header::HeaderName, String),
    /// The client address the proxy forwards.
    forwarded: String,
    client: Client,
}

impl User {
    /// A request for `path`, as the proxy forwards it.
    fn request(&self, method: &str, path: &str, body: Option<String>) -> Request<Full<Bytes>> {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, "kartenn.bench.invalid")
            .header(header::FORWARDED, &self.forwarded)
            .header(header::ACCEPT, "application/json")
            .header(header::ACCEPT_ENCODING, ACCEPT_ENCODING)
            .header(&self.proof.0, &self.proof.1);
        if self.proof.0 == header::COOKIE {
            builder = builder.header("sec-fetch-site", "same-origin");
        }
        let body = match body {
            Some(body) => {
                builder = builder.header(header::CONTENT_TYPE, "application/json");
                Full::new(Bytes::from(body))
            }
            None => Full::new(Bytes::new()),
        };
        builder.body(body).expect("valid parts")
    }

    /// `GET path` with the headers `extra`: its duration until the whole body, its status,
    /// its `ETag` and its body (as sent: compressed unless `Accept-Encoding` is removed).
    async fn get_with(
        &mut self,
        path: &str,
        extra: &[(header::HeaderName, &str)],
        identity: bool,
    ) -> anyhow::Result<(Duration, StatusCode, Option<String>, Bytes)> {
        let mut request = self.request("GET", path, None);
        if identity {
            request.headers_mut().remove(header::ACCEPT_ENCODING);
        }
        for (name, value) in extra {
            request
                .headers_mut()
                .insert(name.clone(), header::HeaderValue::from_str(value)?);
        }
        self.client.ready().await?;
        let t = Instant::now();
        let response = self
            .client
            .sender
            .send_request(request)
            .await
            .context("sending a request")?;
        let status = response.status();
        let etag = response
            .headers()
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = response.into_body().collect().await?.to_bytes();
        Ok((t.elapsed(), status, etag, body))
    }

    /// The duration of `method path` until the whole body, or why it did not answer
    /// `expected`.
    async fn timed(
        &mut self,
        method: &str,
        path: &str,
        body: Option<String>,
        expected: StatusCode,
    ) -> anyhow::Result<Duration> {
        let request = self.request(method, path, body);
        self.client.ready().await?;
        let t = Instant::now();
        let (status, body) = self.client.send(request).await?;
        let elapsed = t.elapsed();
        if status != expected {
            bail!(
                "{method} {path}: {status}, {}",
                String::from_utf8_lossy(&body[..body.len().min(300)])
            );
        }
        Ok(elapsed)
    }
}

/// The application of the server, on a local port, until `stop` is dropped.
struct Served {
    address: SocketAddr,
    _stop: tokio::sync::oneshot::Sender<()>,
    signing: EncodingKey,
    ring: KeyRing,
}

/// Serves the router as `kartenn-server` composes it: its own pool bounded as the
/// server's (default size), the real authentication with a published key, the default
/// quotas, the loopback as the trusted proxy.
async fn serve(database: &str) -> anyhow::Result<Served> {
    let options = kartenn_server::app::bounded(PgConnectOptions::from_str(database)?, WRITE_BOUND);
    let pool = PgPool::connect_with(options).await?;
    let secret = p256::SecretKey::random(&mut rand_core::OsRng);
    let pem = secret.to_pkcs8_pem(p256::pkcs8::LineEnding::default())?;
    let signing = EncodingKey::from_ec_pem(pem.as_bytes())?;
    let mut jwk = Jwk::from_encoding_key(&signing, Algorithm::ES256)?;
    jwk.common.key_id = Some("k1".into());
    let ring = KeyRing::parse(&KeyRing::generate_line("k1")).map_err(|e| anyhow!("{e}"))?;
    let journal = PgAuditJournal::new(pool.clone(), Instance::new("bench"));
    let authn = Authn {
        identities: PgStore::new(pool.clone()),
        journal: journal.clone(),
        ring: ring.clone(),
        oidc: Oidc::new(
            TokenVerifier::new(vec![issuer()], Published(vec![jwk])),
            HttpClient::new()?,
            ring.clone(),
            "https://kartenn.bench.invalid/v1/auth/callback".into(),
        ),
        states: StateCache::new(Duration::from_secs(5)),
        limiter: FailureLimiter::new(20, Duration::from_mins(1)),
        aggregator: FailureAggregator::new(10, Duration::from_mins(1)),
        idle: Duration::from_mins(30),
        absolute: Duration::from_hours(8),
    };
    let max_in_flight = 2 * pool.options().get_max_connections() as usize;
    let api = kartenn_rest::objects::Api {
        pool,
        ring: ring.clone(),
        bound: WRITE_BOUND,
        journal,
        audit: ReadAudit::default(),
        sources: Sources::none(),
        outputs: Arc::from(Vec::new()),
        modules: Arc::default(),
    };
    let app = kartenn_rest::router(
        kartenn_rest::v1(api),
        Arc::new(Ready),
        Settings {
            trusted_proxies: vec!["127.0.0.0/8".parse().map_err(|e| anyhow!("{e:?}"))?],
            max_in_flight,
            public_origin: Some("https://kartenn.bench.invalid".into()),
            write_transaction_timeout: WRITE_BOUND,
            ..Settings::default()
        },
        Arc::new(Quotas::new(Rates::default())),
        Arc::new(authn),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(kartenn_server::http::serve(listener, app, None, async {
        let _ = stopped.await;
    }));
    Ok(Served {
        address,
        _stop: stop,
        signing,
        ring,
    })
}

fn unix_now() -> anyhow::Result<i64> {
    Ok(i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
    )?)
}

/// The users, each known to the issuer by its subject with the 50 groups; the even ones
/// prove themselves by an access token, the odd ones by a session.
async fn users(served: &Served, store: &PgStore) -> anyhow::Result<Vec<User>> {
    let mut users = Vec::with_capacity(USERS);
    for k in 0..USERS {
        let subject = format!("u-{k}");
        let (identity, _) = store
            .upsert_person(ISSUER, &subject, &format!("Person {k}"))
            .await
            .map_err(|e| anyhow!("{e:?}"))?;
        let proof = if k % 2 == 0 {
            let now = unix_now()?;
            let claims = json!({
                "iss": ISSUER, "aud": "kartenn", "sub": subject, "client_id": "bench",
                "iat": now, "exp": now + 7_200, "jti": Uuid::now_v7(),
                "name": format!("Person {k}"), "groups": groups()
            });
            let mut header = Header::new(Algorithm::ES256);
            header.kid = Some("k1".into());
            header.typ = Some("at+jwt".into());
            (
                header::AUTHORIZATION,
                format!("Bearer {}", encode(&header, &claims, &served.signing)?),
            )
        } else {
            let login = LoggedIn {
                issuer_id: "main".into(),
                issuer: ISSUER.into(),
                subject,
                name: None,
                groups: groups(),
                refresh_token: None,
                id_token: "id".into(),
                expires_in: 3_600,
            };
            let chunks = Session::new(identity, &login)
                .seal(&served.ring)
                .map_err(|e| anyhow!("{e:?}"))?;
            let cookie = match chunks.as_slice() {
                [one] => format!("__Host-kartenn={one}"),
                many => many
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("__Host-kartenn-{}={c}", i + 1))
                    .collect::<Vec<_>>()
                    .join("; "),
            };
            (header::COOKIE, cookie)
        };
        users.push(User {
            subject: Subject {
                id: identity,
                issuer: Some(ISSUER.into()),
                groups: groups(),
            },
            proof,
            // The benchmarking range (RFC 2544), one address per user.
            forwarded: format!("for=198.18.{}.{};proto=https", k / 250, k % 250 + 1),
            client: Client::connect(served.address).await?,
        });
    }
    Ok(users)
}

/// One budget measured both ways.
struct Pair {
    name: &'static str,
    limit: Duration,
    service: Vec<Duration>,
    http: Vec<Duration>,
}

impl Pair {
    const fn new(name: &'static str, limit: Duration) -> Self {
        Self {
            name,
            limit,
            service: Vec::new(),
            http: Vec::new(),
        }
    }

    /// Records a draw: the service first on even draws, HTTP first on odd ones, so that
    /// neither benefits from the cache warmed by the other.
    async fn measure<S, H>(&mut self, i: usize, service: S, http: H) -> anyhow::Result<()>
    where
        S: AsyncFnOnce() -> anyhow::Result<()>,
        H: AsyncFnOnce() -> anyhow::Result<Duration>,
    {
        if i.is_multiple_of(2) {
            let t = Instant::now();
            service().await?;
            self.service.push(t.elapsed());
            self.http.push(http().await?);
        } else {
            self.http.push(http().await?);
            let t = Instant::now();
            service().await?;
            self.service.push(t.elapsed());
        }
        Ok(())
    }
}

fn rfc3339(at: OffsetDateTime) -> anyhow::Result<String> {
    Ok(at.format(&Rfc3339)?)
}

/// The body of a unitary changeset: a new address in `network`, at its first free one.
async fn new_address(pool: &PgPool, network: (Uuid, Uuid)) -> anyhow::Result<(Operation, String)> {
    let free = next_free_address(pool, network.0, OffsetDateTime::now_utc())
        .await?
        .context("a network of the dataset is full")?
        .to_string();
    let body = json!({
        "motive": "bench",
        "operations": [{"op": "create_address", "local": null, "domain": {"id": network.1},
                        "address": free, "usage": "host"}]
    })
    .to_string();
    let operation = Operation::CreateAddress {
        local: None,
        domain: Ref::Id(network.1),
        address: free,
        usage: AddressUsage::Host,
    };
    Ok((operation, body))
}

/// What a response of the load tells.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Answer {
    Ok,
    Shed,
    /// A 5xx but the shedding.
    Error,
    /// A refusal: a quota or a fault of the scenario.
    Refused,
}

fn answer(status: StatusCode, body: &[u8]) -> Answer {
    if status.is_success() {
        Answer::Ok
    } else if status == StatusCode::SERVICE_UNAVAILABLE
        && String::from_utf8_lossy(body).contains("in flight")
    {
        Answer::Shed
    } else if status.is_server_error() {
        Answer::Error
    } else {
        Answer::Refused
    }
}

/// The samples of a task of the load: reads, then writes.
#[derive(Default)]
struct Load {
    reads: Vec<Duration>,
    writes: Vec<Duration>,
    answers: Vec<Answer>,
}

impl Load {
    async fn record(
        &mut self,
        user: &mut User,
        write: bool,
        method: &str,
        path: &str,
        body: Option<String>,
    ) -> anyhow::Result<()> {
        let request = user.request(method, path, body);
        user.client.ready().await?;
        let t = Instant::now();
        let (status, body) = user.client.send(request).await?;
        let elapsed = t.elapsed();
        let answer = answer(status, &body);
        if answer == Answer::Ok {
            if write {
                &mut self.writes
            } else {
                &mut self.reads
            }
            .push(elapsed);
        } else if answer != Answer::Shed {
            progress(&format!(
                "load: {method} {path}: {status}, {}",
                String::from_utf8_lossy(&body[..body.len().min(200)])
            ));
        }
        self.answers.push(answer);
        Ok(())
    }
}

/// Loads, binds, serves and measures what `selection` runs; returns the report and whether
/// every budget holds through HTTP.
///
/// # Errors
///
/// Database, server or client failures, a scenario refused.
#[allow(clippy::too_many_lines)] // one linear scenario, step by step
pub async fn run(
    database: &str,
    data: &Dataset,
    seed: u64,
    selection: &Selection,
) -> anyhow::Result<(String, bool)> {
    let started = Instant::now();
    let Prepared {
        pool,
        author: _,
        rack,
        mut report,
    } = prepare(database, data, seed).await?;
    let store = PgStore::new(pool.clone());
    let served = serve(database).await?;
    let mut users = users(&served, &store).await?;
    // The first user is the person of `bench-rbac`, with its own bindings.
    bind(&pool, data, users[0].subject.id, seed).await?;
    progress(&format!("bindings, users: {:?}", started.elapsed()));
    let authorizer = PgAuthorizer::new(pool.clone());
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(1));
    let now = OffsetDateTime::now_utc();
    // HISTORY_FROM of `bench-core`, 2023-10-01T00:00Z.
    let from = OffsetDateTime::from_unix_timestamp(1_696_118_400)?;
    let day = Duration::from_hours(24);
    let draw = |rng: &mut ChaCha8Rng| {
        if rng.random_range(0..4) == 0 {
            (
                "prefixes",
                data.prefixes[rng.random_range(0..data.prefixes.len())].id,
            )
        } else {
            (
                "addresses",
                data.addresses[rng.random_range(0..data.addresses.len())].id,
            )
        }
    };
    let kind = |collection: &str| {
        if collection == "prefixes" {
            "prefix"
        } else {
            "address"
        }
    };
    let decided = async |subject: &Subject, permission: String, id: Uuid| {
        let outside = authorizer
            .authorize_many(subject, &permission, &[id])
            .await
            .map_err(|e| anyhow!(e.0))?;
        if outside.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("the person cannot read {id}"))
        }
    };

    // Warm-up of the connections, the statements and the identities, both ways.
    for (k, user) in users.iter_mut().enumerate() {
        let (collection, id) = draw(&mut rng);
        object(&pool, id, now, None).await?;
        let as_of = (k % 2 == 0)
            .then(|| rfc3339(from + day * 100))
            .transpose()?;
        let path = match as_of {
            Some(as_of) => format!("/v1/{collection}/{id}?as_of={as_of}"),
            None => format!("/v1/{collection}/{id}"),
        };
        user.timed("GET", &path, None, StatusCode::OK).await?;
    }
    let mut turn = 0;
    let mut next = || {
        turn += 1;
        turn % USERS
    };

    let mut pairs = Vec::new();
    // Reading an object.
    let mut current = None;
    if selection.runs(READ) {
        let mut read = Pair::new("read an object", Duration::from_millis(50));
        for i in 0..200 {
            let (collection, id) = draw(&mut rng);
            let user = &mut users[next()];
            let subject = user.subject.clone();
            read.measure(
                i,
                async || {
                    object(&pool, id, now, None)
                        .await?
                        .context("a loaded object is missing")?;
                    decided(&subject, format!("core.{}.read", kind(collection)), id).await
                },
                async || {
                    user.timed(
                        "GET",
                        &format!("/v1/{collection}/{id}"),
                        None,
                        StatusCode::OK,
                    )
                    .await
                },
            )
            .await?;
        }
        current = Some(p95(read.http.clone()));
        pairs.push(read);
    }

    // Reading an object as known in the past (INV-33): twice the current read through
    // HTTP (floor 5 ms).
    if selection.runs(READ_PAST) {
        let current = current.context("the current read is a prerequisite")?;
        let mut dated = Pair::new(
            "read an object as known in the past",
            (current * 2).max(Duration::from_millis(5)),
        );
        for i in 0..200 {
            let (collection, id) = draw(&mut rng);
            let at = from + day * rng.random_range(0..700);
            let as_of = rfc3339(at)?;
            let user = &mut users[next()];
            let subject = user.subject.clone();
            dated
                .measure(
                    i,
                    async || {
                        object(&pool, id, now, Some(at))
                            .await?
                            .context("a past version is missing")?;
                        decided(&subject, format!("core.{}.read", kind(collection)), id).await
                    },
                    async || {
                        user.timed(
                            "GET",
                            &format!("/v1/{collection}/{id}?as_of={as_of}"),
                            None,
                            StatusCode::OK,
                        )
                        .await
                    },
                )
                .await?;
        }
        pairs.push(dated);
    }

    // Filtered pages of 100.
    if selection.runs(FILTERED_PAGE) {
        let mut page = Pair::new("filtered page of 100", Duration::from_millis(150));
        for i in 0..100 {
            let domain = &data.domains[rng.random_range(0..data.domains.len())];
            let low: i64 = rng.random_range(0..900_000);
            let user = &mut users[next()];
            let subject = user.subject.clone();
            page.measure(
                i,
                async || {
                    let scope = authorizer
                        .read_scope(&subject, now)
                        .await
                        .map_err(|e| anyhow!(e.0))?;
                    let condition = Condition::Between(
                        Some(low.to_string()),
                        Some((low + 100_000).to_string()),
                    );
                    filter_in(&pool, rack, domain.id, &condition, now, None, 100, &scope).await?;
                    Ok(())
                },
                async || {
                    let path = format!(
                        "/v1/addresses?domain={}&prop.custom.rack:between={low}..{}&page_size=100",
                        domain.slug,
                        low + 100_000
                    );
                    user.timed("GET", &path, None, StatusCode::OK).await
                },
            )
            .await?;
        }
        pairs.push(page);
    }

    // Subtree (a page of the direct content) and occupancy, now and in the past.
    if selection.runs(SUBTREE) {
        let mut subtree = Pair::new("subtree and occupancy", Duration::from_millis(200));
        let mut dated_subtree = Pair::new(
            "subtree and occupancy in the past",
            Duration::from_millis(200),
        );
        for i in 0..400 {
            let p = data.prefixes[rng.random_range(0..data.prefixes.len())].id;
            let at = (i % 2 == 1).then(|| from + day * rng.random_range(0..700));
            let query = at
                .map(|at| rfc3339(at).map(|a| format!("?as_of={a}")))
                .transpose()?
                .unwrap_or_default();
            let user = &mut users[next()];
            let subject = user.subject.clone();
            let pair = if at.is_some() {
                &mut dated_subtree
            } else {
                &mut subtree
            };
            pair.measure(
                i / 2,
                async || {
                    let scope = authorizer
                        .read_scope(&subject, now)
                        .await
                        .map_err(|e| anyhow!(e.0))?;
                    children_as_of(&pool, p, now, at, &scope).await?;
                    occupancy_as_of(&pool, p, now, at)
                        .await?
                        .context("a loaded prefix is missing")?;
                    Ok(())
                },
                async || {
                    let children = user
                        .timed(
                            "GET",
                            &format!("/v1/prefixes/{p}/children{query}"),
                            None,
                            StatusCode::OK,
                        )
                        .await?;
                    let occupancy = user
                        .timed(
                            "GET",
                            &format!("/v1/prefixes/{p}/occupancy{query}"),
                            None,
                            StatusCode::OK,
                        )
                        .await?;
                    Ok(children + occupancy)
                },
            )
            .await?;
        }
        pairs.push(subtree);
        pairs.push(dated_subtree);
    }

    // Unitary changesets: a new address in a random network, each way.
    let networks: Vec<(Uuid, Uuid)> = data
        .prefixes
        .iter()
        .filter(|p| p.nature == Nature::Network)
        .map(|p| (p.id, data.domains[p.domain].id))
        .collect();
    if selection.runs(UNITARY) {
        let mut changeset = Pair::new("unitary changeset", Duration::from_millis(200));
        for i in 0..100 {
            let (operation, _) =
                new_address(&pool, networks[rng.random_range(0..networks.len())]).await?;
            let (_, body) =
                new_address(&pool, networks[rng.random_range(0..networks.len())]).await?;
            let user = &mut users[next()];
            let subject = user.subject.clone();
            changeset
                .measure(
                    i,
                    async || {
                        let request = ChangesetRequest {
                            author: subject.id,
                            motive: "bench".into(),
                            period: None,
                            operations: vec![operation],
                            simulate: false,
                            idempotency_key: None,
                            approval_self: true,
                        };
                        match submit_in(
                            &store,
                            &authorizer,
                            &Sources::none(),
                            &RequestContext::of(&subject),
                            &request,
                            MAX_OPERATIONS,
                        )
                        .await?
                        {
                            Outcome::Applied { .. } => Ok(()),
                            other => bail!("changeset not applied: {other:?}"),
                        }
                    },
                    async || {
                        user.timed("POST", CHANGESETS, Some(body), StatusCode::CREATED)
                            .await
                    },
                )
                .await?;
        }
        pairs.push(changeset);
    }
    // The dictionary of labels (spec 018, REQ-018-20): the whole of one language, then
    // the same under its `ETag` (304); the stored labels read by the service.
    let mut largest = None;
    if selection.runs(LABELS) {
        let mut dictionary = Pair::new(
            "dictionary of labels, one language",
            Duration::from_millis(50),
        );
        let mut unchanged = Pair::new("dictionary of labels, 304", Duration::from_millis(10));
        let mut size = 0;
        for i in 0..200 {
            let lang = if i % 2 == 0 { "fr" } else { "en" };
            let path = format!("/v1/labels?lang={lang}");
            let user = &mut users[next()];
            let mut etag = None;
            let stored = async || {
                kartenn_store_pg::labels_read::stored(&pool, OffsetDateTime::now_utc(), None, &[])
                    .await
                    .map(drop)
                    .map_err(|e| anyhow!(e))
            };
            dictionary
                .measure(i, stored, async || {
                    let (elapsed, status, tag, body) = user.get_with(&path, &[], i < 2).await?;
                    if status != StatusCode::OK {
                        bail!("GET {path}: {status}");
                    }
                    if i < 2 {
                        size = size.max(body.len());
                    }
                    etag = tag;
                    Ok(elapsed)
                })
                .await?;
            let etag = etag.context("the dictionary has an ETag")?;
            unchanged
                .measure(i, stored, async || {
                    let (elapsed, status, _, _) = user
                        .get_with(&path, &[(header::IF_NONE_MATCH, etag.as_str())], false)
                        .await?;
                    if status != StatusCode::NOT_MODIFIED {
                        bail!("GET {path} under its ETag: {status}");
                    }
                    Ok(elapsed)
                })
                .await?;
        }
        largest = Some(size);
        pairs.push(dictionary);
        pairs.push(unchanged);
    }
    progress(&format!("scenarios: {:?}", started.elapsed()));

    let mut ok = true;
    // REQ-018-20: less than 1 MB uncompressed.
    if let Some(largest) = largest {
        let small = largest < 1_000_000;
        ok &= small;
        writeln!(
            report,
            "{} dictionary of labels, one language: {largest} bytes uncompressed, budget < 1 MB",
            if small { "PASS" } else { "FAIL" },
        )?;
    }
    for pair in pairs {
        let service = p95(pair.service);
        let http = p95(pair.http);
        let overhead = http.saturating_sub(service);
        let pass = http <= pair.limit;
        ok &= pass;
        writeln!(
            report,
            "{} {}, p95: {http:?} through HTTP, budget {:?} ({service:?} through the service, +{overhead:?}, target < {TARGET:?}: {})",
            if pass { "PASS" } else { "FAIL" },
            pair.name,
            pair.limit,
            if overhead < TARGET { "met" } else { "missed" },
        )?;
    }

    // The load: every user reads with a think time; the writers write once a second on
    // networks of their own as well, on a connection of their own.
    if selection.runs(LOAD_TEST) {
        let paths = Arc::new(
        (0..2_000)
            .map(|i| {
                let (collection, id) = draw(&mut rng);
                match i % 4 {
                    0 | 1 => format!("/v1/{collection}/{id}"),
                    2 => {
                        let domain = &data.domains[rng.random_range(0..data.domains.len())];
                        let low: i64 = rng.random_range(0..900_000);
                        format!(
                            "/v1/addresses?domain={}&prop.custom.rack:between={low}..{}&page_size=100",
                            domain.slug,
                            low + 100_000
                        )
                    }
                    _ => {
                        let p = data.prefixes[rng.random_range(0..data.prefixes.len())].id;
                        format!("/v1/prefixes/{p}/children")
                    }
                }
            })
            .collect::<Vec<_>>(),
    );
        let mut tasks: Vec<JoinHandle<anyhow::Result<Load>>> = Vec::new();
        let begun = Instant::now();
        for (k, mut user) in users.into_iter().enumerate() {
            if k < WRITERS {
                let mut writer = User {
                    subject: user.subject.clone(),
                    proof: user.proof.clone(),
                    forwarded: user.forwarded.clone(),
                    client: Client::connect(served.address).await?,
                };
                let mine: Vec<(Uuid, Uuid)> = networks
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| i % WRITERS == k)
                    .map(|(_, n)| *n)
                    .collect();
                let pool = pool.clone();
                let offset = Duration::from_millis(rng.random_range(0..1_000));
                tasks.push(tokio::spawn(async move {
                    let mut load = Load::default();
                    let mut ticks = tokio::time::interval_at(
                        tokio::time::Instant::from_std(begun + offset),
                        Duration::from_secs(1),
                    );
                    ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
                    let mut i = 0;
                    loop {
                        ticks.tick().await;
                        if begun.elapsed() >= LOAD {
                            break;
                        }
                        let (_, body) = new_address(&pool, mine[i % mine.len()]).await?;
                        load.record(&mut writer, true, "POST", CHANGESETS, Some(body))
                            .await?;
                        i += 1;
                    }
                    Ok(load)
                }));
            }
            let paths = paths.clone();
            let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(100 + k as u64));
            tasks.push(tokio::spawn(async move {
                let mut load = Load::default();
                while begun.elapsed() < LOAD {
                    tokio::time::sleep(Duration::from_millis(rng.random_range(500..1_500))).await;
                    let path = &paths[rng.random_range(0..paths.len())];
                    load.record(&mut user, false, "GET", path, None).await?;
                }
                Ok(load)
            }));
        }
        let mut total = Load::default();
        for task in tasks {
            let load = task.await??;
            total.reads.extend(load.reads);
            total.writes.extend(load.writes);
            total.answers.extend(load.answers);
        }
        let elapsed = begun.elapsed();
        progress(&format!("load: {:?}", started.elapsed()));
        let count = |a: Answer| total.answers.iter().filter(|x| **x == a).count();
        let (sheddings, errors, refused) = (
            count(Answer::Shed),
            count(Answer::Error),
            count(Answer::Refused),
        );
        let writes = total.writes.len();
        let rate = f64::from(u32::try_from(writes)?) / elapsed.as_secs_f64();
        let sustained = rate >= 50.0;
        let clean = errors == 0 && refused == 0;
        ok &= sustained && clean;
        writeln!(
            report,
            "{} sustained writes through HTTP: {rate:.1}/s ({writes} in {elapsed:?}, {WRITERS} writers at 1/s, at least 50/s)",
            if sustained { "PASS" } else { "FAIL" },
        )?;
        writeln!(
            report,
            "{} {USERS} concurrent users: {} requests, {errors} 5xx but the shedding, {refused} refused, {sheddings} shed; p95 of a read {:?}, of a write {:?}",
            if clean { "PASS" } else { "FAIL" },
            total.answers.len(),
            p95(total.reads),
            p95(total.writes),
        )?;
    }
    Ok((report, ok))
}
