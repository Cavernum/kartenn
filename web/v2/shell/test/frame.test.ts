// @vitest-environment happy-dom
import { ApiProblem, queryClient, reads } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView, type RouteRecordRaw } from "vue-router";
import { crumbsOf } from "../src/frame/breadcrumb.ts";
import { frameRoutes } from "../src/frame/routes.ts";
import { installSession } from "../src/frame/session.ts";
import ShellFrame from "../src/frame/ShellFrame.vue";
import {
  forgetSettings,
  keepPanel,
  PANEL_KEY,
  readPanel,
  SIDEBAR_KEY,
  THEME_KEY,
  useSettings,
} from "../src/frame/settings.ts";
import { CORE_SPACES, locate, sidebarGroups, spaceEntry } from "../src/frame/spaces.ts";
import { installPrefetch, queriesOf } from "../src/prefetch/intent.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, Rights, type Me } from "../src/rights/index.ts";
import {
  checkPeriod,
  goToPeriod,
  installTime,
  probePath,
  refusalOnField,
} from "../src/time/index.ts";

// The frame of the shell (REQ-021-1, 2, 4, 5): spaces by rights, breadcrumb, guards of
// session, time and prefetch in their order, the check of a period by the API, the
// settings kept.

const t = (key: string) => key;
const Empty = { render: () => null };
const READER = new Rights([
  "core.entity.read",
  "core.site.read",
  "core.domain.read",
  "core.prefix.read",
  "core.range.read",
  "core.address.read",
  "core.vlan_group.read",
  "core.vlan.read",
]);

describe("spaces of the sidebar", () => {
  it("shows a reader the referential and the network, in their order, not the admin", () => {
    const groups = sidebarGroups(READER, t);
    expect(groups.map((g) => g.id)).toEqual(["referential", "network"]);
    expect(groups[0]!.entries.map((e) => e.id)).toContain("prefixes");
    expect(groups[0]!.entries[0]).toMatchObject({ label: "nav.home", to: "/" });
  });

  it("hides every space of the core from whom holds none of its rights", () => {
    // A flow requester: the spaces of Flux and Politique come from the plugins (task 16).
    expect(sidebarGroups(new Rights(["flow.request.edit"]), t)).toEqual([]);
  });

  it("shows the administration and the changesets to an administrator", () => {
    const ids = sidebarGroups(new Rights(["*"]), t).map((g) => g.id);
    expect(ids).toEqual(["referential", "changes", "network", "admin"]);
  });

  it("gives the drafts to an editor of a property or an administrator of drafts", () => {
    for (const entry of ["prop.core.owner.edit", "prop.*.edit", "core.draft.admin"]) {
      const changes = sidebarGroups(new Rights([entry]), t).find((g) => g.id === "changes");
      expect(
        changes?.entries.map((e) => e.id),
        entry,
      ).toEqual(["drafts", "imports"]);
    }
    expect(sidebarGroups(new Rights(["prop.core.owner.read"]), t)).toEqual([]);
  });

  it("shows roles to a role administrator, identities to an account administrator", () => {
    const ids = (entry: string) =>
      sidebarGroups(new Rights([entry]), t)
        .find((g) => g.id === "admin")
        ?.entries.map((e) => e.id);
    expect(ids("core.role.admin")).toEqual(["roles"]);
    expect(ids("core.account.admin")).toEqual(["identities", "accounts"]);
  });

  it("leads a space to its first entry the user may see", () => {
    const admin = CORE_SPACES.find((space) => space.id === "admin")!;
    expect(spaceEntry(admin, new Rights(["core.audit.read"])).to).toBe("/admin/audit");
    expect(spaceEntry(admin, new Rights([])).to).toBe("/admin/groups");
  });

  it("finds the space and entry of a path, the longest entry first", () => {
    expect(locate("/")?.entry.id).toBe("home");
    expect(locate("/prefixes/0b5e")?.entry.id).toBe("prefixes");
    expect(locate("/vlan-groups")?.entry.id).toBe("vlan-groups");
    expect(locate("/admin/roles")?.space.id).toBe("admin");
    expect(locate("/nowhere")).toBeNull();
  });
});

describe("breadcrumb", () => {
  it("links every crumb but the last, from the routes of the frame", async () => {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: Empty, children: frameRoutes }],
    });
    await router.push("/addresses");
    expect(crumbsOf(router.currentRoute.value, t)).toEqual([
      { label: "space.referential", to: "/" },
      { label: "nav.addresses" },
    ]);
    await router.push("/admin");
    expect(router.currentRoute.value.path).toBe("/admin/groups");
    await router.push("/nowhere/at/all");
    expect(crumbsOf(router.currentRoute.value, t)).toEqual([{ label: "app.notFound" }]);
  });
});

describe("guards of the frame", () => {
  const VALID_AT = "2026-01-01T00:00:00Z";
  const AS_OF = "2026-06-30T12:00:00Z";
  afterEach(() => resetContext());

  const signedOut = async () => ({ me: null });
  // Signed in: a page that needs a session shows (signed out, the login page, task 16).
  const signedIn = async () => ({ me: { identity: "0b5e3c1a-0000-4000-8000-000000000001" } as Me });

  function frameRouter(
    guarded: unknown[][],
    load: () => Promise<{ me: null | Me }>,
    start: () => unknown = () => {},
  ) {
    const routes: RouteRecordRaw[] = [
      {
        path: "/",
        component: Empty,
        meta: { frame: true },
        children: [
          { path: "", component: Empty },
          {
            path: "prefixes/:id",
            component: Empty,
            meta: {
              queries: (params, time) => [reads.object("core.prefix", String(params.id), time)],
            },
          },
        ],
      },
      { path: "/_ui", component: Empty },
    ];
    const router = createRouter({ history: createMemoryHistory(), routes });
    installSession(router, load, start);
    installTime(router);
    installPrefetch(router, async (queries) => {
      guarded.push(queries.map((q) => q.queryKey));
      return "ready";
    });
    return router;
  }

  it("carries the dates into the target before the prefetch reads it", async () => {
    const guarded: unknown[][] = [];
    const router = frameRouter(guarded, signedIn);
    await router.push(`/?valid_at=${VALID_AT}&as_of=${AS_OF}`);
    await router.push("/prefixes/p1");
    const to = router.currentRoute.value;
    expect(to.query).toMatchObject({ valid_at: VALID_AT, as_of: AS_OF });
    // The read the guard waited for is the dated one.
    expect(guarded).toEqual([queriesOf(to, false).map((q) => q.queryKey)]);
    expect(JSON.stringify(guarded)).toContain(AS_OF);
  });

  it("goes back to the present without carrying the dates", async () => {
    const router = frameRouter([], signedIn);
    await router.push(`/prefixes/p1?valid_at=${VALID_AT}`);
    await goToPeriod(router, { validAt: null, asOf: null });
    expect(router.currentRoute.value.query).toEqual({});
    await router.push("/");
    expect(router.currentRoute.value.query).toEqual({});
  });

  it("carries no date on a move in the history", async () => {
    const router = frameRouter([], signedIn);
    await router.push("/prefixes/p1");
    await router.push(`/prefixes/p1?valid_at=${VALID_AT}`);
    // Back, to the page as it was: at the present.
    router.back();
    await vi.waitFor(() => expect(router.currentRoute.value.query).toEqual({}));
    expect(router.currentRoute.value.path).toBe("/prefixes/p1");
    // A link followed afterwards from that page carries nothing either.
    await router.push("/");
    expect(router.currentRoute.value.query).toEqual({});
  });

  it("reads the session once, before the first page of the frame, never for /_ui", async () => {
    let loads = 0;
    let starts = 0;
    const me = { identity: "0b5e3c1a-0000-4000-8000-000000000009" } as Me;
    const router = frameRouter(
      [],
      async () => {
        loads++;
        return { me };
      },
      () => starts++,
    );
    await router.push("/_ui");
    expect(loads).toBe(0);
    await router.push("/");
    await router.push("/prefixes/p1");
    expect(loads).toBe(1);
    // Signed in: the data layer starts once, with the first page.
    expect(starts).toBe(1);
  });

  it("starts no data layer without a session", async () => {
    let starts = 0;
    const router = frameRouter([], signedOut, () => starts++);
    await router.push("/");
    expect(starts).toBe(0);
  });
});

describe("check of a period by the API", () => {
  it("asks nothing at the present, nor without a dated list the user may read", async () => {
    const asked: string[] = [];
    const get = async (u: string) => asked.push(u);
    expect(await checkPeriod({ validAt: null, asOf: null }, "/v1/domains", get)).toBeNull();
    expect(await checkPeriod({ validAt: null, asOf: "2026-01-01T00:00:00Z" }, null, get)).toBe(
      null,
    );
    expect(asked).toEqual([]);
  });

  it("asks the lightest dated list the user may read", () => {
    expect(probePath((p) => READER.has(p))).toBe("/v1/domains");
    expect(probePath((p) => p === "core.address.read")).toBe("/v1/addresses");
    expect(probePath(() => false)).toBeNull();
  });

  it("brings a 400 on as_of back on its field, lets a 403 through", async () => {
    const asked: string[] = [];
    const refuse = (status: number) => async (url: string) => {
      asked.push(url);
      throw new ApiProblem({
        status,
        type: "about:blank",
        title: "Bad Request",
        detail: "as_of is later than the time of kartenn",
      });
    };
    const asOf = { validAt: null, asOf: "2026-01-01T00:00:00Z" };
    const refused = await checkPeriod(asOf, "/v1/sites", refuse(400));
    expect(refused).toMatchObject({ field: "asOf", problem: { status: 400 } });
    expect(asked[0]).toBe("/v1/sites?page_size=1&as_of=2026-01-01T00%3A00%3A00Z");
    const validAt = { validAt: "2026-01-01T00:00:00Z", asOf: null };
    expect(await checkPeriod(validAt, "/v1/sites", refuse(403))).toBeNull();
  });

  it("says a refused date on its field by problems/: its own words, else its type's", () => {
    const tr = (key: string, params?: Record<string, unknown>) =>
      `${key}${params ? JSON.stringify(params) : ""}`;
    const sentence = (error: unknown) =>
      error instanceof ApiProblem ? `said:${error.code}` : "said:network";
    const later = new ApiProblem({
      status: 400,
      type: "urn:kartenn:problem:bad-request",
      title: "",
      detail: "as_of is later than the time of kartenn",
      requestId: "r-1",
    });
    expect(refusalOnField({ field: "asOf", problem: later }, tr, sentence)).toEqual({
      asOf: "time.asOfLater",
    });
    const malformed = new ApiProblem({
      status: 400,
      type: "urn:kartenn:problem:bad-request",
      title: "",
      detail: "valid_at is not RFC 3339",
    });
    const words = refusalOnField({ field: "validAt", problem: malformed }, tr, sentence);
    expect(words).toEqual({ validAt: 'time.refused{"why":"said:bad-request"}' });
    // A new object each time: the date pill shows each refusal again.
    expect(refusalOnField({ field: "validAt", problem: malformed }, tr, sentence)).not.toBe(words);
  });
});

describe("settings of the user", () => {
  beforeEach(() => {
    localStorage.clear();
    forgetSettings();
  });

  it("applies the kept theme and keeps a change", async () => {
    localStorage.setItem(THEME_KEY, "dark");
    const settings = useSettings();
    expect(document.documentElement.dataset.theme).toBe("dark");
    settings.theme.value = "system";
    settings.sidebarCollapsed.value = true;
    await Promise.resolve();
    expect(document.documentElement.dataset.theme).toBeUndefined();
    expect(localStorage.getItem(THEME_KEY)).toBe("system");
    expect(localStorage.getItem(SIDEBAR_KEY)).toBe("collapsed");
    forgetSettings();
    expect(useSettings().sidebarCollapsed.value).toBe(true);
  });

  it("reads the panel kept, and ignores what is not one", () => {
    expect(readPanel()).toEqual({ open: false, tab: "detail", width: null });
    keepPanel({ open: true, tab: "draft", width: 480 });
    expect(readPanel()).toEqual({ open: true, tab: "draft", width: 480 });
    localStorage.setItem(PANEL_KEY, "{");
    expect(readPanel().open).toBe(false);
  });
});

// Out of a session (task 16i), the banner and the surface alone: no sidebar (nor its
// drawer, nor its button), date pill, band, draft counter or right panel mounted.
describe("the frame out of a session", () => {
  let app: App | undefined;
  const ME = {
    identity: "0b5e3c1a-0000-4000-8000-000000000009",
    kind: "human",
    bindings: [],
    global_permissions: [],
  } as unknown as Me;
  /** The parts signed in only; the right panel open, or its rail closed. */
  const PARTS = [
    ".k-sidebar",
    ".k-date-pill",
    ".k-time-band",
    ".k-draft-counter",
    ".k-side-panel, .k-side-panel-rail",
  ];

  beforeEach(() => {
    setFetch(
      (async () =>
        new Response(JSON.stringify({ items: [], links: {} }), {
          headers: { "Content-Type": "application/json" },
        })) as typeof fetch,
    );
  });
  afterEach(async () => {
    app?.unmount();
    app = undefined;
    document.body.replaceChildren();
    queryClient.clear();
    await loadSession(async () => null);
    resetContext();
  });

  /** The frame mounted at `path`, its pages empty. */
  async function frame(path: string): Promise<HTMLElement> {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        {
          path: "/",
          component: ShellFrame,
          children: [
            { path: "", component: Empty },
            { path: "login", component: Empty },
            { path: "about", component: Empty },
          ],
        },
      ],
    });
    await router.push(path);
    // In the document: the layers of the frame are its own element, found by id.
    const root = document.body.appendChild(document.createElement("div"));
    app = createApp({ render: () => h(RouterView) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    await nextTick();
    return root;
  }

  for (const path of ["/login", "/about"]) {
    it(`mounts at ${path} the banner, its user menu and the surface alone`, async () => {
      const root = await frame(path);
      expect(root.querySelector("header.k-frame__head")).not.toBeNull();
      expect(root.querySelector(".k-frame__bar-end [aria-haspopup='menu']")).not.toBeNull();
      expect(root.querySelector("main#k-surface")).not.toBeNull();
      for (const part of [...PARTS, ".k-frame__nav", ".k-drawer"]) {
        expect(root.querySelector(part), part).toBeNull();
      }
      expect(root.querySelector(".k-frame")?.getAttribute("data-sidebar")).toBe("none");
    });
  }

  it("mounts the whole frame once signed in", async () => {
    await loadSession(async (url) => (url === "/v1/me" ? ME : null));
    const root = await frame("/about");
    for (const part of PARTS) expect(root.querySelector(part), part).not.toBeNull();
    expect(root.querySelector(".k-frame")?.getAttribute("data-sidebar")).toBe("expanded");
  });
});
