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
    loginAt("dev", "/x/flow/flows", (url) => went.push(url));
    expect(went).toEqual(["/v1/auth/login?issuer=dev"]);
    expect(lastIssuer()).toBe("dev");
    expect(sessionStorage.getItem(RETURN_KEY)).toBe("/x/flow/flows");
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
    const accepted = vi.fn(async () => new Response(null, { status: 204 }));
    expect(
      await breakglassLogin(
        { name: "e2e", passphrase: "x".repeat(15), code: "000000" },
        accepted as unknown as typeof fetch,
      ),
    ).toBeNull();
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

  it("says a fixed end without the way to push it back", () => {
    const { expiry, said } = watch();
    expiry.read("2026-01-15T09:10:00Z");
    vi.advanceTimersByTime(9 * 60_000);
    expect(said.at(-1)?.extendable).toBe(true);
    // « Rester connecté » read /v1/me again: the end did not move.
    expiry.read("2026-01-15T09:10:00Z");
    expect(said.at(-1)).toEqual({ end: Date.parse("2026-01-15T09:10:00Z"), extendable: false });
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
  afterEach(() => {
    vi.useRealTimers();
    expired.value = false;
  });

  /** The frame's wiring, on a store of toasts and an end of the session as `/v1/me` gave it. */
  function frame(reading: () => Promise<EndReading>) {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-01-15T09:00:00Z"));
    const toasts = createToasts();
    const end = ref<string | null>(END);
    const read = vi.fn(reading);
    const stop = watchSessionEnd({
      toasts,
      t: (key, params) => (params && "why" in params ? `${key}:${String(params.why)}` : key),
      sentence: (error) => (error instanceof ApiProblem ? `said:${error.code}` : "said:network"),
      locale: () => "fr",
      end,
      readEnd: read,
    });
    vi.advanceTimersByTime(28 * 60_000);
    const warning = () => toasts.list.find((toast) => toast.title === "session.ending");
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

  it("says a fixed end again, without the action, when the end did not move", async () => {
    const { warning, stay, stop } = frame(async () => ({ kind: "end", expiresAt: END }));
    expect(warning()?.action?.label).toBe("session.stay");
    await stay();
    await vi.waitFor(() => expect(warning()).toBeDefined());
    expect(warning()!.description).toBe("session.endingFixed");
    expect(warning()!.action).toBeUndefined();
    stop();
  });

  it("clears the warning when the end moved", async () => {
    const { warning, stay, stop } = frame(async () => ({
      kind: "end",
      expiresAt: "2026-01-15T09:58:00Z",
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
    const answer = vi.fn(
      async () =>
        new Response(JSON.stringify({ session: { expires_at: END } }), {
          headers: { "content-type": "application/json" },
        }),
    );
    expect(await readEnd(answer as unknown as typeof fetch)).toEqual({
      kind: "end",
      expiresAt: END,
    });
    expect(localStorage.getItem(ACTIVITY_KEY)).toBeNull();
    const refused = vi.fn(async () => new Response(null, { status: 401 }));
    expect(await readEnd(refused as unknown as typeof fetch)).toEqual({ kind: "expired" });
  });
});
