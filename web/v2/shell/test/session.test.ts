// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiProblem } from "@kartenn/sdk";
import { createToasts } from "@kartenn/ui";
import { ref } from "vue";
import { createMemoryHistory, createRouter } from "vue-router";
import { watchSessionEnd } from "../src/session/warning.ts";
import { installSession } from "../src/frame/session.ts";
import { THEME_KEY, MOTION_KEY } from "../src/frame/settings.ts";
import { LOCALE_KEY } from "../src/i18n/index.ts";
import type { Me } from "../src/rights/index.ts";
import {
  ACTIVITY_KEY,
  activityFetch,
  createExpiry,
  GESTURE_WINDOW_MS,
  installGestures,
  isActivity,
  markActivity,
  markGesture,
  readEnd,
  serverSkew,
  WARN_BEFORE_MS,
  type EndReading,
} from "../src/session/expiry.ts";
import {
  breakglassLogin,
  clearSensitive,
  expired,
  internalPath,
  ISSUER_KEY,
  lastIssuer,
  loginAt,
  logout,
  readIssuers,
  rememberReturn,
  RETURN_KEY,
  returnAfterLogin,
  takeReturn,
} from "../src/session/index.ts";
import { RECENT_KEY, recentObjects, rememberObject } from "../src/shortcuts/recent.ts";

// The session of the new shell (spec 021, REQ-021-14; REQ-007-28): the return URL that
// never leaves the shell, the guard that sends to the login page and back, the logout that
// clears what is sensitive, the end of the session said before it comes.

beforeEach(() => {
  localStorage.clear();
  sessionStorage.clear();
});

describe("the URL to come back to", () => {
  it("is an internal path of the shell only", () => {
    expect(internalPath("/prefixes/p1?valid_at=2026-01-01T00%3A00%3A00Z")).toBe(
      "/prefixes/p1?valid_at=2026-01-01T00%3A00%3A00Z",
    );
    for (const outside of [
      "//evil.example",
      "/\\evil.example",
      "https://evil.example/",
      "/login",
      "/login?return=/",
      "/v1/me",
      3,
    ]) {
      expect(internalPath(outside)).toBeNull();
    }
  });

  it("is kept for the tab, read once", () => {
    rememberReturn("/sites?q=1");
    rememberReturn("//evil.example");
    expect(takeReturn()).toBe("/sites?q=1");
    expect(takeReturn()).toBeNull();
  });

  it("goes with the issuer, which is kept for « Se reconnecter »", () => {
    const went: string[] = [];
    // The activity of the session before is no activity of the next one.
    localStorage.setItem(ACTIVITY_KEY, "1");
    loginAt("dev", "/x/flow/flows", (url) => went.push(url));
    expect(went).toEqual(["/v1/auth/login?issuer=dev"]);
    expect(lastIssuer()).toBe("dev");
    expect(sessionStorage.getItem(RETURN_KEY)).toBe("/x/flow/flows");
    expect(localStorage.getItem(ACTIVITY_KEY)).toBeNull();
  });
});

describe("the guard of the session", () => {
  const Empty = { render: () => null };
  const ME = { identity: "0b5e3c1a-0000-4000-8000-000000000009" } as Me;

  function router(me: Me | null, starts: { n: number } = { n: 0 }) {
    const r = createRouter({
      history: createMemoryHistory(),
      routes: [
        {
          path: "/",
          component: Empty,
          meta: { frame: true },
          children: [
            { path: "", component: Empty },
            { path: "sites", component: Empty },
            { path: "login", component: Empty, meta: { public: true } },
            { path: "about", component: Empty, meta: { public: true } },
          ],
        },
      ],
    });
    installSession(
      r,
      async () => ({ me }),
      () => starts.n++,
      () => ({ me: null }),
    );
    return r;
  }

  it("sends a page that needs a session to the login page, with its URL", async () => {
    const r = router(null);
    await r.push("/sites?valid_at=2026-01-01T00%3A00%3A00Z");
    expect(r.currentRoute.value.path).toBe("/login");
    expect(r.currentRoute.value.query.return).toBe("/sites?valid_at=2026-01-01T00%3A00%3A00Z");
    await r.push("/about");
    expect(r.currentRoute.value.path).toBe("/about");
  });

  it("goes back to the URL kept after a login, and starts the data layer once", async () => {
    rememberReturn("/sites?q=1");
    const starts = { n: 0 };
    const r = router(ME, starts);
    await r.push("/");
    expect(r.currentRoute.value.fullPath).toBe("/sites?q=1");
    await r.push("/");
    expect(r.currentRoute.value.fullPath).toBe("/");
    expect(starts.n).toBe(1);
  });

  it("goes, after a login in place, to the return of its URL over an older one kept", async () => {
    // `/sites` kept by an older login; the login page asked with `?return=/about`.
    rememberReturn("/sites?old=1");
    const r = router(ME);
    await r.push(returnAfterLogin("/about"));
    expect(r.currentRoute.value.fullPath).toBe("/about");
    await r.push("/");
    expect(r.currentRoute.value.fullPath).toBe("/");
    // Without a return in its URL, the one kept.
    rememberReturn("/sites?old=1");
    expect(returnAfterLogin(undefined)).toBe("/sites?old=1");
    expect(returnAfterLogin("//evil.example")).toBe("/");
  });
});

describe("the login page's reads", () => {
  it("reads the issuers by their identifier, the problem when they cannot be read", async () => {
    const ok = vi.fn(async () => Response.json([{ id: "dev" }, { id: 3 }, { id: "corp" }]));
    expect(await readIssuers(ok as unknown as typeof fetch)).toEqual(["dev", "corp"]);
    const down = vi.fn(
      async () =>
        new Response(JSON.stringify({ type: "urn:kartenn:problem:unavailable", title: "Down" }), {
          status: 503,
          headers: { "content-type": "application/problem+json", "request-id": "r-2" },
        }),
    );
    expect(await readIssuers(down as unknown as typeof fetch)).toMatchObject({
      type: "urn:kartenn:problem:unavailable",
      requestId: "r-2",
    });
  });

  it("gives the problem of a refused break-glass login, with its Request-Id", async () => {
    const refused = vi.fn(
      async () =>
        new Response(
          JSON.stringify({ type: "urn:kartenn:problem:unauthenticated", title: "Unauthorized" }),
          {
            status: 401,
            headers: { "content-type": "application/problem+json", "request-id": "r-1" },
          },
        ),
    );
    const problem = await breakglassLogin(
      { name: "e2e", passphrase: "x".repeat(15), code: "000000" },
      refused as unknown as typeof fetch,
    );
    expect(problem?.type).toBe("urn:kartenn:problem:unauthenticated");
    expect(problem?.requestId).toBe("r-1");
    localStorage.setItem(ACTIVITY_KEY, "1");
    const accepted = vi.fn(async () => new Response(null, { status: 204 }));
    expect(
      await breakglassLogin(
        { name: "e2e", passphrase: "x".repeat(15), code: "000000" },
        accepted as unknown as typeof fetch,
      ),
    ).toBeNull();
    // A new session: the activity of the one before is forgotten.
    expect(localStorage.getItem(ACTIVITY_KEY)).toBeNull();
  });
});

describe("the logout (REQ-021-14)", () => {
  const OBJECT = {
    type: "prefix",
    id: "p1",
    value: "192.0.2.0/24",
    icon: "prefix" as const,
    to: "/prefixes/p1",
  };

  it("clears the recent objects of every user and what is sensitive, keeps the settings", () => {
    rememberObject("u1", OBJECT);
    rememberObject("u2", OBJECT);
    localStorage.setItem(LOCALE_KEY, "fr");
    localStorage.setItem(THEME_KEY, "dark");
    localStorage.setItem(MOTION_KEY, "reduce");
    localStorage.setItem(ISSUER_KEY, "dev");
    localStorage.setItem(ACTIVITY_KEY, "1");
    localStorage.setItem("kartenn.recent.searches", '["192.0.2.10"]');
    localStorage.setItem("other.app", "kept");
    sessionStorage.setItem(RETURN_KEY, "/sites");
    clearSensitive();
    expect(recentObjects("u1", () => true)).toEqual([]);
    expect(Object.keys(localStorage).filter((k) => k.startsWith(RECENT_KEY))).toEqual([]);
    expect(Object.keys(localStorage).sort()).toEqual(
      [LOCALE_KEY, THEME_KEY, MOTION_KEY, ISSUER_KEY, "other.app"].sort(),
    );
    expect(sessionStorage.length).toBe(0);
  });

  it("posts the logout as a top-level form, after the clearing", () => {
    rememberObject("u1", OBJECT);
    const sent: HTMLFormElement[] = [];
    logout((form) => {
      expect(recentObjects("u1", () => true)).toEqual([]);
      sent.push(form);
    });
    expect(sent).toHaveLength(1);
    expect(sent[0]!.method).toBe("post");
    expect(new URL(sent[0]!.action, location.href).pathname).toBe("/v1/auth/logout");
  });
});

describe("the end of the session, said before it comes (REQ-021-14, 022/A6)", () => {
  afterEach(() => vi.useRealTimers());

  function watch() {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-15T09:00:00Z"));
    const said: { end: number; extendable: boolean }[] = [];
    let cleared = 0;
    const expiry = createExpiry({
      warn: (end, extendable) => said.push({ end, extendable }),
      clear: () => cleared++,
    });
    return { expiry, said, cleared: () => cleared };
  }

  it("warns before the end, and the next activity pushes it back", () => {
    const { expiry, said, cleared } = watch();
    expiry.read("2026-01-15T09:30:00Z");
    vi.advanceTimersByTime(30 * 60_000 - WARN_BEFORE_MS - 1);
    expect(said).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(said).toEqual([{ end: Date.parse("2026-01-15T09:30:00Z"), extendable: true }]);
    // A request of the shell: the session is seen again, the warning goes.
    markActivity();
    expect(cleared()).toBe(1);
    vi.advanceTimersByTime(30 * 60_000 - WARN_BEFORE_MS - 1);
    expect(said).toHaveLength(1);
    expiry.stop();
  });

  it("follows the activity of another tab, kept in the browser", () => {
    const { expiry, said } = watch();
    expiry.read("2026-01-15T09:30:00Z");
    vi.advanceTimersByTime(20 * 60_000);
    localStorage.setItem(ACTIVITY_KEY, String(Date.now()));
    vi.advanceTimersByTime(8 * 60_000);
    // Expected at 09:28 from this tab alone; 09:48 with the other's activity at 09:20.
    expect(said).toEqual([]);
    vi.advanceTimersByTime(20 * 60_000 - 1);
    expect(said).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(said).toHaveLength(1);
    expiry.stop();
  });

  it("clears a warning said once another tab is active (its storage event)", () => {
    const { expiry, said, cleared } = watch();
    expiry.read("2026-01-15T09:30:00Z");
    vi.advanceTimersByTime(28 * 60_000);
    expect(said).toHaveLength(1);
    localStorage.setItem(ACTIVITY_KEY, String(Date.now()));
    window.dispatchEvent(new StorageEvent("storage", { key: ACTIVITY_KEY }));
    expect(cleared()).toBe(1);
    // Another key of the storage is no activity.
    vi.advanceTimersByTime(28 * 60_000);
    expect(said).toHaveLength(2);
    window.dispatchEvent(new StorageEvent("storage", { key: "kartenn.theme" }));
    expect(cleared()).toBe(1);
    expiry.stop();
  });

  it("says the absolute end two minutes before it, without the way to push it back (A14)", () => {
    const { expiry, said, cleared } = watch();
    // Idle until 09:30, at most until 09:40 whatever the activity.
    expiry.read("2026-01-15T09:30:00Z", "2026-01-15T09:40:00Z");
    vi.advanceTimersByTime(15 * 60_000);
    // Active at 09:15: the idle end would be 09:45, after the absolute one.
    markActivity();
    vi.advanceTimersByTime(23 * 60_000 - 1);
    expect(said).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(said).toEqual([{ end: Date.parse("2026-01-15T09:40:00Z"), extendable: false }]);
    // Still active: the end does not move, the warning stays as it was said.
    markActivity();
    expect(cleared()).toBe(0);
    expect(said).toHaveLength(1);
    expiry.stop();
  });

  it("says the idle end with the way to stay while it comes before the absolute one", () => {
    const { expiry, said, cleared } = watch();
    expiry.read("2026-01-15T09:30:00Z", "2026-01-15T09:30:30Z");
    vi.advanceTimersByTime(28 * 60_000);
    expect(said).toEqual([{ end: Date.parse("2026-01-15T09:30:00Z"), extendable: true }]);
    // Seen again at 09:28: the end is now the absolute one (the nearer of the two), fixed.
    expiry.read("2026-01-15T09:30:30Z", "2026-01-15T09:30:30Z");
    expect(cleared()).toBe(1);
    vi.advanceTimersByTime(30_000);
    expect(said.at(-1)).toEqual({ end: Date.parse("2026-01-15T09:30:30Z"), extendable: false });
    expiry.stop();
  });

  it("never expects the end later than a reading of the background says it", () => {
    const { expiry, said } = watch();
    expiry.read("2026-01-15T09:30:00Z", "2026-01-15T17:00:00Z");
    // At 09:20, a revalidation without a gesture: the server's end did not move (A14).
    vi.advanceTimersByTime(20 * 60_000);
    expiry.read("2026-01-15T09:30:00Z", "2026-01-15T17:00:00Z");
    vi.advanceTimersByTime(8 * 60_000);
    expect(said).toEqual([{ end: Date.parse("2026-01-15T09:30:00Z"), extendable: true }]);
    expiry.stop();
  });

  it("reads the end on the server's clock, said by the Date of its answers", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-15T09:00:00Z"));
    // The server is ten minutes ahead of the browser.
    const inner = vi.fn(
      async () => new Response("{}", { headers: { date: "Thu, 15 Jan 2026 09:10:00 GMT" } }),
    );
    await activityFetch(inner as unknown as typeof fetch)("/v1/me");
    expect(serverSkew()).toBe(10 * 60_000);
    const said: number[] = [];
    const expiry = createExpiry({ warn: (end) => said.push(end), clear: () => {} });
    expiry.read("2026-01-15T09:40:00Z");
    vi.advanceTimersByTime(28 * 60_000);
    // 09:40 on the server is 09:30 here: said at 09:28, by the browser's clock.
    expect(said).toEqual([Date.parse("2026-01-15T09:30:00Z")]);
    expiry.stop();
    await activityFetch(
      vi.fn(
        async () => new Response("{}", { headers: { date: new Date().toUTCString() } }),
      ) as unknown as typeof fetch,
    )("/v1/me");
  });

  it("says nothing without an end", () => {
    const { expiry, said } = watch();
    expiry.read(null);
    vi.advanceTimersByTime(24 * 3600_000);
    expect(said).toEqual([]);
    expiry.stop();
  });

  it("counts the answers of the API as activity, not the event stream nor a 401", async () => {
    expect(isActivity("/v1/prefixes")).toBe(true);
    expect(isActivity("/v1/events?after=1")).toBe(false);
    expect(isActivity("/plugins/notes/1.0.0/index.js")).toBe(false);
    const inner = vi.fn(async (input: RequestInfo | URL) =>
      String(input).endsWith("/me") ? new Response(null, { status: 401 }) : new Response("{}"),
    );
    const fetcher = activityFetch(inner as unknown as typeof fetch);
    markGesture();
    await fetcher("/v1/me");
    expect(localStorage.getItem(ACTIVITY_KEY)).toBeNull();
    await fetcher("/v1/prefixes");
    expect(Number(localStorage.getItem(ACTIVITY_KEY))).toBeGreaterThan(0);
  });

  it("counts only the reads the user asked for: a gesture just before, not the background", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-15T09:00:00Z"));
    localStorage.removeItem(ACTIVITY_KEY);
    const fetcher = activityFetch((async () => new Response("{}")) as unknown as typeof fetch);
    const target = new EventTarget();
    const stop = installGestures(target as unknown as Document);
    // A key pressed: the reads of the next seconds are the user's.
    target.dispatchEvent(new Event("keydown"));
    vi.advanceTimersByTime(GESTURE_WINDOW_MS - 1_000);
    await fetcher("/v1/prefixes");
    const asked = Number(localStorage.getItem(ACTIVITY_KEY));
    expect(asked).toBe(Date.now());
    // Long after: a read of the background (an event of the stream, a revalidation).
    vi.advanceTimersByTime(30 * 60_000);
    await fetcher("/v1/stats");
    expect(Number(localStorage.getItem(ACTIVITY_KEY))).toBe(asked);
    // A navigation is a gesture.
    markGesture();
    await fetcher("/v1/stats");
    expect(Number(localStorage.getItem(ACTIVITY_KEY))).toBe(Date.now());
    stop();
    target.dispatchEvent(new Event("pointerdown"));
    vi.advanceTimersByTime(GESTURE_WINDOW_MS + 1_000);
    await fetcher("/v1/prefixes");
    expect(Number(localStorage.getItem(ACTIVITY_KEY))).toBe(Date.now() - GESTURE_WINDOW_MS - 1_000);
    vi.useRealTimers();
  });

  describe("Kartenn-Background: ?1 on the reads of the background (A14)", () => {
    /** The header each request left with, `null` without it. */
    function sent() {
      const headers: (string | null)[] = [];
      const inner = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const from = new Headers(init?.headers ?? (input instanceof Request ? input.headers : {}));
        headers.push(from.get("Kartenn-Background"));
        return new Response("{}");
      });
      return { fetcher: activityFetch(inner as unknown as typeof fetch), headers };
    }

    beforeEach(() => {
      vi.useFakeTimers();
      vi.setSystemTime(new Date("2026-01-15T09:00:00Z"));
    });
    afterEach(() => vi.useRealTimers());

    it("never marks a read born of a gesture, which stays activity", async () => {
      const { fetcher, headers } = sent();
      const target = new EventTarget();
      const stop = installGestures(target as unknown as Document);
      for (const gesture of ["keydown", "pointerdown", "wheel", "touchstart"]) {
        vi.advanceTimersByTime(GESTURE_WINDOW_MS * 2);
        target.dispatchEvent(new Event(gesture));
        vi.advanceTimersByTime(GESTURE_WINDOW_MS);
        await fetcher("/v1/prefixes", { headers: { Accept: "application/json" } });
      }
      markGesture();
      await fetcher(new Request("http://localhost/v1/sites"));
      expect(headers).toEqual([null, null, null, null, null]);
      expect(Number(localStorage.getItem(ACTIVITY_KEY))).toBe(Date.now());
      stop();
    });

    it("marks a read nobody asked for, which is no activity, and keeps its headers", async () => {
      const { fetcher, headers } = sent();
      markGesture();
      vi.advanceTimersByTime(GESTURE_WINDOW_MS + 1);
      // An event of the stream, a revalidation on focus or on reconnection, a prefetch.
      await fetcher("/v1/stats", { headers: { Accept: "application/json" } });
      await fetcher(
        new Request("http://localhost/v1/sites", { headers: { "If-None-Match": "x" } }),
      );
      await fetcher("/v1/domains", { method: "QUERY", body: "{}" });
      expect(headers).toEqual(["?1", "?1", "?1"]);
      expect(localStorage.getItem(ACTIVITY_KEY)).toBeNull();
      const kept = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
        const from = new Headers(init?.headers);
        return new Response(JSON.stringify([from.get("Accept"), from.get("If-None-Match")]));
      });
      const answer = await activityFetch(kept as unknown as typeof fetch)(
        new Request("http://localhost/v1/sites", { headers: { "If-None-Match": "x" } }),
        { headers: { Accept: "application/json", "If-None-Match": "y" } },
      );
      expect(await answer.json()).toEqual(["application/json", "y"]);
    });

    it("marks a read asked by a gesture once it is read again in the background", async () => {
      const { fetcher, headers } = sent();
      markGesture();
      await fetcher("/v1/prefixes");
      // The same query, revalidated on focus a minute later, then invalidated by an event.
      vi.advanceTimersByTime(60_000);
      await fetcher("/v1/prefixes");
      vi.advanceTimersByTime(60_000);
      await fetcher("/v1/prefixes");
      // Read again just after a gesture: the user's again.
      markGesture();
      await fetcher("/v1/prefixes");
      expect(headers).toEqual([null, "?1", "?1", null]);
    });

    it("never marks a write, nor what is not the API", async () => {
      const { fetcher, headers } = sent();
      vi.advanceTimersByTime(GESTURE_WINDOW_MS * 10);
      await fetcher("/v1/prefixes", { method: "POST", body: "{}" });
      expect(Number(localStorage.getItem(ACTIVITY_KEY))).toBe(Date.now());
      await fetcher("/v1/drafts/d1", { method: "PATCH", body: "{}" });
      await fetcher("/plugins/notes/1.0.0/index.js");
      await fetcher("/v1/events?after=1");
      expect(headers).toEqual([null, null, null, null]);
    });
  });

  it("dates an activity from its request, not from an answer that came late (task 029-5)", async () => {
    const { expiry, said, cleared } = watch();
    expiry.read("2026-01-15T09:30:00Z");
    // A read asked at 09:00 whose answer comes 27 minutes later (a device suspended, a
    // clock moved on meanwhile): the server saw it at 09:00 at the latest.
    let answer: (response: Response) => void = () => {};
    const fetcher = activityFetch(
      (() => new Promise<Response>((done) => (answer = done))) as unknown as typeof fetch,
    );
    markGesture();
    const read = fetcher("/v1/domains");
    vi.advanceTimersByTime(27 * 60_000);
    answer(new Response("{}"));
    await read;
    expect(Number(localStorage.getItem(ACTIVITY_KEY))).toBe(Date.parse("2026-01-15T09:00:00Z"));
    // The end expected stays 09:30: said at 09:28, not pushed back by the late answer.
    vi.advanceTimersByTime(60_000);
    expect(said).toEqual([{ end: Date.parse("2026-01-15T09:30:00Z"), extendable: true }]);
    expect(cleared()).toBe(0);
    expiry.stop();
  });

  it("says the warning once for an end moved by less than its minute", () => {
    const { expiry, said, cleared } = watch();
    expiry.read("2026-01-15T09:30:00Z");
    vi.advanceTimersByTime(29 * 60_000);
    expect(said).toHaveLength(1);
    // A read asked just after the reading, answered during the warning: the end moves by
    // half a second, the time written does not.
    markActivity(Date.parse("2026-01-15T09:00:00.500Z"));
    expect(said).toHaveLength(1);
    expect(cleared()).toBe(0);
    expiry.stop();
  });

  it("brings back to now an activity kept in the future (the clock set back)", () => {
    const { expiry, said } = watch();
    // Kept at 10:00 before the clock went back to 09:00 (by hand, by NTP).
    localStorage.setItem(ACTIVITY_KEY, String(Date.parse("2026-01-15T10:00:00Z")));
    expiry.read("2026-01-15T09:30:00Z");
    expect(Number(localStorage.getItem(ACTIVITY_KEY))).toBe(Date.now());
    // Said at 09:28, not at 10:28.
    vi.advanceTimersByTime(28 * 60_000);
    expect(said).toEqual([{ end: Date.parse("2026-01-15T09:30:00Z"), extendable: true }]);
    // The activities of the tabs replace it again.
    markActivity();
    expect(Number(localStorage.getItem(ACTIVITY_KEY))).toBe(Date.now());
    expiry.stop();
  });

  it("keeps the newer activity of another tab over an older one of this tab", () => {
    const { expiry } = watch();
    localStorage.setItem(ACTIVITY_KEY, String(Date.now()));
    markActivity(Date.now() - 60_000);
    expect(Number(localStorage.getItem(ACTIVITY_KEY))).toBe(Date.now());
    expiry.stop();
  });
});

describe("« Rester connecté » in the frame (watchSessionEnd)", () => {
  const END = "2026-01-15T09:30:00Z";
  const ABSOLUTE = "2026-01-15T17:00:00Z";
  afterEach(() => {
    vi.useRealTimers();
    expired.value = false;
  });

  /** The frame's wiring, on a store of toasts and an end of the session as `/v1/me` gave it. */
  function frame(reading: () => Promise<EndReading>, absolute = ABSOLUTE) {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-15T09:00:00Z"));
    const toasts = createToasts();
    const end = ref<string | null>(END);
    const absoluteEnd = ref<string | null>(absolute);
    const read = vi.fn(reading);
    const stop = watchSessionEnd({
      toasts,
      t: (key, params) => (params && "why" in params ? `${key}:${String(params.why)}` : key),
      sentence: (error) => (error instanceof ApiProblem ? `said:${error.code}` : "said:network"),
      locale: () => "fr",
      end,
      absoluteEnd,
      readEnd: read,
    });
    vi.advanceTimersByTime(28 * 60_000);
    const warning = () => toasts.list.find((toast) => toast.title?.startsWith("session.ending"));
    /** Plays « Rester connecté » as the region does: the action, then the toast dismissed. */
    async function stay() {
      const shown = warning()!;
      shown.action!.run();
      toasts.dismiss(shown.id);
      await vi.waitFor(() => expect(read).toHaveBeenCalled());
      await Promise.resolve();
    }
    return { toasts, warning, stay, stop };
  }

  it("says the absolute end without the action, which would not push it back", async () => {
    const { warning, stop } = frame(
      async () => ({ kind: "end", expiresAt: END, absoluteAt: END }),
      END,
    );
    expect(warning()).toMatchObject({
      tone: "warn",
      title: "session.endingAbsolute",
      description: "session.endingFixed",
    });
    expect(warning()!.action).toBeUndefined();
    stop();
  });

  it("says the absolute end again, without the action, once staying reaches it", async () => {
    const LAST = "2026-01-15T09:30:30Z";
    const { warning, stay, stop } = frame(
      async () => ({ kind: "end", expiresAt: LAST, absoluteAt: LAST }),
      LAST,
    );
    expect(warning()).toMatchObject({ title: "session.ending", description: "session.endingText" });
    expect(warning()?.action?.label).toBe("session.stay");
    await stay();
    // Pushed back to the absolute end only: said again two minutes before it.
    expect(warning()).toBeUndefined();
    vi.advanceTimersByTime(30_000);
    await vi.waitFor(() => expect(warning()).toBeDefined());
    expect(warning()).toMatchObject({
      title: "session.endingAbsolute",
      description: "session.endingFixed",
    });
    expect(warning()!.action).toBeUndefined();
    stop();
  });

  it("clears the warning when the end moved", async () => {
    const { warning, stay, stop } = frame(async () => ({
      kind: "end",
      expiresAt: "2026-01-15T09:58:00Z",
      absoluteAt: ABSOLUTE,
    }));
    await stay();
    expect(warning()).toBeUndefined();
    stop();
  });

  it("opens « Session expirée » on a 401, and keeps the session on another failure", async () => {
    const gone = frame(async () => ({ kind: "expired" }));
    await gone.stay();
    expect(expired.value).toBe(true);
    gone.stop();
    expired.value = false;

    const down = frame(async () => ({ kind: "failed", problem: null }));
    await down.stay();
    expect(expired.value).toBe(false);
    const failed = () => down.toasts.list.find((toast) => toast.title === "session.stayFailed");
    // No answer: the network said, no code to pass on.
    expect(failed()).toMatchObject({ description: "session.stayFailedText:said:network" });
    expect(failed()?.code).toBeUndefined();
    down.stop();

    // An answer: by problems/, the sentence of its type, its type and Request-Id.
    const busy = frame(async () => ({
      kind: "failed",
      problem: new ApiProblem({
        status: 503,
        type: "urn:kartenn:problem:unavailable",
        title: "",
        requestId: "r-9",
      }),
    }));
    await busy.stay();
    expect(busy.toasts.list.find((toast) => toast.title === "session.stayFailed")).toMatchObject({
      tone: "ko",
      description: "session.stayFailedText:said:unavailable",
      code: "urn:kartenn:problem:unavailable",
      requestId: "r-9",
    });
    busy.stop();
  });

  it("reads the end outside the SDK's client: no activity that would clear the warning", async () => {
    const answer = vi.fn<typeof fetch>(
      async () =>
        new Response(
          JSON.stringify({ session: { expires_at: END, absolute_expires_at: ABSOLUTE } }),
          { headers: { "content-type": "application/json" } },
        ),
    );
    expect(await readEnd(answer)).toEqual({
      kind: "end",
      expiresAt: END,
      absoluteAt: ABSOLUTE,
    });
    expect(localStorage.getItem(ACTIVITY_KEY)).toBeNull();
    // A gesture (« Rester connecté »): the server sees the session again, no header.
    expect(new Headers(answer.mock.calls[0]?.[1]?.headers).has("Kartenn-Background")).toBe(false);
    const refused = vi.fn(async () => new Response(null, { status: 401 }));
    expect(await readEnd(refused as unknown as typeof fetch)).toEqual({ kind: "expired" });
  });
});
