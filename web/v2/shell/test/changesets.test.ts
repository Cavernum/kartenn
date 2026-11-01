// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import {
  countedGroups,
  diffGroups,
  permissionsOf,
  reviewActions,
  stateBadge,
  typeOfOperation,
  type Changeset,
} from "../src/changesets/model.ts";
import { operationLine } from "../src/draft/operations.ts";
import { sidebarGroups } from "../src/frame/spaces.ts";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, Rights, type Me } from "../src/rights/index.ts";

// Changesets, reviews, drafts (task 021-14; REQ-021-6, 10, 11, 37): the review actions a
// reader is offered by their rights — refused ones kept with their permission or reason —,
// the differential grouped by object, an operation as a line; then the screens mounted
// against a fake API: a decision under `If-Match`, the motive of a rejection required,
// the foot of a draft by the rights and the simulation, its submission replaying its key.

const C1 = "0b5e3c1a-0000-4000-8000-0000000000e1";
const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";
const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";

const changeset = (over: Partial<Changeset> = {}): Changeset => ({
  id: C1,
  author: "bob",
  state: "pending",
  kind: "approval",
  motive: "Renumérotation salle B",
  submitted_at: "2026-01-14T16:02:00Z",
  valid_from: "2026-01-14T16:02:00Z",
  operations: [
    { op: "update_prefix", id: { id: P1 }, nature: "container" },
    { op: "close", kind: "vlan", id: { id: P1 } },
  ] as never,
  violations: [],
  diff: [],
  cascaded: [],
  links: { diff: `/v1/changesets/${C1}/diff` },
  ...over,
});

describe("the review of a changeset", () => {
  const reviewer = new Rights(["core.prefix.apply", "core.vlan.apply", "core.prefix.edit"]);

  it("types the operations, and asks the permission of each type once", () => {
    expect(typeOfOperation({ op: "create_vlan_group", key: "g" })).toBe("core.vlan_group");
    expect(typeOfOperation({ op: "rename", kind: "domain", id: { id: P1 }, slug: "x" })).toBe(
      "core.domain",
    );
    expect(typeOfOperation({ op: "within", operation: { op: "update_address" }, period: {} })).toBe(
      "core.address",
    );
    // The server alone knows what an operation of the rights asks for.
    expect(typeOfOperation({ op: "set_role", key: "r" })).toBeNull();
    expect(permissionsOf(changeset().operations, "apply")).toEqual([
      "core.prefix.apply",
      "core.vlan.apply",
    ]);
  });

  it("offers a reviewer validation and rejection, the withdrawal refused with its reason", () => {
    expect(reviewActions(changeset(), { me: "alice", rights: reviewer, present: true })).toEqual([
      { id: "validation" },
      { id: "rejection" },
      { id: "withdrawal", reason: "author" },
    ]);
  });

  it("names the permission lacking (REQ-021-37): Valider and Rejeter refused, visible", () => {
    const lacking = new Rights(["core.prefix.apply"]);
    const actions = reviewActions(changeset(), { me: "alice", rights: lacking, present: true });
    expect(actions.slice(0, 2)).toEqual([
      { id: "validation", forbidden: "core.vlan.apply" },
      { id: "rejection", forbidden: "core.vlan.apply" },
    ]);
  });

  it("lets its author withdraw only; nothing out of the present; an applied one undone", () => {
    expect(reviewActions(changeset(), { me: "bob", rights: reviewer, present: true })).toEqual([
      { id: "validation", reason: "own" },
      { id: "withdrawal" },
    ]);
    expect(
      reviewActions(changeset(), { me: "alice", rights: reviewer, present: false }).every(
        (a) => a.reason === "past",
      ),
    ).toBe(true);
    const applied = changeset({ state: "applied" });
    expect(reviewActions(applied, { me: "alice", rights: reviewer, present: true })).toEqual([
      { id: "undo", forbidden: "core.vlan.edit" },
    ]);
    expect(
      reviewActions(changeset({ state: "rejected" }), { me: "a", rights: reviewer, present: true }),
    ).toEqual([]);
  });

  it("shows a state by its meaning", () => {
    expect(stateBadge("pending")).toBe("review");
    expect(stateBadge("applied")).toBe("ok");
    expect(stateBadge("rejected")).toBe("ko");
    expect(stateBadge("withdrawn")).toBe("neutral");
  });

  it("groups the differential by object, each line by its values, masked said", () => {
    const groups = diffGroups(
      [
        { object: { id: P1 }, property: "core.owner", before: null, after: "nord", masked: false },
        {
          object: { id: P1 },
          property: "dns.ttl",
          before: 60 as never,
          after: 300 as never,
          masked: false,
        },
        { object: { id: D1 }, property: "core.secret", before: null, after: null, masked: true },
        { object: { id: C1 }, property: "core.owner", before: "sud", after: null, masked: false },
      ] as never,
      {
        property: (key) => (key === "core.owner" ? "Propriétaire" : null),
        masked: "masqué",
        object: (id) => `objet ${id.slice(-2)}`,
      },
    );
    expect(groups.map((g) => [g.title, g.kind])).toEqual([
      ["objet a1", "changed"],
      ["objet d1", "changed"],
      ["objet e1", "removed"],
    ]);
    expect(groups[0]!.changes).toEqual([
      { attribute: "Propriétaire (core.owner)", kind: "added", after: "nord" },
      { attribute: "dns.ttl", kind: "changed", before: "60", after: "300" },
    ]);
    expect(groups[1]!.changes[0]).toMatchObject({ before: "masqué", after: "masqué" });
  });

  it("shows the counted differential of a changeset not applied, a group per property", () => {
    const groups = countedGroups(
      [
        { property: "core.owner", before: null, after: '"nord"', count: 2 },
        { property: "core.owner", before: '"sud"', after: '"nord"', count: 1 },
        { property: "core.secret", before: null, after: null, count: 1, masked: true },
      ],
      {
        property: (key) => (key === "core.owner" ? "Propriétaire" : null),
        masked: "masqué",
        objects: (n) => `${n} objets`,
      },
    );
    expect(groups.map((g) => [g.title, g.objectKey, g.kind])).toEqual([
      ["Propriétaire", "core.owner", "changed"],
      ["core.secret", undefined, "changed"],
    ]);
    expect(groups[0]!.changes).toEqual([
      { attribute: "2 objets", kind: "added", after: "nord" },
      { attribute: "1 objets", kind: "changed", before: "sud", after: "nord" },
    ]);
  });

  it("writes an operation as a line: its code, what it names, its other members", () => {
    expect(
      operationLine({ op: "update_prefix", id: { id: P1 }, nature: "container", expected: "v" }),
    ).toEqual({
      op: "update_prefix",
      subject: P1,
      members: [{ name: "nature", value: "container" }],
    });
    expect(
      operationLine({
        op: "create_range",
        domain: { id: D1 },
        first: "192.0.2.1",
        last: "192.0.2.9",
      }),
    ).toMatchObject({ subject: "192.0.2.1–192.0.2.9", members: [{ name: "domain", value: D1 }] });
  });

  it("counts « À instruire » in the sidebar, nothing shown at zero", () => {
    const rights = new Rights(["core.changeset.read"]);
    const entries = (badges: Record<string, number>) =>
      sidebarGroups(rights, (k) => k, undefined, badges).find((g) => g.id === "changes")!.entries;
    expect(entries({ proposals: 3 }).find((e) => e.id === "proposals")?.badge).toBe(3);
    expect(entries({ proposals: 0 }).find((e) => e.id === "proposals")).not.toHaveProperty("badge");
  });
});

describe("the screens of the changesets and the drafts", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
  });

  interface Seen {
    method: string;
    url: string;
    body: unknown;
    ifMatch: string | null;
    key: string | null;
  }
  type Answer = { status?: number; body: unknown; etag?: string } | "lost";

  async function mounted(at: string, permissions: string[], answer: (seen: Seen) => Answer) {
    const seen: Seen[] = [];
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const headers = new Headers(init?.headers);
      const call: Seen = {
        method: init?.method ?? "GET",
        url: String(input),
        body: init?.body ? JSON.parse(String(init.body)) : null,
        ifMatch: headers.get("If-Match"),
        key: headers.get("Idempotency-Key"),
      };
      seen.push(call);
      if (call.url.startsWith("/v1/labels")) {
        return new Response(
          JSON.stringify({ lang: "fr", default_lang: "fr", revision: "1", spaces: {} }),
          { headers: { "Content-Type": "application/json" } },
        );
      }
      const out = answer(call);
      if (out === "lost") throw new TypeError("network");
      const status = out.status ?? 200;
      return new Response(JSON.stringify(out.body), {
        status,
        headers: {
          "Content-Type": status >= 400 ? "application/problem+json" : "application/json",
          ...(out.etag ? { ETag: out.etag } : {}),
        },
      });
    }) as typeof fetch);
    await loadSession(async (url) =>
      url === "/v1/me"
        ? ({
            identity: "alice",
            kind: "person",
            global_permissions: permissions,
            bindings: [],
          } as unknown as Me)
        : null,
    );
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: RouterView, children: frameRoutes }],
    });
    await router.push(at);
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({ render: () => h(RouterView) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    await nextTick();
    return { seen, root, router };
  }

  const button = (root: HTMLElement, name: string) =>
    [...root.querySelectorAll<HTMLButtonElement>("button")].find(
      // Its words, before the reason a refused one carries.
      (b) => b.textContent?.trim().startsWith(name),
    );

  it("validates a changeset under If-Match of its state, then reads it again", async () => {
    let state = "pending";
    const { seen, root } = await mounted(
      `/changesets/${C1}`,
      ["core.changeset.read", "core.prefix.apply", "core.vlan.apply"],
      (call) => {
        if (call.method === "POST") {
          state = "applied";
          return {
            body: { changeset: { id: C1, href: "h" }, state, warnings: [], diff: [], cascaded: [] },
          };
        }
        if (call.url === `/v1/changesets/${C1}`) {
          return { body: changeset({ state }), etag: `"${state}"` };
        }
        return { body: { counts: [], items: [], links: {}, sections: [] } };
      },
    );
    await expect.poll(() => root.textContent).toContain("Renumérotation salle B");
    await expect.poll(() => button(root, "Valider")).toBeDefined();
    button(root, "Valider")!.click();
    await expect.poll(() => seen.some((s) => s.method === "POST")).toBe(true);
    const post = seen.find((s) => s.method === "POST")!;
    expect(post.url).toBe(`/v1/changesets/${C1}/validation`);
    expect(post.ifMatch).toBe('"pending"');
    await expect.poll(() => root.textContent).toContain("Appliqué");
  });

  it("refuses Valider without the permission, its name said; the rejection asks a motive", async () => {
    const { root, seen } = await mounted(
      `/changesets/${C1}`,
      ["core.changeset.read", "core.prefix.apply"],
      (call) =>
        call.url === `/v1/changesets/${C1}`
          ? { body: changeset(), etag: '"pending"' }
          : { body: { counts: [], items: [], links: {}, sections: [] } },
    );
    await expect.poll(() => button(root, "Valider")).toBeDefined();
    const validate = button(root, "Valider")!;
    expect(validate.getAttribute("aria-disabled")).toBe("true");
    const described = validate.getAttribute("aria-describedby")!;
    expect(document.getElementById(described)?.textContent).toContain("core.vlan.apply");
    validate.click();
    await nextTick();
    expect(seen.some((s) => s.method === "POST")).toBe(false);
  });

  const draft = (operations: unknown[], version = 1, reason = {}) => ({
    id: D1,
    title: "Brouillon du jour",
    author: "alice",
    href: `/v1/drafts/${D1}`,
    created_at: "2026-11-01T10:00:00Z",
    updated_at: "2026-11-01T10:00:00Z",
    version,
    operation_count: operations.length,
    operations,
    reason,
  });
  const valid = { valid: true, violations: [], diff: [], sample: [], cascaded: [], sections: [] };
  const OPS = [{ op: "update_prefix", id: { id: P1 }, nature: "container" }];

  /** The split button of a mounted foot: its main button. */
  const main = (root: HTMLElement) =>
    root.querySelector<HTMLButtonElement>(".k-split-button__main")!;
  /** Its label, without the shortcut it shows. */
  const label = (root: HTMLElement) =>
    main(root)
      ?.textContent?.replace(/Ctrl\s*Entrée$/, "")
      .trim();

  it("offers « Appliquer » with approval.self, « Soumettre en revue » without, its permission said", async () => {
    const answer = (call: Seen): Answer =>
      call.url.endsWith("/simulation") ? { body: valid } : { body: draft(OPS), etag: '"1"' };
    const author = await mounted(
      `/drafts/${D1}`,
      ["core.prefix.edit", "core.prefix.approval.self"],
      answer,
    );
    await expect.poll(() => label(author.root), { timeout: 3000 }).toBe("Appliquer");
    // « Appliquer » prefills a generic summary.
    expect(author.root.querySelector<HTMLInputElement>(".k-commit-form input")?.value).toBe(
      "Maintenance du référentiel",
    );
    app?.unmount();
    queryClient.clear();
    const editor = await mounted(`/drafts/${D1}`, ["core.prefix.edit"], answer);
    await expect.poll(() => label(editor.root), { timeout: 3000 }).toBe("Soumettre en revue");
  });

  it("submits with ?approval=self and its key, saving the motive first; a lost answer replays the key", async () => {
    let lost = 1;
    const { seen, root } = await mounted(
      `/drafts/${D1}`,
      ["core.prefix.edit", "core.prefix.approval.self"],
      (call) => {
        if (call.url.endsWith("/simulation")) return { body: valid };
        if (call.url.includes("/submission")) {
          if (lost-- > 0) return "lost";
          return {
            status: 201,
            body: {
              changeset: { id: C1, href: "h" },
              state: "applied",
              warnings: [],
              diff: [],
              cascaded: [],
            },
          };
        }
        if (call.method === "PATCH") {
          return { body: draft(OPS, 2, (call.body as { reason: object }).reason), etag: '"2"' };
        }
        return { body: draft(OPS), etag: '"1"' };
      },
    );
    await expect.poll(() => label(root), { timeout: 3000 }).toBe("Appliquer");
    // Refused while the simulation is still to come.
    expect(main(root).getAttribute("aria-disabled")).toBe("true");
    await expect.poll(() => main(root).getAttribute("aria-disabled"), { timeout: 3000 }).toBeNull();
    main(root).click();
    await expect.poll(() => button(root, "Réessayer")).toBeDefined();
    const patch = seen.find((s) => s.method === "PATCH")!;
    expect(patch.body).toEqual({
      reason: { summary: "Maintenance du référentiel", description: null },
    });
    // The motive saved is a new version: « Réessayer » waits for its simulation.
    const retry = () => button(root, "Réessayer")!;
    expect(retry().getAttribute("aria-disabled")).toBe("true");
    await expect.poll(() => retry().getAttribute("aria-disabled"), { timeout: 3000 }).toBeNull();
    retry().click();
    await expect.poll(() => seen.filter((s) => s.url.includes("/submission")).length).toBe(2);
    const [first, again] = seen.filter((s) => s.url.includes("/submission"));
    expect(first!.url).toBe(`/v1/drafts/${D1}/submission?approval=self`);
    expect(again!.key).toBe(first!.key);
    expect(first!.key).not.toBeNull();
  });

  it("violations of the simulation: on their operation with their code, only the save left", async () => {
    const { root } = await mounted(
      `/drafts/${D1}`,
      ["core.prefix.edit", "core.prefix.approval.self"],
      (call) =>
        call.url.endsWith("/simulation")
          ? {
              body: {
                ...valid,
                valid: false,
                violations: [
                  {
                    rule: "core.prefix.duplicate",
                    severity: "error",
                    detail: "Déjà là.",
                    operation: 0,
                    subjects: [],
                  },
                ],
              },
            }
          : { body: draft(OPS), etag: '"1"' },
    );
    // « Appliquer » waits for the simulation, then gives way to the save alone.
    await expect.poll(() => label(root)).toBe("Appliquer");
    await expect
      .poll(() => root.textContent, { timeout: 3000 })
      .toContain("Opération 1 : refusée par la simulation");
    expect(label(root)).toBe("Enregistrer le brouillon");
    expect(root.textContent).toContain("core.prefix.duplicate");
  });

  it("removes an operation: the others kept, under If-Match of the draft read", async () => {
    const two = [...OPS, { op: "update_prefix", id: { id: P1 }, nature: "network" }];
    const { seen, root } = await mounted(`/drafts/${D1}`, ["core.prefix.edit"], (call) => {
      if (call.url.endsWith("/simulation")) return { body: valid };
      if (call.method === "PATCH") return { body: draft([two[1]], 2), etag: '"2"' };
      return { body: draft(two), etag: '"1"' };
    });
    await expect
      .poll(() => root.querySelectorAll("[data-k-remove]").length, { timeout: 3000 })
      .toBe(2);
    root.querySelector<HTMLButtonElement>("[data-k-remove]")!.click();
    await expect.poll(() => seen.some((s) => s.method === "PATCH")).toBe(true);
    const patch = seen.find((s) => s.method === "PATCH")!;
    expect(patch.ifMatch).toBe('"1"');
    expect(patch.body).toEqual({ operations: [two[1]] });
    await expect.poll(() => root.querySelectorAll("[data-k-remove]").length).toBe(1);
  });
});
