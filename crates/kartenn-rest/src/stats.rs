//! `GET /v1/stats` (spec 024, REQ-024-14 to 16; design « C »): the counters of the home
//! page in one call, each filtered by the read rights of the caller with the predicate of
//! its list; the modules add theirs through [`StatsContributor`]. What depends on the
//! rights alone is kept 30 s by (fingerprint of the rights, revision of the outbox), so that
//! callers of the same bindings share it; what is the caller's own (its pending
//! changesets, those it may apply, the counters of the modules, which may count what it
//! reads as itself) by (subject, fingerprint of the rights, revision). A count discloses no object: the
//! call journals no read.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::Extension;
use axum::extract::State;
use axum::http::{HeaderMap, header};
use axum::response::Response;
use futures_util::future::try_join_all;
use kartenn_service::authn::Principal;
use kartenn_service::authz::{Authorizer, Subject};
use kartenn_service::events::qualified;
use kartenn_service::module::{StatsContributor, StatsScope};
use kartenn_service::reads::{CHANGESET_READ_PERMISSION, CHANGESETS};
use kartenn_service::review::proposals_for;
use kartenn_store_pg::list::{self, Kind};
use kartenn_store_pg::modules::PgModuleRead;
use kartenn_store_pg::rbac::ReadScope;
use kartenn_store_pg::{anomalies, changesets, outbox, read};
use serde::Serialize;
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::changesets::write_store;
use crate::dated::{Dates, ETag, hex, respond};
use crate::objects::{Api, instant, store};
use crate::problem::{Problem, ProblemKind};

/// The days of [`ChangesetStats::by_day`].
pub const DAYS: u16 = 30;
/// The days of [`ChangesetStats::applied_7d`].
const WEEK: usize = 7;
/// How long a result is kept (REQ-024-16).
const KEPT: Duration = Duration::from_secs(30);
/// Most results kept per cache: beyond, it starts over.
const KEPT_MAX: usize = 1_024;

/// The changesets applied (Q-024-5: every changeset, whoever its author), by UTC day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct ChangesetStats {
    /// Applied over the last 7 days, today included.
    pub applied_7d: i64,
    /// Applied over the last 30 days, today included.
    pub applied_30d: i64,
    /// Applied each of the last 30 days, the oldest first and today last.
    pub by_day: Vec<i64>,
}

/// The pending changesets of the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct PendingStats {
    /// Those of others it may apply (`<resource>.apply` on everything they touch), those
    /// of `GET /v1/proposals`.
    pub to_apply: usize,
    /// Its own.
    pub mine: i64,
}

/// The open anomalies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct AnomalyStats {
    /// Open anomalies whose object the caller reads.
    pub open: i64,
}

/// The body of `GET /v1/stats`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Stats {
    /// The objects the caller reads, by qualified kind (`core.prefix`, `vlan.vlan`); a
    /// kind it reads nowhere is absent.
    pub objects: BTreeMap<String, i64>,
    /// The changesets applied; absent without `core.changeset.read`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changesets: Option<ChangesetStats>,
    /// The pending changesets of the caller.
    pub pending: PendingStats,
    /// The open anomalies.
    pub anomalies: AnomalyStats,
    /// The counters of each module, under its name; a module that counts nothing the
    /// caller reads is absent.
    pub modules: BTreeMap<String, BTreeMap<String, u64>>,
}

/// What depends on the rights alone.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shared {
    objects: BTreeMap<String, i64>,
    changesets: Option<ChangesetStats>,
    anomalies: AnomalyStats,
}

/// What depends on the caller itself: its changesets, and the counters of the modules
/// (the flows it requested, REQ-011-17).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Own {
    pending: PendingStats,
    /// Counts of module types that replace those of the core.
    objects: BTreeMap<String, i64>,
    modules: BTreeMap<String, BTreeMap<String, u64>>,
}

/// Results kept, by key, with when they were computed.
type Kept<T> = Mutex<HashMap<String, (Instant, Arc<T>)>>;

fn shared_kept() -> &'static Kept<Shared> {
    static KEPT_ONES: OnceLock<Kept<Shared>> = OnceLock::new();
    KEPT_ONES.get_or_init(Mutex::default)
}

fn own_kept() -> &'static Kept<Own> {
    static KEPT_ONES: OnceLock<Kept<Own>> = OnceLock::new();
    KEPT_ONES.get_or_init(Mutex::default)
}

fn lookup<T>(kept: &Kept<T>, key: &str) -> Option<Arc<T>> {
    kept.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(key)
        .filter(|(at, _)| at.elapsed() < KEPT)
        .map(|(_, found)| found.clone())
}

fn keep<T>(kept: &Kept<T>, key: String, value: Arc<T>) {
    let mut kept = kept
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if kept.len() >= KEPT_MAX {
        kept.retain(|_, (at, _)| at.elapsed() < KEPT);
        if kept.len() >= KEPT_MAX {
            kept.clear();
        }
    }
    kept.insert(key, (Instant::now(), value));
}

/// The rights a count depends on, and the database (tests share the instance).
fn rights_key(api: &Api, scope: &ReadScope, changesets: bool, revision: i64) -> String {
    let scope = scope
        .to_json()
        .map_or_else(|| "all".to_owned(), |json| json.to_string());
    let digest = Sha256::digest(scope.as_bytes());
    format!(
        "{}|{}|{changesets}|{revision}",
        database(api),
        hex(&digest[..16])
    )
}

/// The subject, and its rights.
fn subject_key(subject: &Subject, rights: &str) -> String {
    let mut groups = subject.groups.clone();
    groups.sort();
    groups.dedup();
    format!(
        "{rights}|{}|{}|{}",
        subject.id,
        subject.issuer.as_deref().unwrap_or_default(),
        groups.join("\u{1f}")
    )
}

fn database(api: &Api) -> String {
    api.pool
        .connect_options()
        .get_database()
        .unwrap_or_default()
        .to_owned()
}

/// The counts of the core under `scope` at `now`.
async fn shared(
    api: &Api,
    scope: &ReadScope,
    changesets_read: bool,
    now: OffsetDateTime,
) -> Result<Shared, Problem> {
    let core = try_join_all(
        Kind::ALL
            .into_iter()
            .filter(|kind| scope.reads(kind.as_str()))
            .map(|kind| async move {
                list::count(&api.pool, kind, now, scope)
                    .await
                    .map(|n| (qualified(kind.as_str()), n))
            }),
    );
    let types: Vec<&str> = api.modules.object_types().map(|t| t.kind).collect();
    let of_modules = list::module_counts(&api.pool, &types, now, scope);
    let open = anomalies::open_count(&api.pool, scope);
    let applied = async {
        if changesets_read {
            changesets::applied_by_day(&api.pool, DAYS).await.map(Some)
        } else {
            Ok(None)
        }
    };
    let (core, of_modules, open, applied) = futures_util::try_join!(
        async { core.await.map_err(|e| store(&e)) },
        async { of_modules.await.map_err(|e| store(&e)) },
        async { open.await.map_err(|e| store(&e)) },
        async { applied.await.map_err(Problem::from) },
    )?;
    let objects: BTreeMap<String, i64> = core.into_iter().chain(of_modules).collect();
    let changesets = applied.map(|by_day| ChangesetStats {
        applied_7d: by_day.iter().rev().take(WEEK).sum(),
        applied_30d: by_day.iter().sum(),
        by_day,
    });
    Ok(Shared {
        objects,
        changesets,
        anomalies: AnomalyStats { open },
    })
}

/// What each module of the build counts, on a connection of its own.
async fn contributions(
    api: &Api,
    subject: &Subject,
    scope: &ReadScope,
    now: OffsetDateTime,
) -> Result<Vec<(&'static str, kartenn_service::module::Contribution)>, Problem> {
    let json = scope.to_json();
    let reads = |kind: &str| scope.reads(kind);
    let stats_scope = StatsScope {
        identity: Some(subject.id),
        scope: json.as_ref(),
        reads: &reads,
    };
    let contributors: Vec<(&'static str, Arc<dyn StatsContributor>)> = api
        .modules
        .modules()
        .filter_map(|m| m.stats().map(|c| (m.name(), c)))
        .collect();
    let stats_scope = &stats_scope;
    try_join_all(
        contributors
            .into_iter()
            .map(|(name, contributor)| async move {
                let mut read = PgModuleRead::open(&api.pool, instant(now), None)
                    .await
                    .map_err(|e| store(&e))?;
                let contribution = contributor.contribute(&mut read, stats_scope).await?;
                Ok::<_, Problem>((name, contribution))
            }),
    )
    .await
}

/// The pending changesets of `subject` and what the modules count for it.
async fn own(
    api: &Api,
    subject: &Subject,
    scope: &ReadScope,
    now: OffsetDateTime,
) -> Result<Own, Problem> {
    let store_ = write_store(api);
    let authorizer = api.authorizer();
    let (to_apply, mine, contributed) = futures_util::try_join!(
        async {
            proposals_for(&store_, &authorizer, subject)
                .await
                .map_err(Problem::from)
        },
        async {
            changesets::pending_of(&api.pool, subject.id)
                .await
                .map_err(Problem::from)
        },
        contributions(api, subject, scope, now),
    )?;
    let mut objects = BTreeMap::new();
    let mut modules = BTreeMap::new();
    for (name, contribution) in contributed {
        // The module decides: it reads its types beyond the scope of the core.
        for (kind, n) in contribution.objects {
            objects.insert(kind.to_owned(), i64::try_from(n).unwrap_or(i64::MAX));
        }
        if !contribution.counters.is_empty() {
            modules.insert(
                name.to_owned(),
                contribution
                    .counters
                    .into_iter()
                    .map(|(k, n)| (k.to_owned(), n))
                    .collect(),
            );
        }
    }
    Ok(Own {
        pending: PendingStats {
            to_apply: to_apply.len(),
            mine,
        },
        objects,
        modules,
    })
}

/// Whether `subject` reads the changesets: without, the counts of the changesets are
/// left out (the refusal is not journaled: nothing is asked of them).
async fn changesets_read(api: &Api, subject: &Subject) -> Result<bool, Problem> {
    Ok(api
        .authorizer()
        .authorize_many(subject, CHANGESET_READ_PERMISSION, &[CHANGESETS])
        .await?
        .is_empty())
}

/// The statistics of `subject` at `now`, computed without the cache: what the route
/// serves, and what the bench measures (a cold cache, REQ-024-16).
///
/// # Errors
///
/// The database unavailable.
pub async fn compute(api: &Api, subject: &Subject, now: OffsetDateTime) -> Result<Stats, Problem> {
    let scope = api.authorizer().read_scope(subject, now).await?;
    let changesets_read = changesets_read(api, subject).await?;
    let (shared, own) = futures_util::try_join!(
        shared(api, &scope, changesets_read, now),
        own(api, subject, &scope, now),
    )?;
    Ok(assemble(&shared, &own))
}

fn assemble(shared: &Shared, own: &Own) -> Stats {
    let mut objects = shared.objects.clone();
    objects.extend(own.objects.iter().map(|(k, n)| (k.clone(), *n)));
    Stats {
        objects,
        changesets: shared.changesets.clone(),
        pending: own.pending.clone(),
        anomalies: shared.anomalies.clone(),
        modules: own.modules.clone(),
    }
}

/// `GET /v1/stats`: the counters of the home page (spec 024, REQ-024-14 to 16).
#[utoipa::path(
    get,
    path = "/stats",
    operation_id = "stats",
    tag = "operations",
    responses(
        (status = 200, description = "The counters, each filtered by the read rights of the \
            caller; a kind it reads nowhere is absent, `changesets` without \
            `core.changeset.read`. Kept up to 30 s; the event `stats` of `GET /v1/events` \
            says when to read again. Its strong ETag is a digest of the body.", body = Stats),
        (status = 304, description = "Unchanged since the `ETag` of `If-None-Match`."),
    )
)]
async fn stats(
    State(api): State<Api>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let subject = principal.subject();
    let now = read::now(&api.pool).await.map_err(|e| store(&e))?;
    let moment = Dates::default().at(now, api.bound)?;
    let revision = outbox::revision(&api.pool).await?;
    let scope = api.authorizer().read_scope(&subject, now).await?;
    let changesets_read = changesets_read(&api, &subject).await?;
    let shared_key = rights_key(&api, &scope, changesets_read, revision);
    let own_key = subject_key(&subject, &shared_key);
    let found_shared = lookup(shared_kept(), &shared_key);
    let found_own = lookup(own_kept(), &own_key);
    let shared_part = async {
        if let Some(found) = found_shared {
            return Ok(found);
        }
        let built = Arc::new(shared(&api, &scope, changesets_read, now).await?);
        keep(shared_kept(), shared_key, built.clone());
        Ok(built)
    };
    let own_part = async {
        if let Some(found) = found_own {
            return Ok(found);
        }
        let built = Arc::new(own(&api, &subject, &scope, now).await?);
        keep(own_kept(), own_key, built.clone());
        Ok::<_, Problem>(built)
    };
    let (shared_part, own_part) = futures_util::try_join!(shared_part, own_part)?;
    let body = assemble(&shared_part, &own_part);
    let bytes = serde_json::to_vec(&body).map_err(|_| Problem::new(ProblemKind::Internal))?;
    let etag = ETag::fingerprint(hex(&Sha256::digest(&bytes)[..16]));
    let response = ([(header::CONTENT_TYPE, "application/json")], bytes);
    Ok(respond(&headers, &moment, &etag, response))
}

pub(crate) fn routes() -> OpenApiRouter<Api> {
    OpenApiRouter::new().routes(routes!(stats))
}
