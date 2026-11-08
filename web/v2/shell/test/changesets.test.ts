// @vitest-environment happy-dom
import { draft as pluginDraft, drafts, queryClient } from "@kartenn/sdk";
import { addToActiveDraft, draftHost, resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, nextTick, onScopeDispose, ref, type App } from "vue";
import DraftOperations from "../src/draft/DraftOperations.vue";
import { draftPeriods, lostSubmissions } from "../src/draft/state.ts";
import { pageCount } from "../src/frame/spaces.ts";
import { instantOfDay, listFilterModel } from "../src/views/filters.ts";
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
import { memberText, operationLine } from "../src/draft/operations.ts";
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

  it("lets its author validate under approval.self when no policy holds it (REQ-024-11, 12)", () => {
    const self = new Rights(["core.prefix.approval.self", "core.vlan.approval.self"]);
    // All of it under `approval.self`, no policy: the author applies it.
    const proposal = changeset({ kind: "proposal" });
    expect(reviewActions(proposal, { me: "bob", rights: self, present: true })).toEqual([
      { id: "validation" },
      { id: "withdrawal" },
    ]);
    // A part without `approval.self`: refused, the permission lacking named.
    const partial = new Rights(["core.prefix.approval.self"]);
    expect(reviewActions(proposal, { me: "bob", rights: partial, present: true })).toEqual([
      { id: "validation", forbidden: "core.vlan.approval.self" },
      { id: "withdrawal" },
    ]);
    // Held by a policy: refused whatever the rights, the policy said.
    for (const policy of [
      changeset({ kind: "approval" }),
      changeset({ kind: "proposal", policy: "p1" }),
    ]) {
      expect(reviewActions(policy, { me: "bob", rights: self, present: true })).toEqual([
        { id: "validation", reason: "held" },
        { id: "withdrawal" },
      ]);
    }
  });

  it("nothing out of the present; an applied one undone; a closed one decided no more", () => {
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
    // The values of an assignment, each as text: a local name as `#n` (021/A13).
    expect(
      operationLine({
        op: "assign_property",
        kind: "prefix",
        object: { id: P1 },
        property: { id: D1 },
        values: [{ local: 0 }],
      }).members,
    ).toContainEqual({ name: "values", value: "#0" });
    // Any other list too, item by item (a reference, a text, an object as JSON); empty, `[]`.
    expect(memberText([{ id: P1 }, "x", { a: 1 }])).toBe(`${P1}, x, {"a":1}`);
    expect(memberText([])).toBe("[]");
    // Absent: `null`, said by the line (`NoValue`), never a dash read « tiret » (020-30).
    expect(memberText(null)).toBeNull();
    expect(memberText([null, "x"])).toBe("null, x");
    expect(
      operationLine({ op: "within", period: null, operation: { op: "delete_prefix", id: P1 } })
        .members,
    ).toContainEqual({ name: "period", value: null });
    expect(operationLine({ op: "update_range", first: "192.0.2.1", last: null }).subject).toBe(
      "192.0.2.1",
    );
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
    for (const lost of lostSubmissions.all()) lostSubmissions.drop(lost.id);
    draftPeriods.drop(D1);
    draftHost.onAdd(null);
  });

  interface Seen {
    method: string;
    url: string;
    body: unknown;
    ifMatch: string | null;
    key: string | null;
  }
  /** The draft tab as the frame fills it: the active draft of the tab, followed. */
  const PanelHost = defineComponent({
    setup() {
      const active = ref(drafts.active());
      onScopeDispose(drafts.onActive((id) => (active.value = id)));
      return () => h(DraftOperations, { id: active.value });
    },
  });
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
      routes: [
        // The draft tab of the right panel: the operations of the active draft of the tab.
        { path: "/panel", component: PanelHost },
        { path: "/", component: RouterView, children: frameRoutes },
      ],
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
    // A replay of what was sent: it waits for no simulation.
    button(root, "Réessayer")!.click();
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

  it("a removal refused 404 (deleted elsewhere): said where asked, with its codes; then no draft", async () => {
    sessionStorage.setItem("kartenn.draft.active", D1);
    const { root } = await mounted("/panel", ["core.prefix.edit"], (call) => {
      if (call.url.endsWith("/simulation")) return { body: valid };
      if (call.method === "PATCH") {
        return {
          status: 404,
          body: { type: "urn:kartenn:problem:not-found", title: "Not Found", status: 404 },
        };
      }
      return { body: draft(OPS), etag: '"1"' };
    });
    await expect.poll(() => root.querySelectorAll("[data-k-remove]").length).toBe(1);
    root.querySelector<HTMLButtonElement>("[data-k-remove]")!.click();
    await expect.poll(() => root.textContent).toContain("Ce brouillon n'existe plus");
    expect(root.textContent).toContain("urn:kartenn:problem:not-found");
    // Nothing typed is kept by a removal: not said.
    expect(root.textContent).not.toContain("Votre saisie est gardée");
    expect(root.textContent).toContain("Aucun brouillon actif dans cet onglet");
    expect(drafts.active()).toBeNull();
    expect(document.activeElement?.textContent).toContain("Aucun brouillon actif");
  });

  it("a removal refused 412: the draft read again, its own words, nothing typed kept", async () => {
    sessionStorage.setItem("kartenn.draft.active", D1);
    const { root } = await mounted("/panel", ["core.prefix.edit"], (call) => {
      if (call.url.endsWith("/simulation")) return { body: valid };
      if (call.method === "PATCH") {
        return {
          status: 412,
          body: { type: "urn:kartenn:problem:precondition-failed", title: "", status: 412 },
        };
      }
      return { body: draft(OPS), etag: '"1"' };
    });
    await expect.poll(() => root.querySelectorAll("[data-k-remove]").length).toBe(1);
    root.querySelector<HTMLButtonElement>("[data-k-remove]")!.click();
    await expect
      .poll(() => root.textContent)
      .toContain("Le brouillon a changé depuis votre lecture ; relu");
    expect(root.textContent).toContain("Rien n'est retiré");
    expect(root.textContent).not.toContain("Votre saisie est gardée");
  });

  it("a decision refused 412: the changeset read again, said so, the actions of its state", async () => {
    let state = "pending";
    const { root } = await mounted(
      `/changesets/${C1}`,
      ["core.changeset.read", "core.prefix.apply", "core.vlan.apply"],
      (call) => {
        if (call.method === "POST") {
          state = "rejected";
          return {
            status: 412,
            body: { type: "urn:kartenn:problem:precondition-failed", title: "", status: 412 },
          };
        }
        if (call.url === `/v1/changesets/${C1}`) {
          return { body: changeset({ state }), etag: `"${state}"` };
        }
        return { body: { counts: [], items: [], links: {}, sections: [] } };
      },
    );
    await expect.poll(() => button(root, "Valider")).toBeDefined();
    button(root, "Valider")!.click();
    await expect
      .poll(() => root.textContent)
      .toContain("Ce changeset a changé depuis votre lecture ; relu");
    expect(root.textContent).toContain("les actions offertes sont celles de son état actuel");
    // Rejected meanwhile: nothing more to decide.
    await expect.poll(() => button(root, "Valider")).toBeUndefined();
  });

  it("a lost answer keeps its key past the foot, the draft gone: « Réessayer » replays it", async () => {
    let submissions = 0;
    const { seen, root, router } = await mounted(
      `/drafts/${D1}`,
      ["core.prefix.edit", "core.prefix.approval.self"],
      (call) => {
        if (call.url.endsWith("/simulation")) return { body: valid };
        if (call.url.includes("/submission")) {
          submissions += 1;
          if (submissions === 1) return "lost";
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
    await expect
      .poll(() => main(root)?.getAttribute("aria-disabled"), { timeout: 3000 })
      .toBeNull();
    main(root).click();
    await expect.poll(() => button(root, "Réessayer")).toBeDefined();
    // The draft leaves the tab (applied by the server, its event come first): the foot goes.
    drafts.setActive(null);
    await router.push("/panel");
    await expect.poll(() => root.textContent).toContain("n'a pas eu de réponse");
    button(root, "Réessayer")!.click();
    await expect.poll(() => submissions).toBe(2);
    const [first, again] = seen.filter((s) => s.url.includes("/submission"));
    expect(again!.key).toBe(first!.key);
    expect(again!.url).toBe(first!.url);
    await expect.poll(() => lostSubmissions.all().length).toBe(0);
  });

  it("under valid_at alone, submitted from that date: valid_from set, simulated, replayed as sent", async () => {
    const at = "2031-03-01T08:00:00Z";
    const end = "2031-06-30T22:00:00Z";
    draftPeriods.set(D1, { valid_from: at, valid_to: end });
    let submissions = 0;
    const { seen, root } = await mounted(
      `/drafts/${D1}?valid_at=${at}`,
      ["core.prefix.edit", "core.prefix.approval.self"],
      (call) => {
        if (call.url.endsWith("/simulation")) return { body: valid };
        if (call.url.includes("/submission")) {
          submissions += 1;
          if (submissions === 1) return "lost";
          return {
            status: 201,
            body: { changeset: { id: C1, href: "h" }, state: "applied", warnings: [] },
          };
        }
        if (call.method === "PATCH") {
          return { body: draft(OPS, 2, (call.body as { reason: object }).reason), etag: '"2"' };
        }
        return { body: draft(OPS), etag: '"1"' };
      },
    );
    await expect
      .poll(() => main(root)?.getAttribute("aria-disabled"), { timeout: 3000 })
      .toBeNull();
    // The fields show, the sentence of the result says the period.
    expect(root.querySelectorAll(".k-draft-submit .k-date-field").length).toBe(2);
    expect(root.querySelector(".k-draft-submit__result")?.textContent).toContain("effet du ");
    const simulated = seen.find((s) => s.url.endsWith("/simulation"))!;
    expect(simulated.body).toEqual({ valid_from: at, valid_to: end });
    main(root).click();
    await expect.poll(() => button(root, "Réessayer")).toBeDefined();
    // Awaiting its retry, the dates of the foot are fixed, their reason said.
    const segments = [...root.querySelectorAll(".k-draft-submit [role='spinbutton']")];
    expect(segments.length).toBeGreaterThan(0);
    for (const segment of segments) expect(segment.getAttribute("aria-readonly")).toBe("true");
    expect(root.querySelector(".k-draft-submit")?.textContent).toContain(
      "Dates figées : « Réessayer » rejoue telle quelle",
    );
    // Changed after the lost answer: the foot keeps showing, and the replay sends, what was
    // sent, under its key.
    draftPeriods.set(D1, { valid_from: at });
    await expect
      .poll(() => root.querySelector(".k-draft-submit__result")?.textContent, { timeout: 3000 })
      .toContain("effet du ");
    button(root, "Réessayer")!.click();
    await expect.poll(() => submissions).toBe(2);
    const [first, again] = seen.filter((s) => s.url.includes("/submission"));
    expect(first!.body).toEqual({ valid_from: at, valid_to: end });
    expect(again!.body).toEqual(first!.body);
    expect(again!.key).toBe(first!.key);
  });

  it("without a chosen period, valid_from follows valid_at; under as_of the draft is read only", async () => {
    const answer = (call: Seen): Answer =>
      call.url.endsWith("/simulation") ? { body: valid } : { body: draft(OPS), etag: '"1"' };
    const dated = await mounted(
      `/drafts/${D1}?valid_at=2031-03-01T08:00:00Z`,
      ["core.prefix.edit", "core.prefix.approval.self"],
      answer,
    );
    await expect
      .poll(() => dated.seen.find((s) => s.url.endsWith("/simulation"))?.body, { timeout: 3000 })
      .toEqual({ valid_from: "2031-03-01T08:00:00Z" });
    app?.unmount();
    queryClient.clear();

    const { root } = await mounted(
      `/drafts/${D1}?as_of=2026-01-01T00:00:00Z`,
      ["core.prefix.edit", "core.prefix.approval.self"],
      answer,
    );
    await expect.poll(() => root.textContent, { timeout: 3000 }).toContain("Lecture seule");
    await expect.poll(() => label(root), { timeout: 3000 }).toBe("Enregistrer le brouillon");
    expect(main(root).getAttribute("aria-disabled")).toBe("true");
    const remove = root.querySelector<HTMLButtonElement>("[data-k-remove]")!;
    expect(remove.getAttribute("aria-disabled")).toBe("true");
    // The motive read only, in the tab order, its reason said (task 020-31, 024/M14).
    const summary = root.querySelector<HTMLInputElement>(".k-commit-form input")!;
    expect(summary.disabled).toBe(false);
    expect(summary.readOnly).toBe(true);
    const why = summary.getAttribute("aria-describedby")!;
    expect(document.getElementById(why)?.textContent).toContain("date passée");
    expect(button(root, "Choisir les dates d'effet")!.getAttribute("aria-disabled")).toBe("true");
  });

  it("the period kept with the draft, a reload included: back at the present, simulated at it", async () => {
    const at = "2031-03-01T08:00:00Z";
    // As the SDK keeps it for the tab after an addition under `valid_at` (`PERIOD_KEY`).
    sessionStorage.setItem(`kartenn.draft.period.${D1}`, JSON.stringify({ valid_from: at }));
    const { seen, root } = await mounted(
      `/drafts/${D1}`,
      ["core.prefix.edit", "core.prefix.approval.self"],
      (call) =>
        call.url.endsWith("/simulation") ? { body: valid } : { body: draft(OPS), etag: '"1"' },
    );
    await expect
      .poll(() => seen.find((s) => s.url.endsWith("/simulation"))?.body, { timeout: 3000 })
      .toEqual({ valid_from: at });
    await expect
      .poll(() => root.querySelector(".k-draft-submit__result")?.textContent)
      .toContain("effet à partir du");
  });

  it("under as_of, « Réessayer » of a submission lost past the foot is refused with its reason", async () => {
    lostSubmissions.keep({
      id: D1,
      key: "0b5e3c1a-0000-4000-8000-0000000000f1",
      approvalSelf: true,
      title: "perdu",
      period: {},
    });
    const { seen, root } = await mounted(
      "/panel?as_of=2026-01-01T00:00:00Z",
      ["core.prefix.edit", "core.prefix.approval.self"],
      () => ({ body: draft(OPS), etag: '"1"' }),
    );
    await expect.poll(() => root.textContent).toContain("n'a pas eu de réponse");
    const retry = button(root, "Réessayer")!;
    expect(retry.getAttribute("aria-disabled")).toBe("true");
    const why = retry.getAttribute("aria-describedby")!;
    expect(document.getElementById(why)?.textContent).toContain("date passée");
    retry.click();
    await nextTick();
    expect(seen.filter((s) => s.url.includes("/submission"))).toHaveLength(0);
    expect(lostSubmissions.get(D1)).toBeDefined();
  });

  it("a plugin's draft.add opens the draft; the shell's own additions do not", async () => {
    await mounted("/panel", ["core.prefix.edit"], (call) =>
      call.method === "POST"
        ? { status: 201, body: draft(OPS), etag: '"1"' }
        : { body: draft(OPS), etag: '"1"' },
    );
    let opened = 0;
    draftHost.onAdd(() => (opened += 1));
    await addToActiveDraft(OPS as never);
    expect(opened).toBe(0);
    await pluginDraft.add(OPS[0] as never);
    expect(opened).toBe(1);
  });
});

describe("the counters and the filters of the changesets", () => {
  it("says a bound, not a total, when a next page exists", () => {
    expect(pageCount(3, false)).toBe(3);
    expect(pageCount(100, true)).toBe("100+");
  });

  it("types the submission dates as days, sent as the instants that bound them", () => {
    const words = {
      field: () => undefined,
      value: () => undefined,
      property: () => undefined,
      propertyValue: () => undefined,
    };
    const model = listFilterModel("/v1/changesets", [], words);
    expect(model.find((p) => p.key === "from")?.type).toBe("date");
    expect(model.find((p) => p.key === "to")?.type).toBe("date");
    expect(model.find((p) => p.key === "state")?.type).toBe("enum");
    const start = new Date(instantOfDay("2026-11-01", "start"));
    const end = new Date(instantOfDay("2026-11-01", "end"));
    expect([start.getFullYear(), start.getMonth(), start.getDate(), start.getHours()]).toEqual([
      2026, 10, 1, 0,
    ]);
    expect([end.getDate(), end.getHours(), end.getMinutes()]).toEqual([1, 23, 59]);
    expect(instantOfDay("2026-11-01", "start")).toMatch(/Z$/);
  });
});
