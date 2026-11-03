// @vitest-environment happy-dom
import { ApiProblem, drafts, queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, defineComponent, h, nextTick, ref, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { useWriteAction, type PanelOutcome } from "../src/views/edit/action.ts";
import EditPanel from "../src/views/edit/EditPanel.vue";
import type { Refusal } from "../src/views/edit/inline.ts";
import { closeEdit, editRequest, openEdit } from "../src/views/edit/panel.ts";

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
    const operation = ref<Record<string, unknown>>({ op: "close", expected: "v1" });
    const name = ref("198.51.100.10");
    const refused: Refusal[] = [];
    let writing!: ReturnType<typeof useWriteAction>;
    const Host = defineComponent({
      setup() {
        writing = useWriteAction({
          rights: () => ["core.address.edit"],
          motive: () => `Clôture : ${name.value}`,
          submission: () => operation.value,
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
    return { writing, operation, name, refused };
  }

  it("keeps the key while nothing sent changes, a new one for a new operation or motive", async () => {
    const { writing, operation, name } = await mounted([
      "core.address.edit",
      "core.address.approval.self",
    ]);
    const first = writing.idempotencyKey();
    // A try again after a lost answer: the same submission, the same key.
    expect(writing.idempotencyKey()).toBe(first);
    // The version read again after a conflict: another operation, another key.
    operation.value = { op: "close", expected: "v2" };
    const second = writing.idempotencyKey();
    expect(second).not.toBe(first);
    // The cascade asked: another again.
    operation.value = { ...operation.value, cascade: true };
    const third = writing.idempotencyKey();
    expect(third).not.toBe(second);
    // The motive is sent too: one typed, or the prefilled one changing, a new key.
    name.value = "198.51.100.11";
    await nextTick();
    expect(writing.motive.value).toBe("Clôture : 198.51.100.11");
    const fourth = writing.idempotencyKey();
    expect(fourth).not.toBe(third);
    writing.setMotive("à la main");
    expect(writing.idempotencyKey()).not.toBe(fourth);
    // Typed, the motive is no longer prefilled.
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
    // Run again: the same sentence emptied first, so that the status says it again.
    const said = writing.notice.value;
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

describe("the edit panel, by its write action", () => {
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

  /** The panel creating a domain, the draft's write answered by `draft`. */
  async function panel(permissions: string[]) {
    setFetch((async (input: RequestInfo | URL) =>
      String(input).startsWith("/v1/labels")
        ? Response.json({ lang: "fr", default_lang: "fr", revision: "1", spaces: {} })
        : Response.json({ type: "about:blank", status: 404 }, { status: 404 })) as typeof fetch);
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
          request.value && request.value.mode !== "close"
            ? h(EditPanel, { request: request.value, key: request.value.serial })
            : h("p", "fermé");
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
    openEdit({ mode: "create", collection: "domains" });
    await expect.poll(() => root.querySelector("form")).not.toBeNull();
    return root;
  }
  const ctrlEnter = (root: HTMLElement) =>
    root
      .querySelector("form")!
      .dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, bubbles: true }));
  const status = (root: HTMLElement) =>
    root.querySelector(".k-edit-panel__notice")?.textContent ?? "";
  const main = (root: HTMLElement) => root.querySelector(".k-split-button__main")!;

  it("says the permission of an action refused at the keyboard, nothing sent", async () => {
    const root = await panel(["core.domain.read"]);
    ctrlEnter(root);
    await expect.poll(() => status(root)).toContain("il faut core.domain.edit");
    expect(host.added).toBe(0);
  });

  it("gives the button back when the draft's write is rejected, the panel open", async () => {
    const root = await panel(["core.domain.edit"]);
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
});
