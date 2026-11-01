// The end of the session, said before it comes (REQ-021-14; 022/A6, Q-022-11): `GET
// /v1/me` gives the instant the session ends unless seen again (`session.expires_at`);
// every request of the shell to the API sees it again, the event stream and the pages of
// events do not (they are no activity, 022). The shell keeps the last activity of the
// browser (all its tabs share the session: `localStorage`), and warns `WARN_BEFORE_MS`
// before the end it then expects, with a way to stay (WCAG 2.2.1). The end is the server's
// instant, read on its clock (the `Date` of its answers): a browser's clock off by minutes
// is no early nor late warning. A session whose end did not move when seen again ends at a
// fixed instant (its absolute lifetime): said, without the way to stay.
//
// Activity is the user's (decision of the main session, 2026-11-01, during task 11): a read
// born of a gesture — a navigation, a key, the pointer, the wheel (a grid scrolled), within
// `GESTURE_WINDOW_MS` before the request — is activity; a read nobody asked for (an
// invalidation by an event of the stream, a revalidation on focus, the plugins loaded when
// idle) is not, so that a screen that reads in the background does not keep the warning
// away. The server still counts every request but the stream (`seen`, to arbitrate:
// task 16).
//
// Limit (task 16, to arbitrate): `expires_at` is the nearer of the idle end and the
// absolute one, and every request pushes the idle end; the absolute end of a user who
// stays active is only known when it becomes the nearer one at a reading. Probing would
// cancel the idle lifetime; knowing it needs the absolute end in `SessionState`.
import { ApiProblem } from "@kartenn/sdk";

/** How long before the expected end the warning comes. */
export const WARN_BEFORE_MS = 2 * 60_000;
/** Two readings of the same end this far apart say a fixed end. */
export const FIXED_AFTER_MS = 2_000;
/** The last activity of the browser on the API (Unix milliseconds), shared by its tabs. */
export const ACTIVITY_KEY = "kartenn.session.activity";

/** What is no activity of the session: the event stream and its pages (022/A6). */
export function isActivity(url: string): boolean {
  const path = url.startsWith("/") ? url : new URL(url, location.origin).pathname;
  return path.startsWith("/v1/") && !path.startsWith("/v1/events");
}

function storage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

const listeners = new Set<(at: number) => void>();

/**
 * Records an activity of the session at `at` (now by default), for every tab of the
 * browser; an activity older than the one kept (another tab's) leaves it as it is.
 */
export function markActivity(at: number = Date.now()): void {
  const kept = lastActivity();
  if (kept === null || at > kept) {
    try {
      storage()?.setItem(ACTIVITY_KEY, String(at));
    } catch {
      // A full or refused storage: this tab still knows.
    }
  }
  for (const listener of listeners) listener(at);
}

/**
 * How far the server's clock is ahead of the browser's, in milliseconds, from the `Date` of
 * its last answer (RFC 9110 §6.6.1, to the second): `expires_at` is the server's instant.
 */
let skew = 0;

/** The skew of the server's clock on the browser's, as last seen. */
export function serverSkew(): number {
  return skew;
}

/**
 * The `fetch` of the SDK's client, which records each answer of the API as an activity,
 * and the clock of the server from its `Date`.
 *
 * The activity is the instant the request left, not the one its answer came (task 029-5):
 * the server pushes the end from when it sees the request, never before it left. An answer
 * that comes late (a slow server, a device suspended, a clock moved on meanwhile) must not
 * push the expected end past the server's, which would say the warning too late, or never.
 */
export function activityFetch(inner: typeof fetch = globalThis.fetch): typeof fetch {
  return async (input, init) => {
    const sent = Date.now();
    // Asked by the user if a gesture came just before the request left.
    const asked = sent - lastGesture <= GESTURE_WINDOW_MS;
    const response = await inner(input, init);
    const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    if (!isActivity(url)) return response;
    noteServerClock(response);
    if (asked && response.status !== 401) markActivity(sent);
    return response;
  };
}

/** How long after a gesture of the user a request of the shell is still of his doing. */
export const GESTURE_WINDOW_MS = 10_000;
/** The gestures of the user that make the reads after them his (keys, pointer, wheel). */
const GESTURES = ["keydown", "pointerdown", "wheel", "touchstart"] as const;
/** The last gesture of the user in this tab (opening the page is one). */
let lastGesture = Date.now();

/** Records a gesture of the user now (a navigation of the router, an event of the page). */
export function markGesture(now: number = Date.now()): void {
  lastGesture = now;
}

/** Follows the gestures of the user on `target`; the returned function stops. */
export function installGestures(
  target: Pick<Document, "addEventListener" | "removeEventListener"> = document,
): () => void {
  const on = () => markGesture();
  for (const type of GESTURES) target.addEventListener(type, on, { capture: true, passive: true });
  return () => {
    for (const type of GESTURES) target.removeEventListener(type, on, { capture: true });
  };
}

/** Takes the clock of the server from the `Date` of one of its answers. */
export function noteServerClock(response: Response): void {
  const date = Date.parse(response.headers.get("date") ?? "");
  if (!Number.isNaN(date)) skew = date - Date.now();
}

/** What a reading of the end says: the end, a session gone (401), or no answer. */
export type EndReading =
  | { kind: "end"; expiresAt: string | null }
  | { kind: "expired" }
  /** The problem of the answer (its type and `Request-Id`), `null` without an answer. */
  | { kind: "failed"; problem: ApiProblem | null };

/**
 * Reads `session.expires_at` of `GET /v1/me` for « Rester connecté », outside the client of
 * the SDK: the answer is the check of the end, not an activity that would clear the warning
 * or push the expected end before the end is known (the server sees the session again all
 * the same).
 */
export async function readEnd(fetcher: typeof fetch = (i, o) => fetch(i, o)): Promise<EndReading> {
  const response = await fetcher("/v1/me", { credentials: "same-origin" }).catch(() => null);
  if (!response) return { kind: "failed", problem: null };
  noteServerClock(response);
  if (response.status === 401) return { kind: "expired" };
  if (!response.ok) return { kind: "failed", problem: await ApiProblem.from(response) };
  const me = (await response.json().catch(() => null)) as {
    session?: { expires_at?: string | null } | null;
  } | null;
  if (!me) return { kind: "failed", problem: null };
  return { kind: "end", expiresAt: me.session?.expires_at ?? null };
}

function lastActivity(): number | null {
  const value = Number(storage()?.getItem(ACTIVITY_KEY));
  return Number.isFinite(value) && value > 0 ? value : null;
}

/** What the frame shows: the warning, with its end and whether it can be pushed back. */
export interface ExpiryView {
  warn(end: number, extendable: boolean): void;
  clear(): void;
}

export interface Expiry {
  /** A new reading of `session.expires_at` (`null`: none, nothing to say). */
  read(expiresAt: string | null | undefined): void;
  stop(): void;
}

export interface ExpiryDeps {
  now?: () => number;
  /** The server's clock ahead of the browser's (`serverSkew`). */
  skew?: () => number;
  setTimer?: (run: () => void, ms: number) => unknown;
  clearTimer?: (timer: unknown) => void;
}

/**
 * The watch of the end of the session: from each reading of `expires_at`, the length of its
 * idle window; the expected end follows the last activity (this tab's or another's); the
 * warning comes `WARN_BEFORE_MS` before it and goes with the next activity.
 */
export function createExpiry(view: ExpiryView, deps: ExpiryDeps = {}): Expiry {
  const now = deps.now ?? Date.now;
  const skewOf = deps.skew ?? serverSkew;
  /** The skew of the server's clock at the last reading. */
  let offset = 0;
  const setTimer = deps.setTimer ?? ((run, ms) => setTimeout(run, ms));
  const clearTimer =
    deps.clearTimer ?? ((timer) => clearTimeout(timer as ReturnType<typeof setTimeout>));
  /** The idle window of the session, in milliseconds; `null` without a session end. */
  let idle: number | null = null;
  let seen = 0;
  let lastEnd: number | null = null;
  let lastRead = 0;
  let extendable = true;
  let timer: unknown = null;
  /** The warning said, by its end and its way (`<end>:<extendable>`); `null` if none. */
  let warned: string | null = null;

  /** The end expected now: the last activity plus the window, or the fixed end. */
  const end = () => {
    if (idle === null || lastEnd === null) return null;
    return extendable ? Math.max(seen, lastActivity() ?? 0) + idle : lastEnd - offset;
  };

  function schedule(): void {
    if (timer !== null) clearTimer(timer);
    timer = null;
    const at = end();
    if (at === null) return;
    const wait = at - WARN_BEFORE_MS - now();
    if (wait > 0) {
      if (warned) view.clear();
      warned = null;
      timer = setTimer(schedule, wait);
      return;
    }
    // Said once for an end and a way: a fixed end stays said through the activity.
    const said = `${at}:${extendable}`;
    if (warned === said) return;
    warned = said;
    view.warn(at, extendable);
  }

  const onActivity = (at: number) => {
    seen = Math.max(seen, at);
    schedule();
  };
  listeners.add(onActivity);
  // The activity of another tab of the browser, kept under ACTIVITY_KEY: the expected end
  // follows it, a warning already said goes.
  const onStorage = (event: StorageEvent) => {
    if (event.key === ACTIVITY_KEY) schedule();
  };
  globalThis.addEventListener?.("storage", onStorage);

  return {
    read(expiresAt) {
      const at = expiresAt ? Date.parse(expiresAt) : Number.NaN;
      if (Number.isNaN(at)) {
        idle = null;
        lastEnd = null;
        if (timer !== null) clearTimer(timer);
        timer = null;
        if (warned) view.clear();
        warned = null;
        return;
      }
      // Seen again a while later (the server counts in seconds), an end that did not move
      // is a fixed one (the absolute lifetime): no way to push it back.
      const read = now();
      extendable = lastEnd === null || at > lastEnd || read - lastRead < FIXED_AFTER_MS;
      lastEnd = at;
      lastRead = read;
      seen = read;
      // `at` is the server's instant: read on its clock, then kept on the browser's.
      offset = skewOf();
      idle = Math.max(0, at - offset - seen);
      schedule();
    },
    stop() {
      listeners.delete(onActivity);
      globalThis.removeEventListener?.("storage", onStorage);
      if (timer !== null) clearTimer(timer);
      timer = null;
    },
  };
}
