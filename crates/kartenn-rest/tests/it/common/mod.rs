//! What the tests of the layers share: an authenticator that accepts any access token,
//! for the tests whose subject is not the authentication.

#![allow(dead_code)] // each test binary uses its own part of it

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::response::Response;
use sqlx::PgPool;
use tower::ServiceExt;

use axum::http::{HeaderValue, Request, header};
use kartenn_core::audit::Source;
use kartenn_rest::authn::{Answer, Authenticated, Authenticator, Proof, Refusal};
use kartenn_service::authn::{IdentityKind, Method, Principal};
use uuid::Uuid;

/// Any `Bearer` token is the same person; any other proof is refused.
pub(crate) struct Anyone(pub Uuid);

impl Authenticator for Anyone {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let principal = match proof {
            Proof::Bearer(_) => Ok(Principal {
                identity: self.0,
                kind: IdentityKind::Person,
                issuer: Some("https://idp.example.invalid".into()),
                groups: Vec::new(),
                method: Method::Bearer,
            }
            .into()),
            _ => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(principal))
    }
}

/// Any `Bearer` token is this principal; any other proof is refused.
pub(crate) struct As(pub Principal);

impl Authenticator for As {
    fn authenticate(&self, proof: Proof, _: Source) -> Answer<'_, Authenticated> {
        let principal = match proof {
            Proof::Bearer(_) => Ok(self.0.clone().into()),
            _ => Err(Refusal::Unauthenticated),
        };
        Box::pin(std::future::ready(principal))
    }
}

/// [`Anyone`], as the router takes it.
pub(crate) fn anyone() -> Arc<dyn Authenticator> {
    Arc::new(Anyone(Uuid::now_v7()))
}

/// `request` as a test writes: applied by its author when it may (`approval=self`, spec
/// 024, REQ-024-13), unless it names `approval` itself.
pub(crate) fn applied<B>(mut request: Request<B>) -> Request<B> {
    let uri =
        kartenn_testing::applied_by_author(request.method().as_str(), &request.uri().to_string());
    *request.uri_mut() = uri.parse().expect("a URI");
    request
}

/// `request` with an access token.
pub(crate) fn bearer<B>(mut request: Request<B>) -> Request<B> {
    request.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer test"),
    );
    request
}

/// The answer of `app` to `request`, a page holding the object `id`, with a write of the
/// version of `id` validated between the read of the rows of the page and that of their
/// versions (task 021-11c); the version written.
///
/// The test holds a lock on `object_version`, which only the reads of versions read:
/// the request reads its rows, then waits on the lock at its versions. Once it waits, the
/// locking transaction sets the version of `id` to another changeset and commits, which
/// frees the lock. Nothing in the code served knows of it.
pub(crate) async fn served_across_a_write(
    pool: &PgPool,
    app: &Router,
    request: Request<Body>,
    id: Uuid,
) -> (Response, Uuid) {
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE object_version IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *lock)
        .await
        .unwrap();
    let served = tokio::spawn(app.clone().oneshot(request));
    // The test databases are copies of one template: the same relation has the same
    // identifier in each, hence the database named.
    let waiting = "SELECT EXISTS (SELECT 1 FROM pg_locks
                    WHERE database = (SELECT oid FROM pg_database
                                       WHERE datname = current_database())
                      AND relation = 'object_version'::regclass AND NOT granted)";
    let started = std::time::Instant::now();
    while !sqlx::query_scalar::<_, bool>(waiting)
        .fetch_one(pool)
        .await
        .unwrap()
    {
        assert!(
            !served.is_finished(),
            "the request read no version after its rows"
        );
        assert!(started.elapsed() < Duration::from_secs(10), "no read waits");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let written: Uuid = sqlx::query_scalar(
        "SELECT c.id FROM changeset c
          WHERE c.id IS DISTINCT FROM (SELECT changeset_id FROM object_version
                                        WHERE object_id = $1)
          ORDER BY c.submitted_at LIMIT 1",
    )
    .bind(id)
    .fetch_one(&mut *lock)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO object_version (object_id, changeset_id) VALUES ($1, $2)
         ON CONFLICT (object_id) DO UPDATE SET changeset_id = EXCLUDED.changeset_id",
    )
    .bind(id)
    .bind(written)
    .execute(&mut *lock)
    .await
    .unwrap();
    lock.commit().await.unwrap();
    (served.await.unwrap().unwrap(), written)
}
