// The end of the session, said before it comes (REQ-021-14; 022/A6, Q-022-11): `GET
// /v1/me` gives the instant the session ends unless seen again (`session.expires_at`, the
// nearer of its idle end and its absolute one) and the end of its lifetime alone
// (`session.absolute_expires_at`, the same over the whole session; 021/A14). The shell
// keeps the last activity of the browser (all its tabs share the session:
// `localStorage`), and warns `WARN_BEFORE_MS` before the end it then expects: the idle
// end, with a way to stay (WCAG 2.2.1), or the absolute one once the idle end would come
// after it, without that way, which would not push it back. The end is the server's
// instant, read on its clock (the `Date` of its answers): a browser's clock off by minutes
// is no early nor late warning.
//
// Activity is the user's (decision of the main session, 2026-11-01, during task 11): a read
// born of a gesture — a navigation, a key, the pointer, the wheel (a grid scrolled), within
// `GESTURE_WINDOW_MS` before the request — is activity; a read nobody asked for (an
// invalidation by an event of the stream, a revalidation on focus or on reconnection, a
// prefetch on hover, a poll, the plugins loaded when idle) is not, so that a screen that
// reads in the background does not keep the session alive. The server is told so by
// `Kartenn-Background: ?1` (021/A14, RFC 9651 §3.3.6), set by the `fetch` of the SDK's
// client (`activityFetch`) on exactly the reads this rule says are no gesture: the server
// then neither pushes the idle end nor reissues the cookie, and both ends agree. The event
// stream (`EventSource`) cannot carry a header: the server already counts it as no
// activity, like the pages of events (022/A6).
import { ApiProblem } from "@kartenn/sdk";

/** How long before the expected end the warning comes. */
export const WARN_BEFORE_MS = 2 * 60_000;
/** The header that says a request of the background, no activity (021/A14). */
export const BACKGROUND_HEADER = "Kartenn-Background";
/** The methods of a read: the others (writes) are the user's, whenever they leave. */
const READS = new Set(["GET", "HEAD", "QUERY"]);
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
 *
 * A read of the API that no gesture came just before leaves with `Kartenn-Background: ?1`
 * and is no activity; one that a gesture came before, and every write, leave without it and
 * are. A read asked once and read again later (a revalidation, an event) is judged again at
 * each request: the header follows the gesture, not the query.
 */
export function activityFetch(inner: typeof fetch = globalThis.fetch): typeof fetch {
  return async (input, init) => {
    const sent = Date.now();
    const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    if (!isActivity(url)) return inner(input, init);
    const method = (
      init?.method ?? (input instanceof Request ? input.method : "GET")
    ).toUpperCase();
    // Asked by the user if a gesture came just before the request left; a write always is.
    const asked = !READS.has(method) || sent - lastGesture <= GESTURE_WINDOW_MS;
    let response: Response;
    if (asked) response = await inner(input, init);
    else {
      const headers = new Headers(init?.headers ?? (input instanceof Request ? input.headers : {}));
      headers.set(BACKGROUND_HEADER, "?1");
      response = await inner(input, { ...init, headers });
    }
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
  | { kind: "end"; expiresAt: string | null; absoluteAt: string | null }
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
    session?: { expires_at?: string | null; absolute_expires_at?: string | null } | null;
  } | null;
  if (!me) return { kind: "failed", problem: null };
  return {
    kind: "end",
    expiresAt: me.session?.expires_at ?? null,
    absoluteAt: me.session?.absolute_expires_at ?? null,
  };
}

/**
 * The last activity kept by the browser. One kept in the future (the clock set back since,
 * by hand or by NTP) is brought back to now, kept so: it would push the expected end, and
 * keep every activity of the tabs from replacing it, for as long as the clock was moved.
 */
function lastActivity(): number | null {
  const value = Number(storage()?.getItem(ACTIVITY_KEY));
  if (!Number.isFinite(value) || value <= 0) return null;
  const now = Date.now();
  if (value <= now) return value;
  try {
    storage()?.setItem(ACTIVITY_KEY, String(now));
  } catch {
    // A full or refused storage: now all the same.
  }
  return now;
}

/** Forgets the last activity of the browser: a logout, a new login (another session). */
export function forgetActivity(): void {
  try {
    storage()?.removeItem(ACTIVITY_KEY);
  } catch {
    // A refused storage keeps nothing either.
  }
}

/** What the frame shows: the warning, with its end and whether it can be pushed back. */
export interface ExpiryView {
  warn(end: number, extendable: boolean): void;
  clear(): void;
}

export interface Expiry {
  /**
   * A new reading of `session.expires_at` (`null`: none, nothing to say) and of
   * `session.absolute_expires_at` (`null`: none, the idle end alone).
   */
  read(expiresAt: string | null | undefined, absoluteAt?: string | null): void;
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
 * The watch of the end of the session: each reading gives the end as the server then knew
 * it, and a bound of its idle window (the end minus the instant of the reading); the
 * expected end follows the activity since (this tab's or another's), up to the absolute
 * end; the warning comes `WARN_BEFORE_MS` before it and goes with the next activity, unless
 * the end is the absolute one, which no activity pushes back.
 */
export function createExpiry(view: ExpiryView, deps: ExpiryDeps = {}): Expiry {
  const now = deps.now ?? Date.now;
  const skewOf = deps.skew ?? serverSkew;
  const setTimer = deps.setTimer ?? ((run, ms) => setTimeout(run, ms));
  const clearTimer =
    deps.clearTimer ?? ((timer) => clearTimeout(timer as ReturnType<typeof setTimeout>));
  /**
   * The idle window of the session, in milliseconds: the longest seen between a reading and
   * its end, never longer than the true one (a reading of the background, or one whose end
   * is the absolute one, gives a shorter one: a warning early, never late).
   */
  let idle = 0;
  /** The end at the last reading, on the browser's clock; `null` without a session end. */
  let lastEnd: number | null = null;
  /** The absolute end, on the browser's clock; `null` if the session gives none. */
  let absolute: number | null = null;
  /** The last activity of this tab (the browser's storage may refuse to keep it). */
  let seen = 0;
  let timer: unknown = null;
  /** The warning said, by the minute of its end and its way; `null` if none. */
  let warned: string | null = null;

  /** The end expected now, and whether an activity would push it back. */
  const end = () => {
    if (lastEnd === null) return null;
    const idleEnd = Math.max(lastEnd, Math.max(seen, lastActivity() ?? 0) + idle);
    return absolute !== null && absolute <= idleEnd
      ? { at: absolute, extendable: false }
      : { at: idleEnd, extendable: true };
  };

  function schedule(): void {
    if (timer !== null) clearTimer(timer);
    timer = null;
    const expected = end();
    if (expected === null) return;
    const { at, extendable } = expected;
    const wait = at - WARN_BEFORE_MS - now();
    if (wait > 0) {
      if (warned) view.clear();
      warned = null;
      timer = setTimer(schedule, wait);
      return;
    }
    // Said once for an end as written (to the minute) and a way: a fixed end stays said
    // through the activity, and an end moved by less (an activity dated from its request,
    // its answer come during the warning) is no new warning (task 029-5).
    const said = `${Math.floor(at / 60_000)}:${extendable}`;
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
    read(expiresAt, absoluteAt) {
      const at = expiresAt ? Date.parse(expiresAt) : Number.NaN;
      if (Number.isNaN(at)) {
        lastEnd = null;
        absolute = null;
        idle = 0;
        if (timer !== null) clearTimer(timer);
        timer = null;
        if (warned) view.clear();
        warned = null;
        return;
      }
      // The server's instants: read on its clock, then kept on the browser's.
      const offset = skewOf();
      const fixed = absoluteAt ? Date.parse(absoluteAt) : Number.NaN;
      lastEnd = at - offset;
      absolute = Number.isNaN(fixed) ? null : fixed - offset;
      idle = Math.max(idle, lastEnd - now());
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
