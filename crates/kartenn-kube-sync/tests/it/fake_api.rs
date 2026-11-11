//! A fake API server (task 019-9): a `tower::Service` given to the client of the
//! connector in place of the TLS client, that replays what the connector must survive
//! (réf. § API) — lists by pages (`limit`, `continue`) on one snapshot, a continue token
//! that expires (`410`), a watch from a `resourceVersion` that replays the kept history
//! then the live events, bookmarks, a cut of the stream, `410 Gone` as a response or as
//! an `ERROR` event, and the access reviews of the connector on itself.
//!
//! Collections are named by their path (`/api/v1/namespaces`, `/api/v1/pods`…); every
//! request is recorded, with its `Authorization` header.

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use http::{Method, Request, Response, StatusCode};
use http_body::Frame;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::sync::mpsc;

/// How the server says that a `resourceVersion` is too old for a watch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GoneAs {
    /// A `410` response.
    Response,
    /// A `200` response whose first event is `ERROR` with a `410` status.
    Event,
}

/// A request the server received.
#[derive(Debug, Clone)]
pub(crate) struct Recorded {
    pub(crate) method: Method,
    pub(crate) path: String,
    pub(crate) query: BTreeMap<String, String>,
    pub(crate) authorization: Option<String>,
    pub(crate) body: Option<Value>,
}

/// A rule of the rights of the connector (`SelfSubjectRulesReview`).
#[derive(Debug, Clone)]
pub(crate) struct Rule {
    pub(crate) api_groups: Vec<String>,
    pub(crate) resources: Vec<String>,
    pub(crate) verbs: Vec<String>,
}

impl Rule {
    pub(crate) fn new(api_groups: &[&str], resources: &[&str], verbs: &[&str]) -> Self {
        let own = |items: &[&str]| items.iter().map(|&s| s.to_owned()).collect();
        Self {
            api_groups: own(api_groups),
            resources: own(resources),
            verbs: own(verbs),
        }
    }

    fn allows(&self, group: &str, resource: &str, verb: &str) -> bool {
        let any = |items: &[String], wanted: &str| items.iter().any(|i| i == "*" || i == wanted);
        any(&self.api_groups, group) && any(&self.resources, resource) && any(&self.verbs, verb)
    }
}

type Sender = mpsc::UnboundedSender<Bytes>;

#[derive(Debug)]
struct Collection {
    api_version: String,
    kind: String,
    /// By `namespace/name`.
    objects: BTreeMap<String, Value>,
    /// The events kept, in order, from `oldest` on.
    history: Vec<(u64, &'static str, Value)>,
    /// A watch from a version older than this one is gone.
    oldest: u64,
    watchers: Vec<Sender>,
}

#[derive(Debug)]
struct Page {
    path: String,
    version: u64,
    rest: Vec<Value>,
}

#[derive(Debug)]
struct State {
    version: u64,
    collections: BTreeMap<String, Collection>,
    pages: HashMap<String, Page>,
    next_page: u64,
    gone_as: GoneAs,
    rules: Vec<Rule>,
    requests: Vec<Recorded>,
}

/// The fake server; its clones share one state.
#[derive(Debug, Clone)]
pub(crate) struct FakeApi(Arc<Mutex<State>>);

fn key(object: &Value) -> String {
    let meta = &object["metadata"];
    format!(
        "{}/{}",
        meta["namespace"].as_str().unwrap_or_default(),
        meta["name"].as_str().expect("an object has a name")
    )
}

fn line(value: &Value) -> Bytes {
    let mut text = serde_json::to_vec(value).expect("JSON values serialise");
    text.push(b'\n');
    Bytes::from(text)
}

fn status(code: u16, reason: &str, message: &str) -> Value {
    json!({
        "kind": "Status", "apiVersion": "v1", "metadata": {}, "status": "Failure",
        "message": message, "reason": reason, "code": code,
    })
}

impl FakeApi {
    pub(crate) fn new() -> Self {
        Self(Arc::new(Mutex::new(State {
            version: 1,
            collections: BTreeMap::new(),
            pages: HashMap::new(),
            next_page: 0,
            gone_as: GoneAs::Event,
            rules: Vec::new(),
            requests: Vec::new(),
        })))
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().expect("no test panics with the state locked")
    }

    /// Declares a collection served at `path`, of objects of `kind`.
    pub(crate) fn collection(&self, path: &str, api_version: &str, kind: &str) -> &Self {
        let mut state = self.state();
        let oldest = state.version;
        state.collections.insert(
            path.to_owned(),
            Collection {
                api_version: api_version.to_owned(),
                kind: kind.to_owned(),
                objects: BTreeMap::new(),
                history: Vec::new(),
                oldest,
                watchers: Vec::new(),
            },
        );
        self
    }

    /// The current `resourceVersion`.
    pub(crate) fn version(&self) -> u64 {
        self.state().version
    }

    /// Creates or replaces `object` (`ADDED` or `MODIFIED`); returns its version.
    pub(crate) fn apply(&self, path: &str, mut object: Value) -> u64 {
        let mut state = self.state();
        state.version += 1;
        let version = state.version;
        let collection = state
            .collections
            .get_mut(path)
            .expect("a declared collection");
        object["apiVersion"] = json!(collection.api_version);
        object["kind"] = json!(collection.kind);
        object["metadata"]["resourceVersion"] = json!(version.to_string());
        let kind = if collection
            .objects
            .insert(key(&object), object.clone())
            .is_some()
        {
            "MODIFIED"
        } else {
            "ADDED"
        };
        collection.emit(version, kind, object);
        version
    }

    /// Deletes the object `namespace/name` (`DELETED`).
    pub(crate) fn delete(&self, path: &str, namespace: &str, name: &str) -> u64 {
        let mut state = self.state();
        state.version += 1;
        let version = state.version;
        let collection = state
            .collections
            .get_mut(path)
            .expect("a declared collection");
        let mut object = collection
            .objects
            .remove(&format!("{namespace}/{name}"))
            .expect("a known object");
        object["metadata"]["resourceVersion"] = json!(version.to_string());
        collection.emit(version, "DELETED", object);
        version
    }

    /// Sends a `BOOKMARK` at the current version to the open watches of `path`.
    pub(crate) fn bookmark(&self, path: &str) {
        let mut state = self.state();
        let version = state.version;
        let collection = state
            .collections
            .get_mut(path)
            .expect("a declared collection");
        let event = json!({"type": "BOOKMARK", "object": {
            "apiVersion": collection.api_version, "kind": collection.kind,
            "metadata": {"resourceVersion": version.to_string()},
        }});
        collection.send(&line(&event));
    }

    /// Cuts the open watches of `path`: their streams end.
    pub(crate) fn cut(&self, path: &str) {
        let mut state = self.state();
        state
            .collections
            .get_mut(path)
            .expect("a declared collection")
            .watchers
            .clear();
    }

    /// Forgets the history of `path`: a watch from an older version is gone (`410`).
    pub(crate) fn compact(&self, path: &str) {
        let mut state = self.state();
        let version = state.version;
        let collection = state
            .collections
            .get_mut(path)
            .expect("a declared collection");
        collection.history.clear();
        collection.oldest = version;
    }

    /// Ends the open watches of `path` with an `ERROR` event `410`, as a server whose
    /// history moved past them; the history is forgotten too.
    pub(crate) fn expire_watches(&self, path: &str) {
        self.compact(path);
        let mut state = self.state();
        let collection = state
            .collections
            .get_mut(path)
            .expect("a declared collection");
        let event =
            json!({"type": "ERROR", "object": status(410, "Expired", "too old resource version")});
        collection.send(&line(&event));
        collection.watchers.clear();
    }

    /// Expires every continue token given so far.
    pub(crate) fn expire_continues(&self) {
        self.state().pages.clear();
    }

    /// How a watch from a too old version is refused.
    pub(crate) fn gone_as(&self, gone_as: GoneAs) {
        self.state().gone_as = gone_as;
    }

    /// The rights of the connector, for the access reviews.
    pub(crate) fn rules(&self, rules: Vec<Rule>) {
        self.state().rules = rules;
    }

    /// The requests received so far.
    pub(crate) fn requests(&self) -> Vec<Recorded> {
        self.state().requests.clone()
    }

    /// The number of watches open on `path`.
    pub(crate) fn watchers(&self, path: &str) -> usize {
        let mut state = self.state();
        let collection = state
            .collections
            .get_mut(path)
            .expect("a declared collection");
        collection.watchers.retain(|w| !w.is_closed());
        collection.watchers.len()
    }

    fn respond(&self, recorded: &Recorded) -> Response<ChannelBody> {
        let mut state = self.state();
        state.requests.push(recorded.clone());
        let path = recorded.path.as_str();
        if recorded.method == Method::POST {
            return state.review(path, recorded.body.as_ref());
        }
        if recorded.method != Method::GET || !state.collections.contains_key(path) {
            return ChannelBody::once(StatusCode::NOT_FOUND, &status(404, "NotFound", path));
        }
        if recorded
            .query
            .get("watch")
            .is_some_and(|w| w == "true" || w == "1")
        {
            state.watch(path, &recorded.query)
        } else {
            state.list(path, &recorded.query)
        }
    }
}

impl Collection {
    fn emit(&mut self, version: u64, kind: &'static str, object: Value) {
        let event = json!({"type": kind, "object": object});
        self.history.push((version, kind, object));
        self.send(&line(&event));
    }

    fn send(&mut self, bytes: &Bytes) {
        self.watchers.retain(|w| w.send(bytes.clone()).is_ok());
    }
}

impl State {
    fn list(&mut self, path: &str, query: &BTreeMap<String, String>) -> Response<ChannelBody> {
        let limit = query
            .get("limit")
            .and_then(|l| l.parse::<usize>().ok())
            .filter(|&l| l > 0);
        let mut page = match query.get("continue").filter(|c| !c.is_empty()) {
            Some(token) => match self.pages.remove(token) {
                Some(page) if page.path == path => page,
                _ => {
                    return ChannelBody::once(
                        StatusCode::GONE,
                        &status(410, "Expired", "the continue token has expired"),
                    );
                }
            },
            None => Page {
                path: path.to_owned(),
                version: self.version,
                rest: self.collections[path].objects.values().cloned().collect(),
            },
        };
        let take = limit.unwrap_or(page.rest.len()).min(page.rest.len());
        let items: Vec<Value> = page.rest.drain(..take).collect();
        let version = page.version;
        let mut metadata = json!({"resourceVersion": version.to_string()});
        if !page.rest.is_empty() {
            self.next_page += 1;
            let token = format!("page-{}", self.next_page);
            metadata["continue"] = json!(token);
            metadata["remainingItemCount"] = json!(page.rest.len());
            self.pages.insert(token, page);
        }
        let collection = &self.collections[path];
        let body = json!({
            "apiVersion": collection.api_version,
            "kind": format!("{}List", collection.kind),
            "metadata": metadata,
            "items": items,
        });
        ChannelBody::once(StatusCode::OK, &body)
    }

    fn watch(&mut self, path: &str, query: &BTreeMap<String, String>) -> Response<ChannelBody> {
        let gone_as = self.gone_as;
        let collection = self.collections.get_mut(path).expect("a known collection");
        let (sender, body) = ChannelBody::channel();
        let from = query
            .get("resourceVersion")
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&v| v > 0);
        match from {
            // From the start: the current objects as `ADDED`.
            None => {
                for object in collection.objects.values() {
                    let _ = sender.send(line(&json!({"type": "ADDED", "object": object})));
                }
            }
            Some(from) if from < collection.oldest => {
                let gone = status(410, "Expired", "too old resource version");
                return match gone_as {
                    GoneAs::Response => ChannelBody::once(StatusCode::GONE, &gone),
                    GoneAs::Event => {
                        let _ = sender.send(line(&json!({"type": "ERROR", "object": gone})));
                        drop(sender);
                        body.response(StatusCode::OK)
                    }
                };
            }
            Some(from) => {
                for (_, kind, object) in collection.history.iter().filter(|(v, ..)| *v > from) {
                    let _ = sender.send(line(&json!({"type": kind, "object": object})));
                }
            }
        }
        collection.watchers.push(sender);
        body.response(StatusCode::OK)
    }

    /// `SelfSubjectRulesReview` and `SelfSubjectAccessReview` of the connector on itself.
    fn review(&self, path: &str, body: Option<&Value>) -> Response<ChannelBody> {
        let base = "/apis/authorization.k8s.io/v1/";
        match path.strip_prefix(base) {
            Some("selfsubjectrulesreviews") => {
                let rules: Vec<Value> = self
                    .rules
                    .iter()
                    .map(|r| json!({"apiGroups": r.api_groups, "resources": r.resources, "verbs": r.verbs}))
                    .collect();
                let mut review = body.cloned().unwrap_or_else(|| json!({}));
                review["status"] = json!({
                    "resourceRules": rules, "nonResourceRules": [], "incomplete": false,
                });
                ChannelBody::once(StatusCode::CREATED, &review)
            }
            Some("selfsubjectaccessreviews") => {
                let mut review = body.cloned().unwrap_or_else(|| json!({}));
                let attributes = &review["spec"]["resourceAttributes"];
                let field = |name: &str| attributes[name].as_str().unwrap_or_default().to_owned();
                let (group, resource, verb) = (field("group"), field("resource"), field("verb"));
                let allowed = self
                    .rules
                    .iter()
                    .any(|r| r.allows(&group, &resource, &verb));
                review["status"] = json!({"allowed": allowed});
                ChannelBody::once(StatusCode::CREATED, &review)
            }
            _ => ChannelBody::once(StatusCode::NOT_FOUND, &status(404, "NotFound", path)),
        }
    }
}

/// A body fed by a channel: one chunk for a list, a line per event for a watch, that
/// ends when the sender is dropped.
#[derive(Debug)]
pub(crate) struct ChannelBody(mpsc::UnboundedReceiver<Bytes>);

impl ChannelBody {
    fn channel() -> (Sender, Self) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (sender, Self(receiver))
    }

    fn once(code: StatusCode, value: &Value) -> Response<Self> {
        let (sender, body) = Self::channel();
        let _ = sender.send(line(value));
        body.response(code)
    }

    fn response(self, code: StatusCode) -> Response<Self> {
        Response::builder()
            .status(code)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(self)
            .expect("a valid response")
    }
}

impl http_body::Body for ChannelBody {
    type Data = Bytes;
    type Error = Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        self.0
            .poll_recv(cx)
            .map(|chunk| chunk.map(|c| Ok(Frame::data(c))))
    }
}

impl tower::Service<Request<kube::client::Body>> for FakeApi {
    type Response = Response<ChannelBody>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Infallible>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request<kube::client::Body>) -> Self::Future {
        let fake = self.clone();
        Box::pin(async move {
            let (parts, body) = request.into_parts();
            let bytes = body
                .collect()
                .await
                .map(http_body_util::Collected::to_bytes)
                .unwrap_or_default();
            let query = parts
                .uri
                .query()
                .unwrap_or_default()
                .split('&')
                .filter(|pair| !pair.is_empty())
                .map(|pair| {
                    let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
                    (name.to_owned(), value.to_owned())
                })
                .collect();
            let recorded = Recorded {
                method: parts.method,
                path: parts.uri.path().to_owned(),
                query,
                authorization: parts
                    .headers
                    .get(http::header::AUTHORIZATION)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned),
                body: serde_json::from_slice(&bytes).ok(),
            };
            Ok(fake.respond(&recorded))
        })
    }
}
