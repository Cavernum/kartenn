// @vitest-environment happy-dom
import { queryClient, type Operation } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, nextTick, ref, watch, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import DraftOperations from "../src/draft/DraftOperations.vue";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import EditPanel from "../src/views/edit/EditPanel.vue";
import { draftedCreations, EDITS, editOfOperation } from "../src/views/edit/fields.ts";
import { closeEdit, editRequest, lineToReturn } from "../src/views/edit/panel.ts";

// « Modifier » of an operation of a draft (task 021-14l, Q-021-17; REQ-021-11, 36): the
// edit panel of its kind opened prefilled from the operation — a creation, or the
// modification over the object read —, which replaces it at its place under `If-Match` of
// the draft as its line was read; the draft simulated again at its new version; the focus
// back on the line once its tab shows it again. An operation no panel writes: refused,
// said why.

const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";
const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";
const P2 = "0b5e3c1a-0000-4000-8000-0000000000a2";
const V1 = "7";

describe("the form an operation of a draft is made by", () => {
  it("a creation by its members, renamed and flags kept; a modification over its object", () => {
    expect(
      editOfOperation({
        op: "create_prefix",
        domain: { id: P2 },
        prefix: "192.0.2.0/24",
        nature: "network",
        point_to_point: false,
        anycast: true,
        local: 0,
      } as Operation),
    ).toEqual({
      collection: "prefixes",
      mode: "create",
      id: null,
      values: {
        domain: P2,
        value: "192.0.2.0/24",
        nature: "network",
        point_to_point: false,
        anycast: true,
      },
    });
    expect(
      editOfOperation({
        op: "update_prefix",
        id: { id: P1 },
        prefix: "192.0.2.0/25",
        expected: V1,
      } as Operation),
    ).toEqual({ collection: "prefixes", mode: "edit", id: P1, values: { value: "192.0.2.0/25" } });
    // A rename by its kind; an anchor as the form holds it.
    expect(
      editOfOperation({ op: "rename", kind: "site", id: { id: P1 }, slug: "nord" } as Operation),
    ).toMatchObject({ collection: "sites", mode: "edit", values: { slug: "nord" } });
    expect(
      editOfOperation({
        op: "update_vlan_group",
        id: { id: P1 },
        anchor: { site: { id: P2 } },
        vid_max: 200,
      } as Operation),
    ).toMatchObject({
      collection: "vlan-groups",
      values: { anchor: `site:${P2}`, vid_max: "200" },
    });
  });

  it("keeps a reference to an object of the draft by its local name, written back as such", () => {
    const entity = editOfOperation({
      op: "create_entity",
      slug: "nord",
      parent: { local: 0 },
    } as Operation)!;
    expect(entity.values).toEqual({ slug: "nord", parent: "local:0" });
    expect(EDITS.entities.create(entity.values)).toEqual({
      op: "create_entity",
      slug: "nord",
      parent: { local: 0 },
    });
    const group = editOfOperation({
      op: "create_vlan_group",
      slug: "g",
      anchor: { entity: { local: 1 } },
    } as Operation)!;
    expect(group.values.anchor).toBe("entity:local:1");
    expect(EDITS["vlan-groups"].create(group.values)).toMatchObject({
      anchor: { entity: { local: 1 } },
    });
    // The objects a reference of the third operation may name: those created before it.
    const operations = [
      { op: "create_domain", slug: "labo", local: 0 },
      { op: "create_entity", slug: "nord", local: 1 },
      { op: "create_site", slug: "sud", local: 2 },
    ] as Operation[];
    expect(draftedCreations(operations, 2).map((d) => [d.collection, d.local, d.index])).toEqual([
      ["domains", 0, 0],
      ["entities", 1, 1],
    ]);
  });

  it("none for an operation no panel writes: a closure, a move, under within", () => {
    const close = { op: "close", kind: "prefix", id: { id: P1 } } as unknown as Operation;
    expect(editOfOperation(close)).toBeNull();
    expect(editOfOperation({ op: "move_entity", id: { id: P1 } } as Operation)).toBeNull();
    expect(editOfOperation({ op: "within", operation: close } as unknown as Operation)).toBeNull();
  });
});

describe("« Modifier » of an operation of a draft", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    closeEdit(false);
    lineToReturn().value = null;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
  });

  interface Seen {
    method: string;
    url: string;
    body: Record<string, unknown> | null;
    ifMatch: string | null;
  }
  const draft = (operations: unknown[], version = 1) => ({
    id: D1,
    title: "Brouillon du jour",
    author: "alice",
    href: `/v1/drafts/${D1}`,
    created_at: "2026-11-01T10:00:00Z",
    updated_at: "2026-11-01T10:00:00Z",
    version,
    operation_count: operations.length,
    operations,
    reason: {},
  });
  const valid = { valid: true, violations: [], diff: [], sample: [], cascaded: [], sections: [] };
  const CREATE = { op: "create_domain", slug: "labo", local: 0 };
  const UPDATE = { op: "update_prefix", id: { id: P1 }, nature: "container", expected: "6" };
  const CLOSE = { op: "close", kind: "prefix", id: { id: P2 } };
  const PREFIX = {
    id: P1,
    value: "192.0.2.0/24",
    nature: "network",
    point_to_point: false,
    anycast: false,
    version: V1,
  };

  /**
   * The draft tab and the Détail tab as the frame shows them, one at a time: the edit
   * asked takes the place of the draft's operations, which come back once it closes.
   */
  async function mounted(
    answer: (seen: Seen, version: number) => { status?: number; body: unknown; etag?: string },
  ) {
    const seen: Seen[] = [];
    let version = 1;
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const call: Seen = {
        method: init?.method ?? "GET",
        url: String(input),
        body: init?.body ? JSON.parse(String(init.body)) : null,
        ifMatch: new Headers(init?.headers).get("If-Match"),
      };
      seen.push(call);
      if (call.method === "PATCH") version += 1;
      const out = call.url.startsWith("/v1/labels")
        ? { body: { lang: "fr", default_lang: "fr", revision: "1", spaces: {} } }
        : answer(call, version);
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
            global_permissions: ["core.prefix.edit", "core.domain.edit"],
            bindings: [],
          } as unknown as Me)
        : null,
    );
    const Host = defineComponent({
      setup() {
        const request = editRequest();
        const tab = ref("draft");
        watch(request, (shown) => shown && (tab.value = "detail"));
        watch(lineToReturn(), (line) => line && (tab.value = "draft"));
        return () =>
          tab.value === "detail" && request.value && request.value.mode !== "close"
            ? h(EditPanel, { request: request.value, key: request.value.serial })
            : h(DraftOperations, { id: D1 });
      },
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/", component: Host },
        // The link to « Mes brouillons » of the tab, never followed here.
        { path: "/drafts", redirect: "/" },
      ],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({ render: () => h(RouterView) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    await expect
      .poll(() => root.querySelectorAll("[data-k-modify]").length, { timeout: 3000 })
      .toBe(3);
    return { root, seen };
  }
  const modify = (root: HTMLElement, index: number) =>
    root.querySelectorAll<HTMLButtonElement>("[data-k-modify]")[index]!;
  const field = (root: HTMLElement, label: string) => {
    const id = [...root.querySelectorAll("label")].find(
      // Its words, before the mark of a required field.
      (l) => l.textContent?.trim().startsWith(label),
    )!.htmlFor;
    return document.getElementById(id) as HTMLInputElement;
  };
  const type = async (input: HTMLInputElement, text: string) => {
    input.value = text;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
  };
  const replaceButton = (root: HTMLElement) =>
    [...root.querySelectorAll<HTMLButtonElement>("button")].find((b) =>
      b.textContent?.includes("Remplacer dans le brouillon"),
    )!;
  const status = (root: HTMLElement) =>
    root.querySelector(".k-edit-panel__notice")?.textContent ?? "";
  /** The prefix read now (version 7), and its history: version 6 had another value. */
  const prefixRead = (call: Seen) => {
    if (call.url.startsWith(`/v1/prefixes/${P1}/history`)) {
      const then = { ...PREFIX, value: "192.0.2.0/23", version: "6" };
      return { body: { items: [{ object: PREFIX }, { object: then }] } };
    }
    if (call.url.startsWith(`/v1/prefixes/${P1}`)) return { body: PREFIX, etag: `"${V1}"` };
    return undefined;
  };

  it("replaces a modification at its place under If-Match, simulated again, the focus back on its line", async () => {
    const operations: unknown[] = [CREATE, UPDATE, CLOSE];
    const { root, seen } = await mounted((call, version) => {
      if (call.url.endsWith("/simulation")) return { body: valid };
      const read = prefixRead(call);
      if (read) return read;
      if (call.method === "PATCH") {
        operations.splice(0, 3, ...(call.body!.operations as unknown[]));
      }
      return { body: draft(operations, version), etag: `"${version}"` };
    });
    // The closure: no panel writes it, refused with why.
    expect(modify(root, 2).getAttribute("aria-disabled")).toBe("true");
    expect(modify(root, 2).getAttribute("aria-label")).toBe(`Modifier l'opération 3 : close ${P2}`);
    // Focused by its click, as a browser does: the button leaves with its tab.
    modify(root, 1).focus();
    modify(root, 1).click();
    await expect.poll(() => root.querySelector("form")).not.toBeNull();
    await expect
      .poll(() => root.querySelector("h2")?.textContent)
      .toBe(`Modifier l'opération 2 : update_prefix ${P1}`);
    expect(root.textContent).toContain("remplacée à sa place dans « Brouillon du jour »");
    // Prefilled: the operation's own values over the object read.
    expect(field(root, "Valeur").value).toBe("192.0.2.0/24");
    expect(root.querySelector("form")!.textContent).toContain("Conteneur");
    // Written on version 6, the object read at 7: said, with the field that moved since.
    await expect
      .poll(() => root.textContent)
      .toContain("Valeur : 192.0.2.0/23 alors, 192.0.2.0/24 maintenant");
    expect(root.textContent).toContain("L'objet a changé depuis cette opération");
    // Nothing typed: said, nothing sent.
    replaceButton(root).click();
    await expect.poll(() => status(root)).toContain("Rien n'a changé");
    expect(seen.some((s) => s.method === "PATCH")).toBe(false);
    await type(field(root, "Valeur"), "192.0.2.0/25");
    const simulations = () => seen.filter((s) => s.url.endsWith("/simulation")).length;
    const before = simulations();
    replaceButton(root).click();
    await expect.poll(() => seen.some((s) => s.method === "PATCH")).toBe(true);
    const patch = seen.find((s) => s.method === "PATCH")!;
    expect(patch.url).toBe(`/v1/drafts/${D1}`);
    expect(patch.ifMatch).toBe('"1"');
    // At its place, the others kept; still on the version it was written on (Q-021-20).
    expect(patch.body).toEqual({
      operations: [
        CREATE,
        {
          op: "update_prefix",
          id: { id: P1 },
          prefix: "192.0.2.0/25",
          nature: "container",
          expected: "6",
        },
        CLOSE,
      ],
    });
    // The panel closed, the draft tab back, the focus on the line's « Modifier ».
    await expect.poll(() => root.querySelectorAll("[data-k-modify]").length).toBe(3);
    await expect.poll(() => document.activeElement).toBe(modify(root, 1));
    expect(root.textContent).toContain("192.0.2.0/25");
    // The draft at its new version, simulated again.
    await expect.poll(simulations, { timeout: 3000 }).toBeGreaterThan(before);
  });

  it("replaces a creation keeping its local name; a draft changed elsewhere: said, nothing replaced", async () => {
    let refuse = false;
    const operations: unknown[] = [CREATE, UPDATE, CLOSE];
    const { root, seen } = await mounted((call, version) => {
      if (call.url.endsWith("/simulation")) return { body: valid };
      if (call.method === "PATCH" && refuse) {
        return {
          status: 412,
          body: { type: "urn:kartenn:problem:precondition-failed", title: "", status: 412 },
        };
      }
      if (call.method === "PATCH") {
        operations.splice(0, 3, ...(call.body!.operations as unknown[]));
      }
      // Read again after the 412: another tab took the creation out.
      const held = refuse ? [UPDATE, CLOSE, CLOSE] : operations;
      return { body: draft(held, version), etag: `"${version}"` };
    });
    // A click that leaves the focus on `body` (a browser that does not focus a button).
    modify(root, 0).click();
    await expect.poll(() => root.querySelector("form")).not.toBeNull();
    expect(field(root, "Clé").value).toBe("labo");
    await type(field(root, "Clé"), "labo-nord");
    replaceButton(root).click();
    await expect.poll(() => seen.some((s) => s.method === "PATCH")).toBe(true);
    expect(seen.find((s) => s.method === "PATCH")!.body).toEqual({
      operations: [{ op: "create_domain", slug: "labo-nord", local: 0 }, UPDATE, CLOSE],
    });
    await expect.poll(() => document.activeElement).toBe(modify(root, 0));

    refuse = true;
    modify(root, 0).click();
    await expect.poll(() => root.querySelector("form")).not.toBeNull();
    await type(field(root, "Clé"), "labo-sud");
    replaceButton(root).click();
    await expect
      .poll(() => root.textContent)
      .toContain("Le brouillon a changé depuis votre lecture ; relu");
    expect(root.textContent).toContain("Rien n'est remplacé : l'opération n'est plus à sa place");
    // Read again once; its place holds another operation: not tried again.
    expect(seen.filter((s) => s.method === "PATCH")).toHaveLength(2);
    expect(editRequest().value).not.toBeNull();
  });

  it("« Reprendre sur la version actuelle » writes the operation on the version read now", async () => {
    const operations: unknown[] = [CREATE, UPDATE, CLOSE];
    const { root, seen } = await mounted((call, version) => {
      if (call.url.endsWith("/simulation")) return { body: valid };
      const read = prefixRead(call);
      if (read) return read;
      if (call.method === "PATCH") {
        operations.splice(0, 3, ...(call.body!.operations as unknown[]));
      }
      return { body: draft(operations, version), etag: `"${version}"` };
    });
    modify(root, 1).click();
    const rebase = () =>
      [...root.querySelectorAll<HTMLButtonElement>("button")].find((b) =>
        b.textContent?.includes("Reprendre sur la version actuelle"),
      );
    await expect.poll(rebase).toBeDefined();
    rebase()!.click();
    await expect.poll(() => status(root)).toContain("Reprise sur la version 7");
    expect(rebase()).toBeUndefined();
    // Nothing typed: taken up all the same, what differs from the object as it is.
    replaceButton(root).click();
    await expect.poll(() => seen.some((s) => s.method === "PATCH")).toBe(true);
    expect(seen.find((s) => s.method === "PATCH")!.body).toEqual({
      operations: [
        CREATE,
        { op: "update_prefix", id: { id: P1 }, nature: "container", expected: V1 },
        CLOSE,
      ],
    });
  });

  it("replaces a creation naming an object of the draft: its local name kept, others offered", async () => {
    const prefix = {
      op: "create_prefix",
      domain: { local: 0 },
      prefix: "198.51.100.0/24",
      nature: "network",
      point_to_point: false,
      anycast: false,
    };
    const operations: unknown[] = [CREATE, prefix, CLOSE];
    const { root, seen } = await mounted((call, version) => {
      if (call.url.endsWith("/simulation")) return { body: valid };
      if (call.url.startsWith("/v1/domains")) return { body: { items: [] } };
      if (call.method === "PATCH") {
        operations.splice(0, 3, ...(call.body!.operations as unknown[]));
      }
      return { body: draft(operations, version), etag: `"${version}"` };
    });
    modify(root, 1).click();
    await expect.poll(() => root.querySelector("form")).not.toBeNull();
    // The domain shown by what its operation names, never empty.
    await expect.poll(() => field(root, "Domaine").value).toBe("labo");
    await type(field(root, "Valeur"), "198.51.100.0/25");
    replaceButton(root).click();
    await expect.poll(() => seen.some((s) => s.method === "PATCH")).toBe(true);
    expect(seen.find((s) => s.method === "PATCH")!.body).toEqual({
      operations: [CREATE, { ...prefix, prefix: "198.51.100.0/25" }, CLOSE],
    });
  });
});
