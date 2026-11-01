// The end of the session, said before it comes (REQ-021-14; 022/A6, Q-022-11): `GET
// /v1/me` gives the instant the session ends unless seen again (`session.expires_at`);
// every request of the shell to the API sees it again, the event stream and the pages of
// events do not (they are no activity, 022). The shell keeps the last activity of the
// browser (all its tabs share the session: `localStorage`), and warns `WARN_BEFORE_MS`
// before the end it then expects, with a way to stay (WCAG 2.2.1). The end is the server's
// instant, read on its clock (the `Date` of its answers): a browser's clock off by minutes
// is no early nor late warning. A session whose end did not move when seen again ends at a
// fixed instant (its absolute lifetime): said, without the way to stay.

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

const listeners = new Set<() => void>();

/** Records an activity of the session now, for every tab of the browser. */
export function markActivity(now: number = Date.now()): void {
  try {
    storage()?.setItem(ACTIVITY_KEY, String(now));
  } catch {
    // A full or refused storage: this tab still knows.
  }
  for (const listener of listeners) listener();
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
 */
export function activityFetch(inner: typeof fetch = globalThis.fetch): typeof fetch {
  return async (input, init) => {
    const response = await inner(input, init);
    const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    if (!isActivity(url)) return response;
    const date = Date.parse(response.headers.get("date") ?? "");
    if (!Number.isNaN(date)) skew = date - Date.now();
    if (response.status !== 401) markActivity();
    return response;
  };
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
  let warned = false;

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
      warned = false;
      timer = setTimer(schedule, wait);
      return;
    }
    warned = true;
    view.warn(at, extendable);
  }

  const onActivity = () => {
    seen = now();
    schedule();
  };
  listeners.add(onActivity);

  return {
    read(expiresAt) {
      const at = expiresAt ? Date.parse(expiresAt) : Number.NaN;
      if (Number.isNaN(at)) {
        idle = null;
        lastEnd = null;
        if (timer !== null) clearTimer(timer);
        timer = null;
        if (warned) view.clear();
        warned = false;
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
      if (timer !== null) clearTimer(timer);
      timer = null;
    },
  };
}
