//! The body of a connection (spec 022, REQ-022-6, 9 to 12, 18, 21): the events of its
//! channel as blocks of `text/event-stream`, each with its cursor; a comment after a
//! silence; a resumption point at most every 30 s when its position moved; a `reset`
//! after an overflow or a change of rights; a `retry:` then the end at the stop; the end
//! at the expiry of its credentials or at a revocation. Dropped, it frees its place.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use axum::body::Bytes;
use kartenn_service::events::{
    Frame, Kind, ResetReason, STATS_DATA, comment, ready_data, reset_data,
};
use tokio::sync::{mpsc, watch};
use tokio::time::Instant as TokioInstant;
use uuid::Uuid;

use super::cursor::{Cursor, Position};
use super::hub::{Cause, Conn, Hub, Message, jitter};

/// What a connection did, for `events.stream.closed` (REQ-022-24).
#[derive(Debug, Clone, Default)]
pub struct Tally {
    /// Events written.
    pub delivered: u64,
    /// `reset` written.
    pub resets: u64,
    /// The objects named, as many as the audit records.
    pub objects: Vec<Uuid>,
    /// The objects beyond that bound.
    pub omitted: u64,
}

/// Where a connection reports: its deliveries beyond the bound of the audit, and its end.
pub trait Report: Send + Sync {
    /// The bound of the identifiers recorded by `events.stream.closed`.
    fn ids_max(&self) -> usize;
    /// An event beyond the bound (`events.delivery`, DEBUG, REQ-022-24).
    fn beyond(&self, conn: u64, identity: Uuid, kind: Kind, objects: &[Uuid]);
    /// The connection ended.
    fn closed(&self, cause: Cause, duration_ms: u64, tally: Tally);
}

/// The state of a body.
pub struct Body {
    hub: Arc<Hub>,
    conn: Arc<Conn>,
    rx: mpsc::Receiver<Message>,
    queue: VecDeque<String>,
    position: Position,
    checkpointed: Position,
    heartbeat_at: TokioInstant,
    checkpoint_at: TokioInstant,
    deadline: Option<TokioInstant>,
    closing: watch::Receiver<bool>,
    finished: bool,
    cause: Cause,
    started: Instant,
    tally: Tally,
    report: Option<Arc<dyn Report>>,
}

impl std::fmt::Debug for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body")
            .field("conn", &self.conn.id)
            .field("position", &self.position)
            .finish_non_exhaustive()
    }
}

/// How a body starts.
#[derive(Debug)]
pub struct Start {
    /// The position of `ready`: the cursor resumed, or the floor of the hub.
    pub position: Position,
    /// The `reset` to write after `ready`.
    pub reset: Option<(ResetReason, i64)>,
    /// The events replayed.
    pub replay: Vec<Message>,
    /// When the credentials expire (Unix seconds).
    pub expires: Option<i64>,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl Body {
    /// The body of `conn`: `ready`, then the `reset` or the events replayed.
    #[must_use]
    pub fn new(
        hub: Arc<Hub>,
        conn: Arc<Conn>,
        rx: mpsc::Receiver<Message>,
        start: Start,
        report: Option<Arc<dyn Report>>,
    ) -> Self {
        let now = TokioInstant::now();
        let closing = hub.closing();
        let deadline = start.expires.map(|at| {
            let left = u64::try_from(at - unix_now()).unwrap_or(0);
            now + std::time::Duration::from_secs(left)
        });
        let mut body = Self {
            heartbeat_at: now + hub.settings.heartbeat,
            checkpoint_at: now + hub.settings.checkpoint,
            hub,
            conn,
            rx,
            queue: VecDeque::new(),
            position: start.position,
            checkpointed: start.position,
            deadline,
            closing,
            finished: false,
            cause: Cause::Client,
            started: Instant::now(),
            tally: Tally::default(),
            report,
        };
        let id = body.cursor();
        let data = ready_data(
            body.hub.instance,
            body.hub.settings.heartbeat.as_secs(),
            body.hub.settings.ring_age.as_secs(),
        );
        body.queue.push_back(
            Frame {
                retry: Some(jitter(body.hub.settings.retry_open)),
                event: Some(Kind::Ready.as_str()),
                id: Some(&id),
                data: Some(&data),
            }
            .encode(),
        );
        if let Some((reason, floor)) = start.reset {
            body.reset(reason, floor);
        }
        for m in start.replay {
            body.write(m);
        }
        body
    }

    fn cursor(&self) -> String {
        Cursor {
            position: self.position,
            identity: self.conn.subject_identity(),
            issued: unix_now(),
        }
        .encode(&self.hub.ring_keys)
    }

    fn reset(&mut self, reason: ResetReason, floor: i64) {
        self.position = Position {
            floor,
            last: self.position.last.max(floor),
        };
        let id = self.cursor();
        self.queue.push_back(
            Frame {
                event: Some(Kind::Reset.as_str()),
                id: Some(&id),
                data: Some(&reset_data(reason)),
                retry: None,
            }
            .encode(),
        );
        self.tally.resets += 1;
        self.hub.stats.resets[super::hub::Stats::reset_index(reason)]
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn write(&mut self, message: Message) {
        let (kind, data, objects, seq, floor, time) = match message {
            Message::Event {
                kind,
                data,
                objects,
                seq,
                floor,
                time,
            } => (kind, data, objects, seq, floor, time),
            // Outside the outbox: no cursor, the position stays (spec 024, REQ-024-16).
            Message::Signal { kind } => {
                self.queue.push_back(
                    Frame {
                        event: Some(kind.as_str()),
                        id: None,
                        data: Some(STATS_DATA),
                        retry: None,
                    }
                    .encode(),
                );
                self.tally.delivered += 1;
                return;
            }
        };
        self.position = Position {
            floor,
            last: self.position.last.max(seq),
        };
        let id = self.cursor();
        self.queue.push_back(
            Frame {
                event: Some(kind.as_str()),
                id: Some(&id),
                data: Some(&data),
                retry: None,
            }
            .encode(),
        );
        self.tally.delivered += 1;
        self.hub.stats.delivered(time);
        if let Some(report) = &self.report {
            let room = report.ids_max().saturating_sub(self.tally.objects.len());
            let (kept, beyond) = objects.split_at(room.min(objects.len()));
            self.tally.objects.extend_from_slice(kept);
            if !beyond.is_empty() {
                self.tally.omitted += beyond.len() as u64;
                report.beyond(self.conn.id, self.conn.subject_identity(), kind, beyond);
            }
        }
    }

    /// A resumption point (REQ-022-9): the floor published, when nothing waits.
    fn checkpoint(&mut self) {
        if !self.rx.is_empty() {
            return;
        }
        let floor = self.hub.floor();
        if floor > self.position.floor {
            self.position = Position {
                floor,
                last: self.position.last.max(floor),
            };
        }
        if self.position != self.checkpointed {
            self.checkpointed = self.position;
            let id = self.cursor();
            self.queue.push_back(
                Frame {
                    id: Some(&id),
                    ..Frame::default()
                }
                .encode(),
            );
        }
    }

    /// The next block, `None` at the end.
    pub async fn next(&mut self) -> Option<Bytes> {
        loop {
            if let Some(block) = self.queue.pop_front() {
                self.heartbeat_at = TokioInstant::now() + self.hub.settings.heartbeat;
                return Some(Bytes::from(block));
            }
            if self.finished {
                return None;
            }
            if let Some(cause) = Hub::ending(&self.conn) {
                self.cause = cause;
                self.finished = true;
                continue;
            }
            if let Some((floor, reason)) = self.hub.take_overflow(&self.conn, &mut self.rx) {
                self.reset(reason, floor);
                continue;
            }
            if *self.closing.borrow() {
                self.stop();
                continue;
            }
            let deadline = self.deadline;
            tokio::select! {
                biased;
                _ = self.closing.changed() => self.stop(),
                () = self.conn.wake.notified() => {}
                m = self.rx.recv() => {
                    if let Some(m) = m {
                        self.write(m);
                    } else {
                        self.cause = Hub::ending(&self.conn).unwrap_or(Cause::Overflow);
                        self.finished = true;
                    }
                }
                () = tokio::time::sleep_until(self.heartbeat_at) => {
                    self.queue.push_back(comment("ping"));
                }
                () = tokio::time::sleep_until(self.checkpoint_at) => {
                    self.checkpoint_at = TokioInstant::now() + self.hub.settings.checkpoint;
                    self.checkpoint();
                }
                () = async {
                    match deadline {
                        Some(at) => tokio::time::sleep_until(at).await,
                        None => std::future::pending().await,
                    }
                } => {
                    self.cause = Cause::Expired;
                    self.finished = true;
                }
            }
        }
    }

    /// The stop: a `retry:` drawn wide, then the end (REQ-022-12).
    fn stop(&mut self) {
        self.queue.push_back(format!(
            "retry: {}\n\n",
            jitter(self.hub.settings.retry_stop)
        ));
        self.cause = Cause::Shutdown;
        self.finished = true;
    }
}

impl Drop for Body {
    fn drop(&mut self) {
        self.hub.release(self.conn.id);
        if let Some(report) = &self.report {
            report.closed(
                self.cause,
                u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
                std::mem::take(&mut self.tally),
            );
        }
    }
}

/// The body as a stream of blocks for the response.
pub fn stream(
    body: Body,
) -> impl futures_util::Stream<Item = Result<Bytes, std::convert::Infallible>> + Send {
    futures_util::stream::unfold(body, |mut body| async move {
        body.next().await.map(|block| (Ok(block), body))
    })
}
