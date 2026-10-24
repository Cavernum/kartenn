// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The event stream of the browser (design « Flux d'événements », REQ-021-23; 022 « Contrat
// consommé par les clients »): one `GET /v1/events` per browser. The tabs contend for the
// lock `kartenn-events` (`navigator.locks`); the holder opens the `EventSource` and relays
// each event on `BroadcastChannel('kartenn-events')`; when its tab closes, another takes
// the lock and opens again after the last cursor received. Without `navigator.locks` or
// `BroadcastChannel`, one stream per tab (Q-021-3).

/** The kinds of event of the stream (022, `x-kartenn-events`). */
export type EventKind = "ready" | "changes" | "changeset" | "labels" | "reset";
export const KINDS: readonly EventKind[] = ["ready", "changes", "changeset", "labels", "reset"];

/** An event as the SDK hands it: its kind, its data, its cursor. */
export interface KartennEvent {
  readonly kind: EventKind;
  readonly data: Record<string, unknown>;
  /** The opaque cursor (`id:`), to resume after. */
  readonly id: string | null;
}

export type StreamState = "open" | "connecting" | "closed";

/** What the stream needs of an `EventSource`. */
export interface SourceLike {
  readonly readyState: number;
  onerror: ((event: unknown) => void) | null;
  addEventListener(
    type: string,
    listener: (event: { data: string; lastEventId: string }) => void,
  ): void;
  close(): void;
}

/** What the stream needs of a `BroadcastChannel`. */
export interface ChannelLike {
  postMessage(message: Relay): void;
  onmessage: ((event: { data: Relay }) => void) | null;
  close(): void;
}

/** What the stream needs of `navigator.locks`. */
export interface LocksLike {
  request(
    name: string,
    options: { signal: AbortSignal },
    callback: () => Promise<void>,
  ): Promise<unknown>;
}

/** The messages between tabs. */
export type Relay =
  | { type: "event"; event: KartennEvent }
  | { type: "state"; state: StreamState }
  | { type: "hello" };

/** What a closed stream finds when it checks the session. */
export type Check = "ok" | "unauthorized" | "disabled";

export interface StreamDeps {
  /** Opens an `EventSource` on a URL. */
  open: (url: string) => SourceLike;
  /** The lock manager of the browser; `null` for a stream per tab. */
  locks: LocksLike | null;
  /** A channel between the tabs; `null` for a stream per tab. */
  channel: ((name: string) => ChannelLike) | null;
  /** After a closure (401, 429, 503…): the session, and whether the server has a stream. */
  check: () => Promise<Check>;
  /** Applies an event to the cache. */
  apply: (event: KartennEvent) => unknown;
  /** The path of the stream. */
  url?: string;
  setTimeout?: (run: () => void, ms: number) => unknown;
  clearTimeout?: (timer: unknown) => void;
  random?: () => number;
}

export const LOCK = "kartenn-events";
export const CHANNEL = "kartenn-events";
/** The identities of events kept to recognise a duplicate (022, « Doublons »). */
export const SEEN_WINDOW = 1000;
/** The wait before reopening: doubled at each failure, capped, drawn at random under it. */
export const BACKOFF = { base: 1000, cap: 60_000 } as const;

const CLOSED = 2;

/**
 * The identity of an event, to recognise an exact duplicate: the changeset (or the event
 * of a task) of `changes`, the changeset and its state, the revision of the labels.
 * `ready` and `reset` have none: they always apply.
 */
export function identity(event: KartennEvent): string | null {
  const data = event.data;
  const text = (name: string) => (typeof data[name] === "string" ? (data[name] as string) : null);
  switch (event.kind) {
    case "changes": {
      const id = text("changeset") ?? text("event");
      return id === null ? null : `changes:${id}`;
    }
    case "changeset":
      return `changeset:${text("changeset")}:${text("state")}`;
    case "labels":
      return `labels:${text("revision")}`;
    default:
      return null;
  }
}

export class EventStream {
  private current: StreamState = "closed";
  private source: SourceLike | null = null;
  private channel: ChannelLike | null = null;
  private cursor: string | null = null;
  private attempt = 0;
  private timer: unknown = null;
  private leading = false;
  private running = false;
  private abort: AbortController | null = null;
  private release: (() => void) | null = null;
  private readonly seen = new Set<string>();
  private readonly order: string[] = [];
  private readonly listeners = new Set<(event: KartennEvent) => void>();
  private readonly stateListeners = new Set<(state: StreamState) => void>();

  constructor(private readonly deps: StreamDeps) {}

  /** Opens the stream, or joins the one of another tab: before the first reads (022). */
  start(): void {
    if (this.running) return;
    this.running = true;
    const { locks, channel } = this.deps;
    if (!locks || !channel) {
      this.lead();
      return;
    }
    this.channel = channel(CHANNEL);
    this.channel.onmessage = ({ data }) => this.relayed(data);
    this.abort = new AbortController();
    locks
      .request(LOCK, { signal: this.abort.signal }, () => {
        if (!this.running) return Promise.resolve();
        this.lead();
        return new Promise<void>((done) => (this.release = done));
      })
      .catch(() => undefined);
    this.channel.postMessage({ type: "hello" });
  }

  /** Closes the stream and gives the lock to another tab. */
  stop(): void {
    this.running = false;
    this.leading = false;
    this.cancelTimer();
    this.source?.close();
    this.source = null;
    this.abort?.abort();
    this.abort = null;
    this.release?.();
    this.release = null;
    this.channel?.close();
    this.channel = null;
    this.setState("closed");
  }

  /** Opens again after a closure the session check left closed (a new login). */
  restart(): void {
    if (!this.running) return this.start();
    if (this.leading && this.source === null) {
      this.cancelTimer();
      this.attempt = 0;
      this.connect();
    }
  }

  state(): StreamState {
    return this.current;
  }

  /** Whether this tab holds the stream of the browser. */
  get leader(): boolean {
    return this.leading;
  }

  /** Each event applied, in this tab; returns the function that stops listening. */
  subscribe(listener: (event: KartennEvent) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Each change of state; returns the function that stops listening. */
  onState(listener: (state: StreamState) => void): () => void {
    this.stateListeners.add(listener);
    return () => this.stateListeners.delete(listener);
  }

  private lead(): void {
    this.leading = true;
    this.connect();
  }

  private connect(): void {
    if (!this.running) return;
    const base = this.deps.url ?? "/v1/events";
    // A new `EventSource` does not send `Last-Event-ID`: the cursor goes as a parameter.
    const url = this.cursor ? `${base}?last_event_id=${encodeURIComponent(this.cursor)}` : base;
    const source = this.deps.open(url);
    this.source = source;
    this.setState("connecting");
    for (const kind of KINDS) {
      source.addEventListener(kind, (message) => {
        if (this.source !== source) return;
        let data: Record<string, unknown> = {};
        try {
          data = JSON.parse(message.data) as Record<string, unknown>;
        } catch {
          data = {};
        }
        const event: KartennEvent = { kind, data, id: message.lastEventId || null };
        if (kind === "ready") {
          this.attempt = 0;
          this.setState("open");
        }
        this.deliver(event);
        this.channel?.postMessage({ type: "event", event });
      });
    }
    source.onerror = () => {
      if (this.source !== source) return;
      // CONNECTING: the browser resumes by itself with `Last-Event-ID`. CLOSED: a status
      // other than 200 (WHATWG § 9.2.2), the SDK takes over.
      if (source.readyState === CLOSED) this.closed(source);
      else this.setState("connecting");
    };
  }

  private closed(source: SourceLike): void {
    source.close();
    this.source = null;
    this.setState("closed");
    this.deps
      .check()
      .catch((): Check => "ok")
      .then((check) => {
        if (!this.running || !this.leading || check !== "ok") return;
        const ceiling = Math.min(BACKOFF.cap, BACKOFF.base * 2 ** this.attempt);
        this.attempt += 1;
        const delay = Math.round((this.deps.random ?? Math.random)() * ceiling);
        this.cancelTimer();
        this.timer = (this.deps.setTimeout ?? setTimeout)(() => {
          this.timer = null;
          this.connect();
        }, delay);
      });
  }

  private relayed(message: Relay): void {
    if (this.leading) {
      if (message.type === "hello")
        this.channel?.postMessage({ type: "state", state: this.current });
      return;
    }
    if (message.type === "event") this.deliver(message.event);
    else if (message.type === "state") this.setState(message.state, false);
  }

  private deliver(event: KartennEvent): void {
    if (event.id) this.cursor = event.id;
    const id = identity(event);
    if (id !== null) {
      if (this.seen.has(id)) return;
      this.seen.add(id);
      this.order.push(id);
      if (this.order.length > SEEN_WINDOW) this.seen.delete(this.order.shift()!);
    }
    void Promise.resolve(this.deps.apply(event)).catch(() => undefined);
    for (const listener of [...this.listeners]) listener(event);
  }

  private setState(state: StreamState, relay = true): void {
    if (state === this.current) return;
    this.current = state;
    if (relay && this.leading) this.channel?.postMessage({ type: "state", state });
    for (const listener of [...this.stateListeners]) listener(state);
  }

  private cancelTimer(): void {
    if (this.timer === null) return;
    if (this.deps.clearTimeout) this.deps.clearTimeout(this.timer);
    else clearTimeout(this.timer as ReturnType<typeof setTimeout>);
    this.timer = null;
  }
}
