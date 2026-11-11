//! The fake API server replays what the connector must survive (réf. § API, REQ-019-19,
//! 28), driven through the client of the connector.

use futures_util::{StreamExt, TryStreamExt};
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
    SelfSubjectRulesReview, SelfSubjectRulesReviewSpec,
};
use k8s_openapi::api::core::v1::Namespace;
use kube::api::{ListParams, PostParams, WatchEvent, WatchParams};
use kube::{Api, ResourceExt};
use serde_json::json;

use crate::fake_api::{FakeApi, GoneAs, Rule};
use crate::support::cluster;

const NAMESPACES: &str = "/api/v1/namespaces";

fn namespace(name: &str) -> serde_json::Value {
    json!({"metadata": {"name": name, "uid": format!("uid-{name}")}})
}

fn setup() -> (FakeApi, kube::Client, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let cluster = cluster(dir.path(), "https://192.0.2.10:6443", "golf-0007");
    let fake = FakeApi::new();
    fake.collection(NAMESPACES, "v1", "Namespace");
    let client = kartenn_kube_sync::connect_with(&cluster, fake.clone()).unwrap();
    (fake, client, dir)
}

fn names(list: &[Namespace]) -> Vec<String> {
    list.iter().map(ResourceExt::name_any).collect()
}

#[tokio::test]
async fn fake_lists_by_pages_on_one_snapshot() {
    let (fake, client, _dir) = setup();
    for name in ["alpha", "bravo", "charlie"] {
        fake.apply(NAMESPACES, namespace(name));
    }
    let api: Api<Namespace> = Api::all(client);
    let first = api.list(&ListParams::default().limit(2)).await.unwrap();
    assert_eq!(names(&first.items), ["alpha", "bravo"]);
    let token = first.metadata.continue_.clone().unwrap();
    let version = first.metadata.resource_version.clone().unwrap();

    // A change between two pages is not in the snapshot.
    fake.apply(NAMESPACES, namespace("delta"));
    let second = api
        .list(&ListParams::default().limit(2).continue_token(&token))
        .await
        .unwrap();
    assert_eq!(names(&second.items), ["charlie"]);
    assert_eq!(second.metadata.resource_version, Some(version));
    assert_eq!(second.metadata.continue_, None);

    // An expired continue token: 410, the list starts again.
    let first = api.list(&ListParams::default().limit(1)).await.unwrap();
    fake.expire_continues();
    let token = first.metadata.continue_.unwrap();
    let error = api
        .list(&ListParams::default().limit(1).continue_token(&token))
        .await
        .unwrap_err();
    assert!(
        matches!(&error, kube::Error::Api(s) if s.code == 410),
        "{error:?}"
    );
    let all = api.list(&ListParams::default()).await.unwrap();
    assert_eq!(names(&all.items), ["alpha", "bravo", "charlie", "delta"]);
}

#[tokio::test]
async fn fake_watch_replays_then_streams_with_bookmarks_and_cuts() {
    let (fake, client, _dir) = setup();
    fake.apply(NAMESPACES, namespace("alpha"));
    let api: Api<Namespace> = Api::all(client);
    let snapshot = api.list(&ListParams::default()).await.unwrap();
    let from = snapshot.metadata.resource_version.unwrap();

    // A change after the list, before the watch: replayed from the history. Bookmarks
    // are asked for by default.
    fake.apply(NAMESPACES, namespace("bravo"));
    let mut events = api
        .watch(&WatchParams::default(), &from)
        .await
        .unwrap()
        .boxed();
    let Some(Ok(WatchEvent::Added(bravo))) = events.next().await else {
        panic!("bravo replayed");
    };
    assert_eq!(bravo.name_any(), "bravo");
    let watch = fake.requests().pop().unwrap();
    assert_eq!(watch.query.get("watch").map(String::as_str), Some("true"));
    assert_eq!(
        watch.query.get("allowWatchBookmarks").map(String::as_str),
        Some("true")
    );
    assert_eq!(watch.query.get("resourceVersion"), Some(&from));

    // Live: a change, a deletion, a bookmark.
    fake.apply(NAMESPACES, namespace("alpha"));
    fake.delete(NAMESPACES, "", "bravo");
    fake.bookmark(NAMESPACES);
    let Some(Ok(WatchEvent::Modified(alpha))) = events.next().await else {
        panic!("alpha modified");
    };
    assert_eq!(alpha.name_any(), "alpha");
    assert!(matches!(
        events.next().await,
        Some(Ok(WatchEvent::Deleted(_)))
    ));
    let Some(Ok(WatchEvent::Bookmark(bookmark))) = events.next().await else {
        panic!("a bookmark");
    };
    let bookmarked = bookmark.metadata.resource_version;
    assert_eq!(bookmarked, fake.version().to_string());

    // A cut: the stream ends; a watch from the last version misses nothing.
    fake.cut(NAMESPACES);
    assert!(events.next().await.is_none());
    fake.apply(NAMESPACES, namespace("charlie"));
    let mut events = api
        .watch(&WatchParams::default(), &bookmarked)
        .await
        .unwrap()
        .boxed();
    let Some(Ok(WatchEvent::Added(charlie))) = events.next().await else {
        panic!("charlie after the cut");
    };
    assert_eq!(charlie.name_any(), "charlie");
    assert_eq!(fake.watchers(NAMESPACES), 1);
}

#[tokio::test]
async fn fake_watch_gone_as_event_or_response() {
    let (fake, client, _dir) = setup();
    let old = fake.apply(NAMESPACES, namespace("alpha")).to_string();
    let api: Api<Namespace> = Api::all(client);

    // An open watch ended by an ERROR 410.
    let mut events = api
        .watch(&WatchParams::default(), &old)
        .await
        .unwrap()
        .boxed();
    fake.apply(NAMESPACES, namespace("bravo"));
    assert!(matches!(
        events.next().await,
        Some(Ok(WatchEvent::Added(_)))
    ));
    fake.expire_watches(NAMESPACES);
    let Some(Ok(WatchEvent::Error(error))) = events.next().await else {
        panic!("an ERROR event");
    };
    assert_eq!(error.code, 410);
    assert!(events.next().await.is_none());

    // A watch from a version gone from the history: an ERROR 410 event…
    let events: Vec<_> = api
        .watch(&WatchParams::default(), &old)
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();
    assert!(matches!(events.as_slice(), [WatchEvent::Error(e)] if e.code == 410));

    // … or a 410 response.
    fake.gone_as(GoneAs::Response);
    let Err(kube::Error::Api(status)) = api.watch(&WatchParams::default(), &old).await else {
        panic!("a 410 response");
    };
    assert_eq!(status.code, 410);

    // Relisted, the watch goes on from the version of the new list.
    let list = api.list(&ListParams::default()).await.unwrap();
    let from = list.metadata.resource_version.unwrap();
    let mut events = api
        .watch(&WatchParams::default(), &from)
        .await
        .unwrap()
        .boxed();
    fake.apply(NAMESPACES, namespace("charlie"));
    assert!(matches!(
        events.next().await,
        Some(Ok(WatchEvent::Added(_)))
    ));
}

#[tokio::test]
async fn fake_reviews_the_rights_of_the_connector() {
    let (fake, client, _dir) = setup();
    fake.rules(vec![
        Rule::new(&[""], &["namespaces", "pods"], &["get", "list", "watch"]),
        Rule::new(&[""], &["secrets"], &["list"]),
    ]);
    let rules: Api<SelfSubjectRulesReview> = Api::all(client.clone());
    let review = SelfSubjectRulesReview {
        spec: SelfSubjectRulesReviewSpec {
            namespace: Some("default".into()),
        },
        ..SelfSubjectRulesReview::default()
    };
    let review = rules.create(&PostParams::default(), &review).await.unwrap();
    let status = review.status.unwrap();
    assert_eq!(status.resource_rules.len(), 2);
    assert_eq!(
        status.resource_rules[1].resources.as_deref(),
        Some(&["secrets".to_owned()][..])
    );

    let access: Api<SelfSubjectAccessReview> = Api::all(client);
    for (resource, verb, allowed) in [
        ("secrets", "list", true),
        ("pods", "watch", true),
        ("pods", "delete", false),
    ] {
        let review = SelfSubjectAccessReview {
            spec: SelfSubjectAccessReviewSpec {
                resource_attributes: Some(ResourceAttributes {
                    resource: Some(resource.into()),
                    verb: Some(verb.into()),
                    group: Some(String::new()),
                    ..ResourceAttributes::default()
                }),
                ..SelfSubjectAccessReviewSpec::default()
            },
            ..SelfSubjectAccessReview::default()
        };
        let review = access
            .create(&PostParams::default(), &review)
            .await
            .unwrap();
        assert_eq!(review.status.unwrap().allowed, allowed, "{verb} {resource}");
    }
    let posts = fake
        .requests()
        .into_iter()
        .filter(|r| r.method == http::Method::POST)
        .count();
    assert_eq!(posts, 4);
}
