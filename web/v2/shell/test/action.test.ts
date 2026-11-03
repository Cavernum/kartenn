// @vitest-environment happy-dom
import { ApiProblem, drafts, queryClient, type Submission } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, defineComponent, h, nextTick, ref, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { useWriteAction, type PanelOutcome } from "../src/views/edit/action.ts";
import ClosePanel from "../src/views/edit/ClosePanel.vue";
import EditPanel from "../src/views/edit/EditPanel.vue";
import type { Refusal } from "../src/views/edit/inline.ts";
import { closeEdit, editRequest, openEdit, type EditRequest } from "../src/views/edit/panel.ts";

// The write shared by the edit and close panels (task 021-12b; REQ-006-15, REQ-021-37): one
// idempotency key per submission, a new one as soon as what is sent changes; a write
// rejected outright gives the button back with its problem; an action refused, or off the
// present, says why by the status instead of silence.

const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";

/** The additions of the shell to the draft, counted; `rejection` set, rejected outright. */
const host = vi.hoisted(() => ({ rejection: null as unknown, added: 0, rejected: 0 }));
vi.mock("@kartenn/sdk/host", async (original) => {
  const actual = await original<typeof import("@kartenn/sdk/host")>();
  return {
    ...actual,
    addToActiveDraft: (operations: Parameters<typeof actual.addToActiveDraft>[0]) => {
      host.added += 1;
      if (host.rejection === null) return actual.addToActiveDraft(operations);
      host.rejected += 1;
      return Promise.reject(host.rejection);
    },
  };
});

describe("the write action of a panel", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    closeEdit();
  });

  async function mounted(permissions: string[], path = "/") {
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
    const name = ref("198.51.100.10");
    const refused: Refusal[] = [];
    let writing!: ReturnType<typeof useWriteAction>;
    const Host = defineComponent({
      setup() {
        writing = useWriteAction({
          rights: () => ["core.address.edit"],
          motive: () => `Clôture : ${name.value}`,
          words: {
            draft: "edit.close.toDraft",
            apply: "edit.close.apply",
            past: "edit.close.past",
          },
          refused: (r) => refused.push(r),
        });
        return () => h("p");
      },
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: Host }],
    });
    await router.push(path);
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({ render: () => h(RouterView) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    await nextTick();
    return { writing, name, refused };
  }

  it("keeps the key while the body sent is the same, a new one for another body", async () => {
    const { writing, name } = await mounted(["core.address.edit", "core.address.approval.self"]);
    const close = (over: Record<string, unknown> = {}, motive = "Clôture") => ({
      motive,
      operations: [{ op: "close", kind: "address", id: { id: P1 }, expected: "v1", ...over }],
    });
    const first = writing.idempotencyKey(close() as Submission);
    // A try again after a lost answer: the same body, the same key.
    expect(writing.idempotencyKey(close() as Submission)).toBe(first);
    // The version read again after a conflict: another operation, another key.
    const second = writing.idempotencyKey(close({ expected: "v2" }) as Submission);
    expect(second).not.toBe(first);
    // The cascade asked: another again.
    const third = writing.idempotencyKey(close({ expected: "v2", cascade: true }) as Submission);
    expect(third).not.toBe(second);
    // Another motive: another body, another key.
    expect(
      writing.idempotencyKey(close({ expected: "v2", cascade: true }, "à la main") as Submission),
    ).not.toBe(third);
    // The motive prefilled follows the object until one is typed.
    name.value = "198.51.100.11";
    await nextTick();
    expect(writing.motive.value).toBe("Clôture : 198.51.100.11");
    writing.setMotive("à la main");
    name.value = "198.51.100.12";
    await nextTick();
    expect(writing.motive.value).toBe("à la main");
  });

  it("gives the button back after a rejection, its problem kept, the panel open", async () => {
    const { writing, refused } = await mounted(["core.address.edit"]);
    openEdit({ mode: "close", collection: "addresses", id: P1 });
    const problem = new ApiProblem({ type: "about:blank", title: "Indisponible", status: 503 });
    expect(writing.begin("draft")).toBe(true);
    const pending = writing.write("l'adresse", () => Promise.reject(problem));
    expect(writing.busy.value).toBe(true);
    await pending;
    expect(writing.busy.value).toBe(false);
    expect(writing.refusal.value).toMatchObject({ kind: "failed", problem });
    expect(refused).toHaveLength(1);
    expect(editRequest().value).not.toBeNull();
    // A network failure: no problem to say, the button given back all the same.
    await writing.write("l'adresse", () => Promise.reject(new TypeError("réseau")));
    expect(writing.busy.value).toBe(false);
    expect(writing.refusal.value).toMatchObject({ kind: "failed", problem: null });
    // Written: said, the panel closed.
    await writing.write("l'adresse", async (): Promise<PanelOutcome> => ({ kind: "saved" }));
    expect(editRequest().value).toBeNull();
  });

  it("says an action refused by its permission, never silence, and nothing off the present", async () => {
    const { writing } = await mounted(["core.address.edit"]);
    // « Appliquer » without `.approval.self`: refused, named on the button and by the status.
    expect(writing.actions.value.find((a) => a.id === "apply")?.forbidden).toBe(
      "core.address.approval.self",
    );
    expect(writing.begin("apply")).toBe(false);
    expect(writing.notice.value).toContain("il faut core.address.approval.self");
    // Run again once shown: the same sentence emptied first, so that the status says it again.
    const said = writing.notice.value;
    await nextTick();
    expect(writing.begin("apply")).toBe(false);
    expect(writing.notice.value).toBeNull();
    await nextTick();
    expect(writing.notice.value).toBe(said);
    // The draft stays permitted; the sentence before is cleared.
    expect(writing.begin("draft")).toBe(true);
    expect(writing.notice.value).toBeNull();
    // A write in flight: a second run waits for it.
    writing.busy.value = true;
    expect(writing.begin("draft")).toBe(false);
  });

  it("writes nothing off the present, the draft included, and says so", async () => {
    const { writing } = await mounted(["core.address.edit"], "/?as_of=2026-01-01T00:00:00Z");
    expect(writing.begin("draft")).toBe(false);
    expect(writing.notice.value).toContain("revenez au présent pour clore");
  });
});

describe("the panels, by their write action", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    closeEdit();
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
    drafts.setActive(null);
    Object.assign(host, { rejection: null, added: 0, rejected: 0 });
  });

  interface Seen {
    method: string;
    url: string;
    body: Record<string, unknown> | null;
    key: string | null;
  }
  type Answer = { status?: number; body: unknown; etag?: string };

  /** The panel of `request` against a fake API answering by `answer`, its calls seen. */
  async function panel(
    permissions: string[],
    request: EditRequest = { mode: "create", collection: "domains" },
    answer: (seen: Seen) => Answer | undefined = () => undefined,
  ) {
    const seen: Seen[] = [];
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const call: Seen = {
        method: init?.method ?? "GET",
        url: String(input),
        body: init?.body ? JSON.parse(String(init.body)) : null,
        key: new Headers(init?.headers).get("Idempotency-Key"),
      };
      seen.push(call);
      const out = call.url.startsWith("/v1/labels")
        ? { body: { lang: "fr", default_lang: "fr", revision: "1", spaces: {} } }
        : (answer(call) ?? { status: 404, body: { type: "about:blank", status: 404 } });
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
    const Host = defineComponent({
      setup() {
        const request = editRequest();
        return () =>
          !request.value
            ? h("p", "fermé")
            : request.value.mode === "close"
              ? h(ClosePanel, { request: request.value, key: request.value.serial })
              : h(EditPanel, { request: request.value, key: request.value.serial });
      },
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: Host }],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({ render: () => h(RouterView) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    openEdit(request);
    await expect.poll(() => root.querySelector("form")).not.toBeNull();
    return { root, seen };
  }
  const writes = (seen: Seen[]) =>
    seen.filter((s) => s.method !== "GET" && !s.url.includes("simulate"));
  /** The entries of the menu of the split button, opened at the keyboard. */
  async function splitMenu(root: HTMLElement): Promise<HTMLElement[]> {
    const chevron = root.querySelector<HTMLButtonElement>(
      ".k-split-button [aria-haspopup='menu']",
    )!;
    chevron.focus();
    chevron.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    await expect.poll(() => document.querySelectorAll("[role='menuitem']").length).toBe(2);
    return [...document.querySelectorAll<HTMLElement>("[role='menuitem']")];
  }
  /** « Appliquer » chosen in the menu of the split button. */
  async function chooseApply(root: HTMLElement): Promise<void> {
    const [, apply] = await splitMenu(root);
    apply!.click();
    await expect.poll(() => main(root).textContent).toMatch(/Appliquer/);
  }
  const conflict = (id: string, version: string) => ({
    status: 422,
    body: {
      type: "urn:kartenn:problem:validation",
      title: "Changeset invalide",
      status: 422,
      violations: [
        {
          rule: "changeset.operation.conflict",
          severity: "error",
          detail: `${id} was modified: current version ${version}`,
          operation: 0,
          pointer: null,
          subjects: [id],
        },
      ],
    },
  });
  const ctrlEnter = (root: HTMLElement) =>
    root
      .querySelector("form")!
      .dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, bubbles: true }));
  const status = (root: HTMLElement) =>
    root.querySelector(".k-edit-panel__notice")?.textContent ?? "";
  const main = (root: HTMLElement) => root.querySelector(".k-split-button__main")!;

  it("says the permission of an action refused at the keyboard, nothing sent", async () => {
    const { root } = await panel(["core.domain.read"]);
    ctrlEnter(root);
    await expect.poll(() => status(root)).toContain("il faut core.domain.edit");
    expect(host.added).toBe(0);
  });

  it("gives the button back when the draft's write is rejected, the panel open", async () => {
    const { root } = await panel(["core.domain.edit"]);
    // The addition to the draft rejected outright, not answered with a refusal.
    host.rejection = new ApiProblem({ type: "about:blank", title: "Indisponible", status: 503 });
    const slug = root.querySelector<HTMLInputElement>(".k-edit-panel__field input")!;
    slug.value = "essai";
    slug.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
    ctrlEnter(root);
    await expect.poll(() => host.rejected).toBe(1);
    await expect.poll(() => main(root).getAttribute("aria-busy")).toBeNull();
    expect(root.querySelector("[role='alert']")?.textContent).toContain("Indisponible");
    expect(editRequest().value).not.toBeNull();
  });

  const A1 = "0b5e3c1a-0000-4000-8000-0000000000e1";
  const E1 = "0b5e3c1a-0000-4000-8000-0000000000f1";
  const E2 = "0b5e3c1a-0000-4000-8000-0000000000f2";
  const V1 = "0b5e3c1a-0000-4000-8000-0000000000c1";
  const V2 = "0b5e3c1a-0000-4000-8000-0000000000c2";
  const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";
  const SIMULATED = {
    valid: true,
    violations: [],
    diff: [],
    sample: [],
    cascaded: [],
    sections: [],
    placements: [],
    placements_truncated: false,
  };
  const APPLIED = { status: 201, body: { changeset: { id: D1 }, warnings: [], cascaded: [] } };
  const operationOf = (call: Seen) => (call.body?.operations as Record<string, unknown>[])[0];

  it("replays the same key after a 503, takes another once a conflict is read again", async () => {
    let submits = 0;
    const { root, seen } = await panel(
      ["core.address.edit", "core.address.approval.self"],
      { mode: "close", collection: "addresses", id: A1 },
      (call) => {
        if (call.url === `/v1/addresses/${A1}`) {
          // Written by another between the read and the second try: V2 once refused for it.
          const version = submits < 2 ? V1 : V2;
          return {
            body: { id: A1, value: "198.51.100.10", version, domain: { id: D1, slug: "essai" } },
            etag: `"${version}"`,
          };
        }
        if (call.url.startsWith("/v1/changesets?simulate=true")) return { body: SIMULATED };
        if (call.method === "POST" && call.url === "/v1/changesets?approval=self") {
          submits += 1;
          if (submits === 1) {
            return {
              status: 503,
              body: { type: "about:blank", title: "Indisponible", status: 503 },
            };
          }
          return submits === 2 ? conflict(A1, V2) : APPLIED;
        }
        return undefined;
      },
    );
    await expect.poll(() => root.textContent).toContain("La clôture passe les règles");
    await chooseApply(root);
    ctrlEnter(root);
    await expect.poll(() => root.textContent).toContain("La clôture n'a pas abouti");
    // Tried again, nothing changed: the same body under the same key (an idempotent replay).
    ctrlEnter(root);
    await expect.poll(() => root.textContent).toContain("L'objet a changé depuis sa lecture");
    // Read again after the conflict: the version of the closure is the new one.
    await expect.poll(() => seen.filter((s) => s.url === `/v1/addresses/${A1}`).length).toBe(2);
    await nextTick();
    ctrlEnter(root);
    await expect.poll(() => submits).toBe(3);
    const [lost, again, reread] = writes(seen);
    expect(again!.key).toBe(lost!.key);
    expect(operationOf(again!)).toMatchObject({ expected: V1 });
    expect(operationOf(reread!)).toMatchObject({ expected: V2 });
    expect(reread!.key).not.toBeNull();
    expect(reread!.key).not.toBe(lost!.key);
    await expect.poll(() => editRequest().value).toBeNull();
  });

  it("moves applied: after a conflict, the node read again, sent at its new version under a new key", async () => {
    let submits = 0;
    const entity = (id: string, slug: string, version: string) => ({
      id,
      slug,
      parent: null,
      version,
      href: `/v1/entities/${id}`,
    });
    const { root, seen } = await panel(
      ["core.entity.edit", "core.entity.approval.self"],
      { mode: "move", collection: "entities", id: E1 },
      (call) => {
        const version = submits === 0 ? V1 : V2;
        if (call.url === `/v1/entities/${E1}`) {
          return { body: entity(E1, "essai-a", version), etag: `"${version}"` };
        }
        if (call.method === "GET" && call.url.startsWith("/v1/entities")) {
          return {
            body: { items: [entity(E1, "essai-a", version), entity(E2, "essai-b", V1)], links: {} },
          };
        }
        if (call.method === "POST" && call.url === "/v1/changesets?approval=self") {
          submits += 1;
          return submits === 1 ? conflict(E1, V2) : APPLIED;
        }
        return undefined;
      },
    );
    // The target taken from the list of the valid ones.
    const parent = root.querySelector<HTMLInputElement>("input[role='combobox']")!;
    parent.value = "essai-b";
    parent.dispatchEvent(new Event("input", { bubbles: true }));
    await expect
      .poll(() => [...document.querySelectorAll<HTMLElement>("[role='option']")].length)
      .toBeGreaterThan(0);
    [...document.querySelectorAll<HTMLElement>("[role='option']")]
      .find((o) => o.textContent?.includes("essai-b"))!
      .click();
    await nextTick();
    await chooseApply(root);
    ctrlEnter(root);
    await expect.poll(() => submits).toBe(1);
    // Refused for the version it was read at: the node read again, at V2.
    await expect.poll(() => seen.filter((s) => s.url === `/v1/entities/${E1}`).length).toBe(2);
    await nextTick();
    ctrlEnter(root);
    await expect.poll(() => submits).toBe(2);
    const [first, second] = writes(seen);
    expect(operationOf(first!)).toMatchObject({
      op: "move_entity",
      parent: { id: E2 },
      expected: V1,
    });
    expect(operationOf(second!)).toMatchObject({
      op: "move_entity",
      parent: { id: E2 },
      expected: V2,
    });
    expect(second!.key).not.toBeNull();
    expect(second!.key).not.toBe(first!.key);
    await expect.poll(() => editRequest().value).toBeNull();
  });
});
