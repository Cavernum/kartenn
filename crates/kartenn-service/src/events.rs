//! Server-sent events (spec 022, ADR 0034): the pure part of the stream `GET /v1/events`.
//!
//! The shared reader of an instance reads the outbox (ADR 0018) and [`translate`]s each
//! event into [`Notice`]s: keys and revisions only, never values (REQ-022-4). Each
//! connection then [`render`]s a notice after its [`Filters`] and the rights of its
//! subscriber, and writes it as a [`Frame`] of the WHATWG format (HTML § 9.2.6): one
//! `data:` line, never a line feed nor a carriage return inside a field.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use uuid::Uuid;

/// Most qualified types in the filter `types` (REQ-022-2).
pub const MAX_TYPES: usize = 32;
/// Most domains in the filter `domains`.
pub const MAX_DOMAINS: usize = 32;
/// Most objects in the filter `objects`.
pub const MAX_OBJECTS: usize = 100;
/// Most objects an event names before it becomes a broad invalidation (REQ-022-5).
pub const DEFAULT_BOUND: usize = 1_000;
/// The source of the events kartenn writes to its outbox.
const CHANGESET_PREFIX: &str = "org.kartenn.changeset.";
/// The event of a change of the labels (spec 018, REQ-018-18).
const LABELS_CHANGED: &str = "org.kartenn.labels.changed";

/// The kind of an event, its `event:` field (REQ-022-3, amended by REQ-018-18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// The stream is open.
    Ready,
    /// Objects written by a changeset or a background task.
    Changes,
    /// The state of a pending changeset.
    Changeset,
    /// The revision of the labels changed (spec 018).
    Labels,
    /// Invalidate everything.
    Reset,
}

impl Kind {
    /// The kinds a subscriber may select with `kinds`; `ready` and `reset` always come.
    pub const SELECTABLE: [Self; 3] = [Self::Changes, Self::Changeset, Self::Labels];

    /// Its name on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Changes => "changes",
            Self::Changeset => "changeset",
            Self::Labels => "labels",
            Self::Reset => "reset",
        }
    }

    /// The selectable kind of this name.
    #[must_use]
    pub fn selectable(name: &str) -> Option<Self> {
        Self::SELECTABLE.into_iter().find(|k| k.as_str() == name)
    }
}

/// Why a stream starts over (`reset`, REQ-022-8, 18, 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResetReason {
    /// The cursor is older than the ring.
    TooOld,
    /// The cursor does not open: altered, another identity, another key.
    InvalidCursor,
    /// The buffer of the connection overflowed.
    Overflow,
    /// The bindings or roles of the subscriber changed.
    RightsChanged,
}

impl ResetReason {
    /// Its name on the wire and in the metrics.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TooOld => "too_old",
            Self::InvalidCursor => "invalid_cursor",
            Self::Overflow => "overflow",
            Self::RightsChanged => "rights_changed",
        }
    }
}

/// An object named by an event: its qualified type (`core.prefix`, `vlan.vlan`) and
/// identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Object {
    /// Qualified type.
    pub kind: String,
    /// Identifier.
    pub id: Uuid,
}

/// Objects written, before the rights of any subscriber.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Changes {
    /// The changeset, the revision of every object named; none for a background task.
    pub changeset: Option<Uuid>,
    /// The CloudEvents identifier of an event of a module.
    pub event: Option<String>,
    /// `<module>:<task>` for an event of a background task (REQ-016-15).
    pub task: Option<String>,
    /// The objects written.
    pub objects: Vec<Object>,
    /// Types whose values changed beyond the objects named (inherited values): a broad
    /// invalidation (REQ-022-5).
    pub derived: Vec<String>,
    /// Addressing domains written.
    pub domains: Vec<Uuid>,
}

/// What a notice tells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Body {
    /// See [`Changes`].
    Changes(Changes),
    /// A pending changeset changed state (REQ-022-17).
    Changeset {
        /// The changeset.
        changeset: Uuid,
        /// `pending`, `applied`, `rejected`, `withdrawn`, `expired`, `failed_revalidation`.
        state: String,
        /// Its author, who always receives it.
        author: Option<Uuid>,
    },
    /// The labels changed (spec 018): every connection receives it.
    Labels {
        /// The new revision.
        revision: String,
    },
}

/// An event of the outbox, translated: what the shared reader keeps in its ring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// Its number in the outbox.
    pub seq: i64,
    /// When the changeset was applied (Unix microseconds), for the lag (REQ-022-25).
    pub time: Option<i64>,
    /// What it tells.
    pub body: Body,
}

/// A row of the outbox, as the reader reads it.
#[derive(Debug, Clone, Copy)]
pub struct OutboxRow<'a> {
    /// Its number.
    pub seq: i64,
    /// Its changeset, none for a background task.
    pub changeset: Option<Uuid>,
    /// Domains of the changeset.
    pub domains: &'a [Uuid],
    /// Ordering keys: `<module>:<task>` for a background task.
    pub order_keys: &'a [String],
    /// CloudEvents envelope.
    pub payload: &'a Value,
}

/// The qualified type of a kind of `carrier_object`: the core's are bare (`prefix`).
#[must_use]
pub fn qualified(kind: &str) -> String {
    if kind.contains('.') {
        kind.to_owned()
    } else {
        format!("core.{kind}")
    }
}

/// The types whose resolved values follow those of an object of `kind` by inheritance
/// (spec 003): the subtree of a domain or a prefix, the addresses of a range, the tree of
/// a module.
fn inheriting(kind: &str) -> &'static [&'static str] {
    match kind {
        "core.domain" | "core.prefix" => &["core.prefix", "core.range", "core.address"],
        "core.range" => &["core.address"],
        _ => &[],
    }
}

fn uuid_of(value: &Value) -> Option<Uuid> {
    value.as_str().and_then(|s| Uuid::parse_str(s).ok())
}

/// Microseconds since the Unix epoch of an RFC 3339 instant, as PostgreSQL writes it in
/// JSON.
fn micros(text: &str) -> Option<i64> {
    let at =
        time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339).ok()?;
    i64::try_from(at.unix_timestamp_nanos() / 1_000).ok()
}

/// The notices of an event of the outbox (design « Format », table of translation), its
/// objects typed by `kind_of` (their kind in `carrier_object`); an object without a type
/// (a policy, a binding, a declaration) is not pushed, and an event that names nothing
/// pushable gives no notice.
pub fn translate(row: OutboxRow<'_>, kind_of: &dyn Fn(Uuid) -> Option<String>) -> Vec<Notice> {
    let payload = row.payload;
    let Some(kind) = payload.get("type").and_then(Value::as_str) else {
        return Vec::new();
    };
    let data = payload.get("data").unwrap_or(&Value::Null);
    let time = payload.get("time").and_then(Value::as_str).and_then(micros);
    let notice = |body| Notice {
        seq: row.seq,
        time,
        body,
    };
    if kind == LABELS_CHANGED {
        return data
            .get("revision")
            .map(|r| match r {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .map(|revision| notice(Body::Labels { revision }))
            .into_iter()
            .collect();
    }
    if let Some(state) = kind.strip_prefix(CHANGESET_PREFIX) {
        let changeset = row
            .changeset
            .or_else(|| data.get("changeset").and_then(uuid_of));
        let Some(changeset) = changeset else {
            return Vec::new();
        };
        let author = data.get("author").and_then(uuid_of);
        let mut out = Vec::new();
        if state == "applied" {
            if let Some(changes) = applied(row, data, kind_of) {
                out.push(notice(Body::Changes(changes)));
            }
            // A changeset that waited for a validator: its author and validators learn
            // that it is applied.
            if data.get("validator").is_some_and(|v| !v.is_null()) {
                out.push(notice(Body::Changeset {
                    changeset,
                    state: state.to_owned(),
                    author,
                }));
            }
        } else {
            out.push(notice(Body::Changeset {
                changeset,
                state: state.to_owned(),
                author,
            }));
        }
        return out;
    }
    if kind.starts_with("org.kartenn.") {
        // An event of a module: pushed when its subject is an object of a known type.
        let Some(subject) = payload
            .get("subject")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
        else {
            return Vec::new();
        };
        let Some(kind) = kind_of(subject) else {
            return Vec::new();
        };
        let event = payload.get("id").and_then(Value::as_str).map(str::to_owned);
        let task = if row.changeset.is_none() {
            row.order_keys.first().cloned()
        } else {
            None
        };
        // Its identity is its own CloudEvents identifier, never its changeset, whose
        // `applied` event has that identity already (REQ-022-27): no revision, the client
        // reads again without condition.
        return vec![notice(Body::Changes(Changes {
            changeset: None,
            event,
            task,
            objects: vec![Object {
                kind: qualified(&kind),
                id: subject,
            }],
            derived: Vec::new(),
            domains: row.domains.to_vec(),
        }))];
    }
    Vec::new()
}

/// The changes of `org.kartenn.changeset.applied`.
fn applied(
    row: OutboxRow<'_>,
    data: &Value,
    kind_of: &dyn Fn(Uuid) -> Option<String>,
) -> Option<Changes> {
    let mut objects: Vec<Object> = data
        .get("objects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(uuid_of)
        .filter_map(|id| {
            kind_of(id).map(|kind| Object {
                kind: qualified(&kind),
                id,
            })
        })
        .collect();
    objects.sort();
    objects.dedup();
    // Inherited values: the differential counts more objects than those written.
    let widest = data
        .get("diff")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|d| d.get("count").and_then(Value::as_u64))
        .max()
        .unwrap_or(0);
    let mut derived: BTreeSet<String> = BTreeSet::new();
    if widest > objects.len() as u64 {
        for object in &objects {
            derived.extend(inheriting(&object.kind).iter().map(|t| (*t).to_owned()));
            if !object.kind.starts_with("core.") {
                derived.insert(object.kind.clone());
            }
        }
    }
    if objects.is_empty() {
        return None;
    }
    let changeset = row
        .changeset
        .or_else(|| data.get("changeset").and_then(uuid_of));
    Some(Changes {
        changeset,
        event: None,
        task: None,
        objects,
        derived: derived.into_iter().collect(),
        domains: row.domains.to_vec(),
    })
}

/// Why filters are refused (400 `events.filter.invalid`).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FilterError {
    /// A parameter that is not a filter.
    #[error("unknown parameter {0}")]
    Unknown(String),
    /// A value that is not of its filter.
    #[error("invalid value {1:?} for {0}")]
    Value(&'static str, String),
    /// Too many values.
    #[error("{0}: at most {1} values")]
    TooMany(&'static str, usize),
}

/// The filters of a stream, combined by AND (REQ-022-2): none means everything.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Filters {
    /// Qualified types.
    pub types: Option<BTreeSet<String>>,
    /// Addressing domains.
    pub domains: Option<BTreeSet<Uuid>>,
    /// Objects watched.
    pub objects: Option<BTreeSet<Uuid>>,
    /// Kinds selected.
    pub kinds: Option<BTreeSet<Kind>>,
}

fn list(value: &str) -> impl Iterator<Item = &str> {
    value.split(',').map(str::trim).filter(|v| !v.is_empty())
}

impl Filters {
    /// The filters of the query `pairs`; `last_event_id` is left to the caller; a type
    /// is known when `known` says so (the core's and those of the modules of the build).
    ///
    /// # Errors
    ///
    /// [`FilterError`] for an unknown parameter, a value of the wrong form, a type
    /// unknown, or too many values.
    pub fn parse<'a>(
        pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
        known: &dyn Fn(&str) -> bool,
    ) -> Result<Self, FilterError> {
        let mut out = Self::default();
        for (name, value) in pairs {
            match name {
                "types" => {
                    let set = out.types.get_or_insert_with(BTreeSet::new);
                    for t in list(value) {
                        if !known(t) {
                            return Err(FilterError::Value("types", t.to_owned()));
                        }
                        set.insert(t.to_owned());
                    }
                    if set.len() > MAX_TYPES {
                        return Err(FilterError::TooMany("types", MAX_TYPES));
                    }
                }
                "domains" | "objects" => {
                    let (field, max) = if name == "domains" {
                        (&mut out.domains, MAX_DOMAINS)
                    } else {
                        (&mut out.objects, MAX_OBJECTS)
                    };
                    let set = field.get_or_insert_with(BTreeSet::new);
                    let label = if name == "domains" {
                        "domains"
                    } else {
                        "objects"
                    };
                    for v in list(value) {
                        let id = Uuid::parse_str(v)
                            .map_err(|_| FilterError::Value(label, v.to_owned()))?;
                        set.insert(id);
                    }
                    if set.len() > max {
                        return Err(FilterError::TooMany(label, max));
                    }
                }
                "kinds" => {
                    let set = out.kinds.get_or_insert_with(BTreeSet::new);
                    for k in list(value) {
                        set.insert(
                            Kind::selectable(k)
                                .ok_or_else(|| FilterError::Value("kinds", k.to_owned()))?,
                        );
                    }
                }
                "last_event_id" => {}
                other => return Err(FilterError::Unknown(other.to_owned())),
            }
        }
        Ok(out)
    }

    /// Whether `kind` is selected.
    #[must_use]
    pub fn selects(&self, kind: Kind) -> bool {
        self.kinds.as_ref().is_none_or(|k| k.contains(&kind))
    }

    /// The filters as the audit records them (`criteria`).
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut out = serde_json::Map::new();
        if let Some(t) = &self.types {
            out.insert("types".into(), json!(t));
        }
        if let Some(d) = &self.domains {
            out.insert("domains".into(), json!(d));
        }
        if let Some(o) = &self.objects {
            out.insert("objects".into(), json!(o));
        }
        if let Some(k) = &self.kinds {
            out.insert(
                "kinds".into(),
                json!(k.iter().map(|k| k.as_str()).collect::<Vec<_>>()),
            );
        }
        Value::Object(out)
    }
}

/// An event for one connection: its kind, its data on one line, and the objects it names
/// (for the audit, REQ-022-24).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// Its kind.
    pub kind: Kind,
    /// Its data, JSON on one line.
    pub data: String,
    /// The objects it names (none when truncated).
    pub objects: Vec<Uuid>,
}

/// What a subscriber may see of a notice, decided by the dispatcher (REQ-022-16, 17).
pub trait Sight {
    /// Whether the object or domain `id` is readable now.
    fn sees(&self, id: Uuid) -> bool;
    /// Whether the changeset `id` is theirs or theirs to validate.
    fn follows(&self, changeset: Uuid) -> bool;
}

/// `notice` as one connection receives it, after its `filters` and what it may see;
/// `None` leaves no trace (REQ-022-16). Beyond `bound` objects visible, or with derived
/// effects, the event names types (`truncated`, REQ-022-5).
#[must_use]
pub fn render(
    notice: &Notice,
    filters: &Filters,
    sight: &dyn Sight,
    bound: usize,
) -> Option<Rendered> {
    match &notice.body {
        Body::Labels { revision } => filters.selects(Kind::Labels).then(|| Rendered {
            kind: Kind::Labels,
            data: json!({ "revision": revision }).to_string(),
            objects: Vec::new(),
        }),
        Body::Changeset {
            changeset, state, ..
        } => (filters.selects(Kind::Changeset) && sight.follows(*changeset)).then(|| Rendered {
            kind: Kind::Changeset,
            data: json!({ "changeset": changeset, "state": state }).to_string(),
            objects: Vec::new(),
        }),
        Body::Changes(changes) => {
            if !filters.selects(Kind::Changes) {
                return None;
            }
            if let Some(wanted) = &filters.domains
                && !changes.domains.iter().any(|d| wanted.contains(d))
            {
                return None;
            }
            let objects: Vec<&Object> = changes
                .objects
                .iter()
                .filter(|o| filters.types.as_ref().is_none_or(|t| t.contains(&o.kind)))
                .filter(|o| filters.objects.as_ref().is_none_or(|w| w.contains(&o.id)))
                .filter(|o| sight.sees(o.id))
                .collect();
            if objects.is_empty() {
                return None;
            }
            let domains: Vec<Uuid> = changes
                .domains
                .iter()
                .copied()
                .filter(|d| sight.sees(*d))
                .collect();
            let derived: BTreeSet<&str> = changes
                .derived
                .iter()
                .filter(|t| filters.types.as_ref().is_none_or(|w| w.contains(*t)))
                .map(String::as_str)
                .collect();
            let mut data = serde_json::Map::new();
            if let Some(c) = changes.changeset {
                data.insert("changeset".into(), json!(c));
            }
            if let Some(t) = &changes.task {
                data.insert("task".into(), json!(t));
            }
            if let Some(e) = &changes.event {
                data.insert("event".into(), json!(e));
            }
            let mut named = Vec::new();
            if objects.len() > bound {
                let mut types: BTreeSet<&str> = objects.iter().map(|o| o.kind.as_str()).collect();
                types.extend(derived);
                data.insert("truncated".into(), json!(true));
                data.insert("types".into(), json!(types));
            } else {
                data.insert(
                    "objects".into(),
                    Value::Array(
                        objects
                            .iter()
                            .map(|o| json!({ "type": o.kind, "id": o.id }))
                            .collect(),
                    ),
                );
                named = objects.iter().map(|o| o.id).collect();
                if !derived.is_empty() {
                    data.insert("truncated".into(), json!(true));
                    data.insert("types".into(), json!(derived));
                }
            }
            data.insert("domains".into(), json!(domains));
            Some(Rendered {
                kind: Kind::Changes,
                data: Value::Object(data).to_string(),
                objects: named,
            })
        }
    }
}

/// The data of `ready`.
#[must_use]
pub fn ready_data(instance: Uuid, heartbeat_s: u64, ring_s: u64) -> String {
    json!({ "instance": instance, "heartbeat": heartbeat_s, "ring_seconds": ring_s }).to_string()
}

/// The data of `reset`.
#[must_use]
pub fn reset_data(reason: ResetReason) -> String {
    json!({ "reason": reason.as_str() }).to_string()
}

/// One block of the stream (WHATWG HTML § 9.2.6): fields then a blank line. A block with
/// an `id` alone is a resumption point: the client moves its last event identifier
/// without receiving an event (REQ-022-9).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Frame<'a> {
    /// `retry:` in milliseconds.
    pub retry: Option<u64>,
    /// `event:`.
    pub event: Option<&'a str>,
    /// `id:`, without NUL, LF nor CR.
    pub id: Option<&'a str>,
    /// `data:`, on one line.
    pub data: Option<&'a str>,
}

fn clean(field: &str) -> bool {
    !field.contains(['\n', '\r', '\0'])
}

impl Frame<'_> {
    /// The block, as written on the wire.
    ///
    /// # Panics
    ///
    /// A field holding a line feed, a carriage return or NUL: a bug of the caller, which
    /// only writes compact JSON and cursors in base64url.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::new();
        if let Some(retry) = self.retry {
            out.push_str("retry: ");
            out.push_str(&retry.to_string());
            out.push('\n');
        }
        for (name, value) in [("event", self.event), ("id", self.id), ("data", self.data)] {
            if let Some(value) = value {
                assert!(clean(value), "an SSE field holds a line break or NUL");
                out.push_str(name);
                out.push_str(": ");
                out.push_str(value);
                out.push('\n');
            }
        }
        out.push('\n');
        out
    }
}

/// A comment line (keep-alive, REQ-022-10, WHATWG HTML § 9.2.7).
///
/// # Panics
///
/// `text` holding a line break or NUL.
#[must_use]
pub fn comment(text: &str) -> String {
    assert!(clean(text), "an SSE comment holds a line break or NUL");
    format!(": {text}\n\n")
}

/// An event as a WHATWG client dispatches it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Parsed {
    /// Event type (`message` when absent).
    pub event: String,
    /// Data, its lines joined by LF.
    pub data: String,
    /// The last event identifier of the source when it was dispatched.
    pub last_id: String,
}

/// A parser of `text/event-stream` after WHATWG HTML § 9.2.6, for tests and clients: it
/// keeps the last event identifier and the reconnection time as `EventSource` does.
#[derive(Debug, Clone, Default)]
pub struct Parser {
    buffer: String,
    event: String,
    data: String,
    /// The last event identifier.
    pub last_id: String,
    /// The reconnection time, in milliseconds.
    pub retry: Option<u64>,
    /// Comments seen.
    pub comments: usize,
}

impl Parser {
    /// Feeds `chunk` and returns the events dispatched.
    pub fn feed(&mut self, chunk: &str) -> Vec<Parsed> {
        self.buffer.push_str(chunk);
        let mut out = Vec::new();
        while let Some(end) = self.buffer.find(['\n', '\r']) {
            let line: String = self.buffer[..end].to_owned();
            let mut skip = end + 1;
            if self.buffer[end..].starts_with("\r\n") {
                skip += 1;
            }
            self.buffer.drain(..skip);
            self.line(&line, &mut out);
        }
        out
    }

    fn line(&mut self, line: &str, out: &mut Vec<Parsed>) {
        if line.is_empty() {
            // Dispatch: the last event identifier moves first, even without data.
            if self.data.is_empty() {
                self.event.clear();
                return;
            }
            if self.data.ends_with('\n') {
                self.data.pop();
            }
            out.push(Parsed {
                event: if self.event.is_empty() {
                    "message".into()
                } else {
                    std::mem::take(&mut self.event)
                },
                data: std::mem::take(&mut self.data),
                last_id: self.last_id.clone(),
            });
            self.event.clear();
            return;
        }
        if line.starts_with(':') {
            self.comments += 1;
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => value.clone_into(&mut self.event),
            "data" => {
                self.data.push_str(value);
                self.data.push('\n');
            }
            "id" if !value.contains('\0') => value.clone_into(&mut self.last_id),
            "retry" if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
                self.retry = value.parse().ok();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn kinds(id: Uuid) -> Option<String> {
        match id.as_u128() {
            1..=99 => Some("prefix".into()),
            100..=199 => Some("address".into()),
            200..=299 => Some("vlan.vlan".into()),
            300..=399 => Some("domain".into()),
            _ => None,
        }
    }

    struct All;
    impl Sight for All {
        fn sees(&self, _: Uuid) -> bool {
            true
        }
        fn follows(&self, _: Uuid) -> bool {
            true
        }
    }

    struct Only(Vec<Uuid>);
    impl Sight for Only {
        fn sees(&self, id: Uuid) -> bool {
            self.0.contains(&id)
        }
        fn follows(&self, id: Uuid) -> bool {
            self.0.contains(&id)
        }
    }

    fn row<'a>(payload: &'a Value, domains: &'a [Uuid], keys: &'a [String]) -> OutboxRow<'a> {
        OutboxRow {
            seq: 7,
            changeset: payload["data"]["changeset"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok()),
            domains,
            order_keys: keys,
            payload,
        }
    }

    #[allow(clippy::needless_pass_by_value)] // `json!` literals
    fn applied_payload(objects: &[Uuid], diff: Value) -> Value {
        json!({
            "specversion": "1.0", "id": id(1000).to_string(), "source": "urn:kartenn:instance",
            "type": "org.kartenn.changeset.applied", "time": "2026-10-23T12:00:00.25+00:00",
            "data": {"changeset": id(1000), "domains": [id(300)], "objects": objects,
                     "author": id(9), "diff": diff},
        })
    }

    #[test]
    fn an_applied_changeset_names_its_typed_objects_and_its_domains() {
        let payload = applied_payload(&[id(1), id(500)], json!([]));
        let domains = [id(300)];
        let notices = translate(row(&payload, &domains, &[]), &kinds);
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].time, Some(1_792_756_800_250_000));
        let rendered = render(&notices[0], &Filters::default(), &All, DEFAULT_BOUND).unwrap();
        assert_eq!(rendered.kind, Kind::Changes);
        let data: Value = serde_json::from_str(&rendered.data).unwrap();
        // REQ-022-4: keys and revisions only; the policy object (no type) is left out.
        assert_eq!(
            data,
            json!({"changeset": id(1000), "objects": [{"type": "core.prefix", "id": id(1)}],
                   "domains": [id(300)]})
        );
        assert!(!rendered.data.contains('\n'));
        assert!(!rendered.data.contains("author"));
    }

    #[test]
    fn beyond_the_bound_an_event_names_types() {
        let objects: Vec<Uuid> = (100..150).map(id).collect();
        let payload = applied_payload(&objects, json!([]));
        let notices = translate(row(&payload, &[id(300)], &[]), &kinds);
        let rendered = render(&notices[0], &Filters::default(), &All, 10).unwrap();
        let data: Value = serde_json::from_str(&rendered.data).unwrap();
        assert_eq!(data["truncated"], json!(true));
        assert_eq!(data["types"], json!(["core.address"]));
        assert!(data.get("objects").is_none());
        assert!(rendered.objects.is_empty());
    }

    #[test]
    fn an_inherited_value_invalidates_the_subtree_by_type() {
        // A property assigned to one prefix, resolved on 40 objects below it.
        let payload = applied_payload(
            &[id(1)],
            json!([{"property": "custom.x", "before": null, "after": "1", "count": 40}]),
        );
        let notices = translate(row(&payload, &[id(300)], &[]), &kinds);
        let rendered = render(&notices[0], &Filters::default(), &All, DEFAULT_BOUND).unwrap();
        let data: Value = serde_json::from_str(&rendered.data).unwrap();
        assert_eq!(data["truncated"], json!(true));
        assert_eq!(
            data["types"],
            json!(["core.address", "core.prefix", "core.range"])
        );
        assert_eq!(data["objects"][0]["id"], json!(id(1)));
    }

    #[test]
    fn a_pending_state_is_a_changeset_event() {
        for state in ["pending", "rejected", "withdrawn", "expired"] {
            let payload = json!({
                "type": format!("org.kartenn.changeset.{state}"),
                "id": format!("{}/{state}", id(1000)),
                "data": {"changeset": id(1000), "author": id(9)},
            });
            let notices = translate(row(&payload, &[], &[]), &kinds);
            assert_eq!(
                notices,
                vec![Notice {
                    seq: 7,
                    time: None,
                    body: Body::Changeset {
                        changeset: id(1000),
                        state: state.into(),
                        author: Some(id(9)),
                    },
                }]
            );
            let rendered = render(&notices[0], &Filters::default(), &All, 10).unwrap();
            assert_eq!(
                rendered.data,
                json!({"changeset": id(1000), "state": state}).to_string()
            );
            assert!(render(&notices[0], &Filters::default(), &Only(vec![]), 10).is_none());
        }
    }

    #[test]
    fn an_approved_changeset_tells_its_state_too() {
        let mut payload = applied_payload(&[id(1)], json!([]));
        payload["data"]["validator"] = json!(id(8));
        let notices = translate(row(&payload, &[id(300)], &[]), &kinds);
        assert_eq!(notices.len(), 2);
        assert!(matches!(&notices[1].body, Body::Changeset { state, .. } if state == "applied"));
    }

    #[test]
    fn a_module_event_is_pushed_only_with_a_typed_subject() {
        let keys = ["vlan:sync".to_owned()];
        let typed = json!({"type": "org.kartenn.vlan.imported", "id": "ev-1",
                           "subject": id(200).to_string(), "data": {}});
        let notices = translate(
            OutboxRow {
                seq: 3,
                changeset: None,
                domains: &[],
                order_keys: &keys,
                payload: &typed,
            },
            &kinds,
        );
        let rendered = render(&notices[0], &Filters::default(), &All, 10).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&rendered.data).unwrap(),
            json!({"task": "vlan:sync", "event": "ev-1",
                   "objects": [{"type": "vlan.vlan", "id": id(200)}], "domains": []})
        );
        let untyped = json!({"type": "org.kartenn.vlan.imported", "id": "ev-2", "data": {}});
        assert!(
            translate(
                OutboxRow {
                    seq: 3,
                    changeset: None,
                    domains: &[],
                    order_keys: &keys,
                    payload: &untyped,
                },
                &kinds
            )
            .is_empty()
        );
    }

    #[test]
    fn labels_reach_every_connection() {
        let payload = json!({"type": "org.kartenn.labels.changed", "data": {"revision": "18-9f"}});
        let notices = translate(row(&payload, &[], &[]), &kinds);
        let rendered = render(&notices[0], &Filters::default(), &Only(vec![]), 10).unwrap();
        assert_eq!(rendered.kind, Kind::Labels);
        assert_eq!(rendered.data, r#"{"revision":"18-9f"}"#);
    }

    #[test]
    fn a_hidden_object_leaves_no_trace() {
        let payload = applied_payload(&[id(1), id(2)], json!([]));
        let notices = translate(row(&payload, &[id(300)], &[]), &kinds);
        // Only the visible one; the domain unseen is left out.
        let rendered = render(&notices[0], &Filters::default(), &Only(vec![id(2)]), 10).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&rendered.data).unwrap()["objects"],
            json!([{"type": "core.prefix", "id": id(2)}])
        );
        assert_eq!(
            serde_json::from_str::<Value>(&rendered.data).unwrap()["domains"],
            json!([])
        );
        assert!(render(&notices[0], &Filters::default(), &Only(vec![id(300)]), 10).is_none());
    }

    #[test]
    fn filters_combine_and_never_widen() {
        let known = |t: &str| ["core.prefix", "vlan.vlan"].contains(&t);
        let f = Filters::parse([("types", "vlan.vlan")], &known).unwrap();
        let payload = applied_payload(&[id(1)], json!([]));
        let notices = translate(row(&payload, &[id(300)], &[]), &kinds);
        assert!(render(&notices[0], &f, &All, 10).is_none());
        let f = Filters::parse([("objects", &*id(1).to_string())], &known).unwrap();
        assert!(render(&notices[0], &f, &All, 10).is_some());
        assert!(render(&notices[0], &f, &Only(vec![]), 10).is_none());
        let f = Filters::parse([("domains", &*id(301).to_string())], &known).unwrap();
        assert!(render(&notices[0], &f, &All, 10).is_none());
        let f = Filters::parse([("kinds", "changeset")], &known).unwrap();
        assert!(render(&notices[0], &f, &All, 10).is_none());
        assert_eq!(
            Filters::parse([("types", "inconnu.truc")], &known),
            Err(FilterError::Value("types", "inconnu.truc".into()))
        );
        assert_eq!(
            Filters::parse([("since", "x")], &known),
            Err(FilterError::Unknown("since".into()))
        );
        assert!(Filters::parse([("kinds", "ready")], &known).is_err());
        let many: Vec<String> = (0..101).map(|n| id(n).to_string()).collect();
        assert_eq!(
            Filters::parse([("objects", &*many.join(","))], &known),
            Err(FilterError::TooMany("objects", MAX_OBJECTS))
        );
    }

    #[test]
    fn frames_follow_the_whatwg_format() {
        let ready = Frame {
            retry: Some(2140),
            event: Some("ready"),
            id: Some("k1.abc"),
            data: Some(r#"{"a":1}"#),
        }
        .encode();
        assert_eq!(
            ready,
            "retry: 2140\nevent: ready\nid: k1.abc\ndata: {\"a\":1}\n\n"
        );
        let mut parser = Parser::default();
        let parsed = parser.feed(&ready);
        assert_eq!(parser.retry, Some(2140));
        assert_eq!(parsed[0].event, "ready");
        assert_eq!(parsed[0].data, r#"{"a":1}"#);
        // A resumption point moves the last identifier without dispatching.
        let point = Frame {
            id: Some("k1.next"),
            ..Frame::default()
        }
        .encode();
        assert!(parser.feed(&point).is_empty());
        assert_eq!(parser.last_id, "k1.next");
        assert!(parser.feed(&comment("ping")).is_empty());
        assert_eq!(parser.comments, 1);
    }

    #[test]
    #[should_panic(expected = "line break")]
    fn a_line_break_in_a_field_is_a_bug() {
        let _ = Frame {
            data: Some("a\nb"),
            ..Frame::default()
        }
        .encode();
    }

    mod property {
        use super::*;
        use proptest::prelude::*;

        fn arb_object() -> impl Strategy<Value = Uuid> {
            (1u128..600).prop_map(Uuid::from_u128)
        }

        proptest! {
            /// Every event rendered is read back by a WHATWG parser as the event itself,
            /// with its identifier, whatever the objects, the visibility and the bound.
            #[test]
            fn every_output_parses_back(
                objects in proptest::collection::vec(arb_object(), 0..40),
                visible in proptest::collection::vec(arb_object(), 0..40),
                bound in 1usize..30,
                count in 0u64..80,
                cursor in "[A-Za-z0-9_-]{1,40}",
            ) {
                let payload = applied_payload(
                    &objects,
                    json!([{"property": "p", "before": null, "after": "x", "count": count}]),
                );
                let domains = [id(300)];
                for notice in translate(row(&payload, &domains, &[]), &kinds) {
                    let Some(rendered) = render(&notice, &Filters::default(), &Only(visible.clone()), bound)
                    else { continue };
                    let id = format!("k1.{cursor}");
                    let block = Frame {
                        event: Some(rendered.kind.as_str()),
                        id: Some(&id),
                        data: Some(&rendered.data),
                        retry: None,
                    }
                    .encode();
                    let mut parser = Parser::default();
                    let parsed = parser.feed(&block);
                    prop_assert_eq!(parsed.len(), 1);
                    prop_assert_eq!(&parsed[0].event, rendered.kind.as_str());
                    prop_assert_eq!(&parsed[0].data, &rendered.data);
                    prop_assert_eq!(&parsed[0].last_id, &id);
                    let data: Value = serde_json::from_str(&parsed[0].data).unwrap();
                    // Nothing hidden is named.
                    for o in data["objects"].as_array().into_iter().flatten() {
                        let oid = Uuid::parse_str(o["id"].as_str().unwrap()).unwrap();
                        prop_assert!(visible.contains(&oid));
                    }
                    if let Some(objects) = data["objects"].as_array() {
                        prop_assert!(objects.len() <= bound);
                    }
                }
            }
        }
    }
}
