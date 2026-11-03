// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { drafts, queryClient, type components, type Extension, type User } from "@kartenn/sdk";
import { resetContext } from "@kartenn/sdk/host";
import { announcePlugin, fakeFetch, mount, type Mounted } from "@kartenn/sdk/testing";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import en from "../public/i18n/en.json";
import fr from "../public/i18n/fr.json";
import manifest from "../public/manifest.json";

// The view « Hébergement et clusters » in Vue (spec 021, task 18cd): a `TreeGrid` whose
// parent is declared — the clusters and the machines without a host, their guests read once
// opened; a machine moved by Ctrl+X then Ctrl+V, each target judged by `canMove`, to the
// draft by `draft.move`, never optimistic; and the record of a cluster. Mounted in the
// sandbox of the SDK, in the light DOM of its element, with a simulated `fetch`.

const ENTITY = "0b7d1c8e-3a54-4b1c-8f0e-2d6a9c4e5f21";
const SITE = "0b7d1c8e-3a54-4b1c-8f0e-2d6a9c4e5f22";
const CLUSTER = "5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8b23";
const HOST = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
const GUEST = "5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8b21";
const AWAY = "5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8b22";
const DRAFT = "1b5e4fc3-4071-4f61-9e82-3d2a1f0e9b86";

type Machine = components["schemas"]["Machine"];

const machine = (id: string, name: string, kind: string, host?: Machine["host"]): Machine => ({
  id,
  href: `/v1/machines/${id}`,
  name,
  kind,
  owner: { id: ENTITY },
  site: { id: SITE },
  ...(host ? { host } : {}),
});
const cluster = {
  id: CLUSTER,
  href: `/v1/clusters/${CLUSTER}`,
  slug: "grappe",
  owner: { id: ENTITY },
};
const HOSTED = machine(GUEST, "vm-a", "virtual", { machine: { id: HOST } });
const MACHINES = [machine(HOST, "srv-a", "physical"), HOSTED, machine(AWAY, "vm-b", "virtual")];
const page = <T>(items: T[], next?: string) => ({ items, links: next ? { next } : {} }) as never;

/** A user who holds every permission, or none of `missing`. */
const user = (missing: string[] = []): User => ({
  identity: "alice",
  name: "Alice",
  kind: "user",
  has: (permission) => !missing.includes(permission),
});

const problem = (status: number, slug: string) =>
  new Response(JSON.stringify({ type: `urn:kartenn:problem:${slug}`, title: "Refused", status }), {
    status,
    headers: { "Content-Type": "application/problem+json", "Request-Id": "req-42" },
  });

/** A draft of the server holding `operations`. */
const draftOf = (operations: unknown[]) =>
  new Response(
    JSON.stringify({
      id: DRAFT,
      href: `/v1/drafts/${DRAFT}`,
      author: DRAFT,
      title: "t",
      operation_count: operations.length,
      version: 1,
      created_at: "2026-10-24T00:00:00Z",
      updated_at: "2026-10-24T00:00:00Z",
      reason: {},
      operations,
    }),
    { status: 201, headers: { "Content-Type": "application/json", ETag: '"1"' } },
  );
const simulation = (valid: boolean, violations: unknown[]) =>
  ({ valid, violations, diff: [], sample: [], cascaded: [], sections: [] }) as never;

/**
 * The reads of the tree: the guests of a host by `?host=`, the machines without one by
 * `?host:exists=false` (A28); any other list of machines holds the hosted ones too.
 */
const READS = {
  "GET /v1/clusters": () => page([cluster]),
  "GET /v1/machines": ({ query }: { query: URLSearchParams }) => {
    const host = query.get("host");
    if (host === HOST) return page([HOSTED]);
    if (host) return page([]);
    return query.get("host:exists") === "false"
      ? page(MACHINES.filter((m) => !m.host))
      : page(MACHINES);
  },
  "GET /v1/sites/{id}": () => ({ id: SITE, slug: "nord", href: `/v1/sites/${SITE}` }) as never,
};

let mounted: Mounted<HTMLElement> | null = null;
/** The links the view followed (the shell's router would have). */
let followed: string[] = [];
const follow = (event: MouseEvent) => {
  const anchor = event.target instanceof Element ? event.target.closest("a") : null;
  if (!anchor) return;
  event.preventDefault();
  followed.push(anchor.getAttribute("href") ?? "");
};

beforeAll(async () => {
  const defined = announcePlugin({
    name: manifest.name,
    extensions: manifest.extensions as Extension[],
    catalogs: { en, fr },
  });
  await import("../src/index.ts");
  await defined;
  document.addEventListener("click", follow);
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
  followed = [];
  queryClient.clear();
  resetContext();
  sessionStorage.clear();
  history.replaceState(null, "", "/");
  vi.restoreAllMocks();
});

/** The text of the element (light DOM), once it holds `expected`. */
async function text(element: HTMLElement, expected: string | RegExp): Promise<string> {
  let found = "";
  await vi.waitFor(() => {
    found = element.textContent ?? "";
    expect(found).toMatch(expected);
  });
  return found;
}

const rowOf = (element: HTMLElement, id: string) =>
  element.querySelector<HTMLElement>(`[role=row][data-node="${id}"]`);
/** The labels of the rows of the tree, with their level. */
const rows = (element: HTMLElement) =>
  [...element.querySelectorAll<HTMLElement>("[role=treegrid] [role=row][data-node]")].map(
    (row) =>
      `${row.getAttribute("aria-level")} ${row.querySelector(".k-tree-grid__label")?.textContent?.trim()}`,
  );
/** A key pressed on the node cell of the row `id`. */
function press(element: HTMLElement, id: string, key: string, ctrlKey = false): void {
  const cell = element.querySelector<HTMLElement>(`[data-row="${id}"][data-column=""]`)!;
  cell.dispatchEvent(new KeyboardEvent("keydown", { key, ctrlKey, bubbles: true }));
}
/** The status region of the view (not the tree's). */
const status = (element: HTMLElement) =>
  [...element.querySelectorAll("[role=status]")].find(
    (r) => !r.closest("[role=treegrid]") && !r.closest(".k-tree-grid"),
  )!;

describe("the tree « Hébergement et clusters » (task 18cd)", () => {
  it("shows the clusters and the machines without a host, the guests once opened; Enter opens a record", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    const asked: string[] = [];
    const ROOTS = "host:exists=false";
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...READS,
        "GET /v1/machines": (request) => {
          const { query } = request;
          asked.push(query.get("host") ?? `host:exists=${query.get("host:exists")}`);
          return READS["GET /v1/machines"](request);
        },
      }),
    });
    const { element } = mounted;
    await text(element, "vm-b");
    expect(element.querySelector("h1")?.textContent).toBe("Hosting and clusters");
    expect(rows(element)).toEqual(["1 grappe", "1 srv-a", "1 vm-b"]);
    expect(element.querySelector("[role=treegrid]")?.getAttribute("aria-label")).toBe(
      "Hosting and clusters",
    );
    await vi.waitFor(() => expect(rowOf(element, HOST)?.textContent).toContain("nord"));
    // Opened: its guests, read by `?host=`.
    press(element, HOST, "ArrowRight");
    await vi.waitFor(() =>
      expect(rows(element)).toEqual(["1 grappe", "1 srv-a", "2 vm-a", "1 vm-b"]),
    );
    expect(asked).toEqual([ROOTS, HOST]);
    // A cluster opened without guests: a leaf.
    press(element, CLUSTER, "ArrowRight");
    await vi.waitFor(() => expect(asked).toEqual([ROOTS, HOST, CLUSTER]));
    await vi.waitFor(() =>
      expect(rowOf(element, CLUSTER)?.hasAttribute("aria-expanded")).toBe(false),
    );
    press(element, CLUSTER, "Enter");
    press(element, GUEST, "Enter");
    expect(followed).toEqual([`/x/machines/clusters/${CLUSTER}`, `/x/machines/machines/${GUEST}`]);
  });

  it("reads the machines without a host a page at a time, the next one when the tree asks (A28)", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    const asked: string[] = [];
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...READS,
        "GET /v1/machines": ({ query }) => {
          const cursor = query.get("cursor");
          asked.push(cursor ?? `host:exists=${query.get("host:exists")}`);
          return cursor
            ? page([machine(AWAY, "vm-b", "virtual")])
            : page([machine(HOST, "srv-a", "physical")], "/v1/machines?cursor=p2");
        },
      }),
    });
    const { element } = mounted;
    await text(element, "srv-a");
    const more = () => element.querySelector<HTMLElement>(".k-tree-grid__more");
    await vi.waitFor(() => expect(more()).not.toBeNull());
    // One page, and no other before the tree asks for it.
    await new Promise((done) => setTimeout(done, 50));
    expect(asked).toEqual(["host:exists=false"]);
    expect(rows(element).slice(0, 2)).toEqual(["1 grappe", "1 srv-a"]);
    // At the keyboard: Enter on the row that reads the next page of the first level.
    press(element, "more:", "Enter");
    await vi.waitFor(() => expect(rows(element)).toEqual(["1 grappe", "1 srv-a", "1 vm-b"]));
    expect(asked).toEqual(["host:exists=false", "p2"]);
    expect(more()).toBeNull();
  });

  it("says a failed first level with its codes, an empty one with what to do", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    mounted = mount("machines-clusters", {
      context: { locale: "en" },
      fetch: fakeFetch({
        "GET /v1/clusters": () => problem(403, "forbidden"),
        "GET /v1/machines": () => problem(403, "forbidden"),
      }),
    });
    await text(mounted.element, "The hosting could not be read");
    expect(mounted.element.textContent).toContain("req-42");
    mounted.unmount();
    queryClient.clear();
    mounted = mount("machines-clusters", {
      context: { locale: "en" },
      fetch: fakeFetch({
        "GET /v1/clusters": () => page([]),
        "GET /v1/machines": () => page([]),
      }),
    });
    await text(mounted.element, "No cluster and no machine");
    expect(mounted.element.querySelector("[role=treegrid]")).toBeNull();
  });

  it("reads a refused list of guests once, again by « Retry » only", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    let reads = 0;
    let refuse = true;
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...READS,
        "GET /v1/machines": (request) => {
          if (request.query.get("host") !== HOST) return READS["GET /v1/machines"](request);
          reads++;
          return refuse ? problem(403, "forbidden") : page([HOSTED]);
        },
      }),
    });
    const { element } = mounted;
    await text(element, "srv-a");
    press(element, HOST, "ArrowRight");
    // Named by its host, read again by its own « Retry ».
    await text(element, "The machines hosted by srv-a could not be read");
    await new Promise((done) => setTimeout(done, 50));
    expect(reads).toBe(1);
    refuse = false;
    [...element.querySelectorAll("button")].find((b) => b.textContent?.includes("Retry"))!.click();
    await vi.waitFor(() => expect(rows(element)).toContain("2 vm-a"));
    expect(reads).toBe(2);
  });

  it("refuses a cycle at the client, with the rule of the server: nothing sent", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    const writes: string[] = [];
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...READS,
        "POST /v1/drafts": () => {
          writes.push("draft");
          return draftOf([]);
        },
      }),
    });
    const { element } = mounted;
    await text(element, "srv-a");
    press(element, HOST, "ArrowRight");
    await vi.waitFor(() => expect(rows(element)).toContain("2 vm-a"));
    press(element, HOST, "x", true);
    press(element, GUEST, "v", true);
    // Said by the tree (its own words, in the language of @kartenn/ui), with the rule.
    await text(element, ": it hosts it, or it is it (machines.host.invalid)");
    expect(writes).toEqual([]);
  });

  it("refuses a target that hosts nothing (M5) at the client, with the rule: nothing written", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    const CONTAINER = "5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8b24";
    const writes: string[] = [];
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...READS,
        "GET /v1/machines": (request) =>
          request.query.get("host")
            ? READS["GET /v1/machines"](request)
            : page([...MACHINES, machine(CONTAINER, "ct-a", "container")]),
        "POST /v1/drafts": () => {
          writes.push("draft");
          return draftOf([]);
        },
      }),
    });
    const { element } = mounted;
    await text(element, "ct-a");
    press(element, AWAY, "x", true);
    press(element, CONTAINER, "v", true);
    await text(element, ": a host of a type not admitted (machines.host.invalid)");
    await new Promise((done) => setTimeout(done, 20));
    expect(writes).toEqual([]);
  });

  it("gives back the place of a list answered once its node closed; never reads a node closed before its turn", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    const ids = [1, 2, 3, 4, 5, 6].map((i) => `5d0a3b61-1f2e-4a66-8b54-7a2e3c2d8c0${i}`);
    const asked: string[] = [];
    let release!: () => void;
    const held = new Promise<void>((done) => (release = done));
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        "GET /v1/clusters": () =>
          page(
            ids.map((id, i) => ({
              id,
              href: `/v1/clusters/${id}`,
              slug: `c${i + 1}`,
              owner: { id: ENTITY },
            })),
          ),
        "GET /v1/machines": async ({ query }) => {
          const host = query.get("host");
          if (!host) return page([]);
          asked.push(host);
          // The first four lists are slow: they hold every place.
          if (ids.slice(0, 4).includes(host)) await held;
          return page([]);
        },
      }),
    });
    const { element } = mounted;
    await text(element, "c6");
    /** Opens or closes a node, the tree drawn again before the next key. */
    const toggle = async (id: string, open: boolean) => {
      press(element, id, open ? "ArrowRight" : "ArrowLeft");
      await new Promise((done) => setTimeout(done, 10));
    };
    for (const id of ids.slice(0, 5)) await toggle(id, true);
    await vi.waitFor(() => expect(asked).toEqual(ids.slice(0, 4)));
    // The fifth waits for a place, shown as loading.
    await vi.waitFor(() =>
      expect(rowOf(element, ids[4]!)?.dataset["state"]).toBe("loading-children"),
    );
    // All closed before any answer; then the four lists answer.
    for (const id of ids.slice(0, 5)) await toggle(id, false);
    release();
    await new Promise((done) => setTimeout(done, 50));
    // The fifth, closed before its turn: never read.
    expect(asked).toEqual(ids.slice(0, 4));
    // The places came back: another node opened is read.
    await toggle(ids[5]!, true);
    await vi.waitFor(() => expect(asked).toEqual([...ids.slice(0, 4), ids[5]]));
  });

  it("moves to a cluster in the draft: pending until simulated, never written in the cache; the warnings said", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    const setQueryData = vi.spyOn(queryClient, "setQueryData");
    const posted: unknown[] = [];
    const changesets: unknown[] = [];
    let release!: () => void;
    const held = new Promise<void>((done) => (release = done));
    const warning = {
      rule: "machines.host.site_mismatch",
      severity: "warning",
      detail: "the machine is hosted on another site",
      subjects: [AWAY],
      operation: null,
      pointer: null,
    };
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...READS,
        "GET /v1/drafts": () => page([]),
        "POST /v1/drafts": ({ body }) => {
          posted.push(body);
          return draftOf((body as { operations: unknown[] }).operations);
        },
        "POST /v1/drafts/{id}/simulation": async () => {
          await held;
          return simulation(true, [warning]);
        },
        "POST /v1/changesets": ({ body }) => {
          changesets.push(body);
          return problem(500, "internal");
        },
      }),
    });
    const { element } = mounted;
    // The status region is there before any move.
    await text(element, "vm-b");
    expect(status(element)).toBeDefined();
    expect(status(element).textContent?.trim()).toBe("");
    press(element, AWAY, "x", true);
    press(element, CLUSTER, "v", true);
    // The wait is the row's, not said again by the view.
    await vi.waitFor(() => expect(rowOf(element, AWAY)?.dataset["move"]).toBe("pending"));
    expect(status(element).textContent?.trim()).toBe("");
    // A second intent in flight: ignored.
    press(element, AWAY, "x", true);
    press(element, HOST, "v", true);
    await new Promise((done) => setTimeout(done, 20));
    expect(posted).toHaveLength(1);
    release();
    await text(element, "Move of vm-b to grappe in the draft, held by its simulation");
    expect(status(element).textContent).toContain("machines.host.site_mismatch");
    expect(status(element).textContent).toContain("the machine is hosted on another site");
    // At its place, marked as moved in the draft; the cluster closed, its guests unread: no
    // preview row.
    expect(rows(element)).toEqual(["1 grappe", "1 srv-a", "1 vm-b"]);
    expect(rowOf(element, AWAY)?.dataset["state"]).toBe("moved");
    expect(rowOf(element, `preview:${AWAY}`)).toBeNull();
    expect(status(element).textContent).toContain("Its preview shows under grappe once opened.");
    expect(rowOf(element, AWAY)?.hasAttribute("data-move")).toBe(false);
    expect(posted).toEqual([
      {
        title: expect.any(String),
        operations: [
          { op: "machines.update_machine", id: { id: AWAY }, host: { cluster: { id: CLUSTER } } },
        ],
      },
    ]);
    expect(changesets).toEqual([]);
    // No key of the tree written (X33): the draft's own entry only, by the SDK.
    const keys = setQueryData.mock.calls.map(([key]) => String((key as unknown[])[1]));
    expect(keys.filter((key) => /^\/v1\/(machines|clusters)/.test(key))).toEqual([]);
    // The draft no longer active (left, deleted, submitted): no longer marked.
    drafts.setActive(null);
    await vi.waitFor(() => expect(rowOf(element, AWAY)?.hasAttribute("data-state")).toBe(false));
  });

  it("moved under a host opened: moved at its place, its preview under the host, until the draft goes", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...READS,
        "GET /v1/drafts": () => page([]),
        "POST /v1/drafts": ({ body }) => draftOf((body as { operations: unknown[] }).operations),
        "POST /v1/drafts/{id}/simulation": () => simulation(true, []),
      }),
    });
    const { element } = mounted;
    await text(element, "vm-b");
    // The host opened: its guests read.
    press(element, HOST, "ArrowRight");
    await vi.waitFor(() =>
      expect(rows(element)).toEqual(["1 grappe", "1 srv-a", "2 vm-a", "1 vm-b"]),
    );
    press(element, AWAY, "x", true);
    press(element, HOST, "v", true);
    await text(element, "Move of vm-b to srv-a in the draft, held by its simulation");
    // Two rows of vm-b, told apart by their state, not by their level only (021/A27).
    expect(rows(element)).toEqual(["1 grappe", "1 srv-a", "2 vm-a", "2 vm-b", "1 vm-b"]);
    expect(status(element).textContent).not.toContain("once opened");
    expect(rowOf(element, AWAY)?.dataset["state"]).toBe("moved");
    const preview = rowOf(element, `preview:${AWAY}`);
    expect(preview?.dataset["state"]).toBe("preview");
    expect(preview?.getAttribute("aria-level")).toBe("2");
    expect(preview?.textContent).toContain("virtual");
    // Enter on the preview: the record of the machine it shows.
    press(element, `preview:${AWAY}`, "Enter");
    expect(followed).toEqual([`/x/machines/machines/${AWAY}`]);
    // « Déplacer vers… » on the preview chosen (Space): refused, with its reason.
    press(element, `preview:${AWAY}`, " ");
    const button = [...element.querySelectorAll("button")].find((b) =>
      b.textContent?.includes("Move to…"),
    )!;
    await vi.waitFor(() =>
      expect(
        document.getElementById(button.getAttribute("aria-describedby") ?? "")?.textContent,
      ).toContain("This is the draft preview: select the machine at its present place."),
    );
    expect(button.getAttribute("aria-disabled")).toBe("true");
    // The draft no longer active: neither mark nor preview, the preview chosen no longer.
    drafts.setActive(null);
    await vi.waitFor(() => expect(rowOf(element, `preview:${AWAY}`)).toBeNull());
    await vi.waitFor(() =>
      expect(
        document.getElementById(button.getAttribute("aria-describedby") ?? "")?.textContent,
      ).toContain("Select a machine of the tree first"),
    );
    expect(rowOf(element, AWAY)?.hasAttribute("data-state")).toBe(false);
    expect(rows(element)).toEqual(["1 grappe", "1 srv-a", "2 vm-a", "1 vm-b"]);
  });

  it("refused by the simulation: the node at its place, refused with the code of the rule", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    const invalid = {
      rule: "machines.host.invalid",
      severity: "error",
      detail: "a physical machine cannot be hosted",
      subjects: [HOST],
      operation: 0,
      pointer: "/operations/0",
    };
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        ...READS,
        "GET /v1/drafts": () => page([]),
        "POST /v1/drafts": ({ body }) => draftOf((body as { operations: unknown[] }).operations),
        "POST /v1/drafts/{id}/simulation": () => simulation(false, [invalid]),
      }),
    });
    const { element } = mounted;
    await text(element, "srv-a");
    press(element, HOST, "x", true);
    press(element, CLUSTER, "v", true);
    await vi.waitFor(() => expect(rowOf(element, HOST)?.dataset["move"]).toBe("refused"));
    expect(rowOf(element, HOST)?.textContent).toContain("machines.host.invalid");
    expect(rowOf(element, HOST)?.getAttribute("aria-level")).toBe("1");
    expect(status(element).textContent).toContain("The operation stays in the draft");
  });

  it("refuses « Move to… » without machines.edit; offers no move at another date", async () => {
    history.replaceState(null, "", "/x/machines/clusters");
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user(["machines.edit"]) },
      fetch: fakeFetch(READS),
    });
    await text(mounted.element, "srv-a");
    const button = [...mounted.element.querySelectorAll("button")].find((b) =>
      b.textContent?.includes("Move to…"),
    )!;
    expect(button.getAttribute("aria-disabled")).toBe("true");
    mounted.unmount();
    queryClient.clear();
    mounted = mount("machines-clusters", {
      context: {
        locale: "en",
        user: user(),
        time: { validAt: "2026-01-01T00:00:00Z", asOf: null },
      },
      fetch: fakeFetch(READS),
    });
    await text(mounted.element, "srv-a");
    expect(
      [...mounted.element.querySelectorAll("button")].some((b) =>
        b.textContent?.includes("Move to…"),
      ),
    ).toBe(false);
    expect(rowOf(mounted.element, HOST)?.getAttribute("draggable")).not.toBe("true");
    expect(mounted.element.textContent).toContain("moves are offered at the present only");
  });
});

describe("the record of a cluster (task 18cd)", () => {
  it("draws its owner, its site and its machines with their tie, its failure with its codes", async () => {
    history.replaceState(null, "", `/x/machines/clusters/${CLUSTER}`);
    mounted = mount("machines-clusters", {
      context: { locale: "en", user: user() },
      fetch: fakeFetch({
        "GET /v1/clusters/{id}": () => ({ ...cluster, site: { id: SITE } }) as never,
        "GET /v1/clusters/{id}/machines": () =>
          page([
            { ...machine(HOST, "srv-a", "physical"), member_of: { id: CLUSTER } },
            machine(GUEST, "vm-a", "virtual", { cluster: { id: CLUSTER } }),
          ]),
        "GET /v1/entities/{id}": () =>
          ({ id: ENTITY, slug: "exemple", href: `/v1/entities/${ENTITY}` }) as never,
        "GET /v1/sites/{id}": READS["GET /v1/sites/{id}"],
      }),
    });
    const { element } = mounted;
    await text(element, "exemple");
    await text(element, "nord");
    expect(element.querySelector("h1")?.textContent?.trim()).toBe("grappe");
    const grid = element.querySelector("[role=grid][aria-label='Machines of the cluster']")!;
    const cells = [...grid.querySelectorAll("[role=row]")]
      .slice(1)
      .map((row) => [...row.querySelectorAll("[role=gridcell]")].map((c) => c.textContent?.trim()));
    // The kind, a `KeyLabel`: its label, then its key.
    expect(cells).toEqual([
      ["srv-a", "physicalphysical", "member"],
      ["vm-a", "virtualvirtual", "hosted"],
    ]);
    mounted.unmount();
    queryClient.clear();
    mounted = mount("machines-clusters", {
      context: { locale: "en" },
      fetch: fakeFetch({
        "GET /v1/clusters/{id}": () => problem(404, "not-found"),
        "GET /v1/clusters/{id}/machines": () => page([]),
      }),
    });
    await text(mounted.element, "The cluster could not be read");
    expect(mounted.element.textContent).toContain("req-42");
  });
});
