// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import GroupDetail from "../src/admin/GroupDetail.vue";
import { frameRoutes } from "../src/frame/routes.ts";
import { frameWidth, setFrameWidth } from "../src/frame/width.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { offered, play } from "../src/shortcuts/view.ts";
import { timeOf, writesOf } from "../src/time/index.ts";

// The administration under 1024 px wide (task 021-21e; REQ-021-29, Q-021-12): read, each
// write refused at its point of action with the width, after the permission; a dialog left
// open as the window narrows keeps its input and sends nothing, the refusal said in it;
// widened, the write goes.

const NARROW = "Modification à partir de 1024 px de large : élargissez la fenêtre pour modifier.";

const GROUP = {
  id: "g",
  href: "/v1/groups/g",
  key: "supervision",
  labels: { fr: "Équipe supervision" },
  builtin: false,
  members: 0,
  bindings: 0,
};

interface Seen {
  method: string;
  url: string;
}

describe("the administration under 1024 px", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
    setFrameWidth("wide");
    document.body.innerHTML = "";
  });

  async function mounted(at: string, permissions: string[]) {
    const seen: Seen[] = [];
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const call = { method: init?.method ?? "GET", url: String(input) };
      seen.push(call);
      if (call.url.startsWith("/v1/labels")) {
        return new Response(
          JSON.stringify({ lang: "fr", default_lang: "fr", revision: "1", spaces: {} }),
          { headers: { "Content-Type": "application/json" } },
        );
      }
      const body =
        call.method === "POST"
          ? { id: "d1", title: "", operations: [], created_at: "2026-01-01T00:00:00Z" }
          : call.url.split("?")[0] === "/v1/groups/g"
            ? GROUP
            : { items: [], links: {} };
      return new Response(JSON.stringify(body), {
        status: call.method === "POST" ? 201 : 200,
        headers: { "Content-Type": "application/json", ETag: '"1"' },
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
        // The Détail tab of the right panel, as the frame fills it (`meta.detail`).
        { path: "/detail/:id", component: GroupDetail, props: true },
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

  const button = (within: ParentNode, name: string) =>
    [...within.querySelectorAll<HTMLButtonElement>("button")].find((b) =>
      b.textContent?.trim().startsWith(name),
    );
  const reasonOf = (element: Element) =>
    document.getElementById(element.getAttribute("aria-describedby") ?? "")?.textContent;
  const writesSent = (seen: Seen[]) => seen.filter((s) => s.method !== "GET");
  const typed = (input: HTMLInputElement, text: string) => {
    input.value = text;
    input.dispatchEvent(new Event("input", { bubbles: true }));
  };

  it("refuses « Créer » with the width, after the permission lacking", async () => {
    const { root } = await mounted("/admin/groups", ["core.rbac.admin", "core.rbac.read"]);
    await expect.poll(() => button(root, "Créer")).toBeDefined();
    expect(button(root, "Créer")!.getAttribute("aria-disabled")).toBeNull();
    setFrameWidth("narrow");
    await nextTick();
    expect(button(root, "Créer")!.getAttribute("aria-disabled")).toBe("true");
    expect(reasonOf(button(root, "Créer")!)).toContain(NARROW);
    setFrameWidth("standard");
    await nextTick();
    expect(button(root, "Créer")!.getAttribute("aria-disabled")).toBeNull();
  });

  it("keeps the permission lacking as the reason, at any width", async () => {
    const { root } = await mounted("/admin/groups", ["core.rbac.read"]);
    await expect.poll(() => button(root, "Créer")).toBeDefined();
    setFrameWidth("narrow");
    await nextTick();
    expect(reasonOf(button(root, "Créer")!)).toContain("core.rbac.admin");
    expect(reasonOf(button(root, "Créer")!)).not.toContain(NARROW);
  });

  it("a dialog left open as the window narrows keeps its input, sends nothing", async () => {
    const { seen, root } = await mounted("/admin/groups", ["core.rbac.admin", "core.rbac.read"]);
    await expect.poll(() => button(root, "Créer")).toBeDefined();
    button(root, "Créer")!.click();
    await expect.poll(() => document.querySelector("[role='dialog'] input")).not.toBeNull();
    const dialog = document.querySelector<HTMLElement>("[role='dialog']")!;
    const input = dialog.querySelector<HTMLInputElement>("input")!;
    typed(input, "supervision");
    const send = () => button(dialog, "Ajouter au brouillon")!;
    expect(send()).toBeDefined();

    setFrameWidth("narrow");
    await nextTick();
    expect(reasonOf(send())).toContain(NARROW);
    send().click();
    dialog.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    await nextTick();
    expect(writesSent(seen)).toEqual([]);
    expect(input.value).toBe("supervision");
    expect(document.querySelector("[role='dialog']")).not.toBeNull();
    expect(dialog.querySelector("[role='status']")?.textContent).toContain(NARROW);

    setFrameWidth("standard");
    await nextTick();
    expect(dialog.querySelector("[role='status']")?.textContent ?? "").not.toContain(NARROW);
    send().click();
    await expect.poll(() => writesSent(seen).map((s) => s.method)).toContain("POST");
  });

  // The lasting reason first (the permission lacking), then the gate: the key and the
  // palette say what the button says (review of 21e), in the four cases.
  const ADMIN = ["core.rbac.admin", "core.rbac.read"];
  const READER = ["core.rbac.read"];
  const LACKING = "Il faut core.rbac.admin";
  const CASES = [
    { name: "nothing refused", permissions: ADMIN, width: "wide", says: null },
    { name: "the right alone", permissions: READER, width: "wide", says: LACKING },
    { name: "the width alone", permissions: ADMIN, width: "narrow", says: NARROW },
    { name: "both", permissions: READER, width: "narrow", says: LACKING },
  ] as const;
  const t = (key: string) => createShellI18n("fr").global.t(key);
  const writesNow = () => writesOf(timeOf({}), frameWidth.value);
  const statusOf = (root: HTMLElement) =>
    root.querySelector("span.k-visually-hidden[role='status']")?.textContent ?? "";

  for (const c of CASES) {
    it(`« Créer » and \`c\` of the groups say the same, ${c.name}`, async () => {
      const { root } = await mounted("/admin/groups", [...c.permissions]);
      await expect.poll(() => button(root, "Créer")).toBeDefined();
      setFrameWidth(c.width);
      await nextTick();
      const create = button(root, "Créer")!;
      expect(play("create", writesNow(), t)).toBe(true);
      await nextTick();
      await nextTick();
      if (c.says === null) {
        expect(create.getAttribute("aria-disabled")).toBeNull();
        await expect.poll(() => document.querySelector("[role='dialog']")).not.toBeNull();
      } else {
        expect(reasonOf(create)).toContain(c.says);
        expect(statusOf(root)).toBe(c.says);
        expect(document.querySelector("[role='dialog']")).toBeNull();
      }
    });

    it(`« Modifier » and \`e\` of a group say the same, ${c.name}`, async () => {
      const { root, router } = await mounted("/admin/groups", [...c.permissions]);
      await expect.poll(() => button(root, "Créer")).toBeDefined();
      await router.push("/detail/g");
      await expect.poll(() => button(root, "Modifier")).toBeDefined();
      setFrameWidth(c.width);
      await nextTick();
      const edit = button(root, "Modifier")!;
      expect(play("edit", writesNow(), t)).toBe(true);
      await nextTick();
      await nextTick();
      if (c.says === null) {
        expect(edit.getAttribute("aria-disabled")).toBeNull();
        await expect.poll(() => document.querySelector("[role='dialog']")).not.toBeNull();
      } else {
        expect(reasonOf(edit)).toContain(c.says);
        expect(statusOf(root)).toBe(c.says);
        expect(document.querySelector("[role='dialog']")).toBeNull();
      }
    });
  }

  it("keeps « Modifier le groupe » in the palette when the width alone holds it", async () => {
    const { root, router } = await mounted("/admin/groups", ADMIN);
    await expect.poll(() => button(root, "Créer")).toBeDefined();
    await router.push("/detail/g");
    await expect.poll(() => button(root, "Modifier")).toBeDefined();
    const command = offered("edit", timeOf({}), "narrow", t);
    expect(command?.label()).toBe("Modifier le groupe supervision");
    command!.run();
    await nextTick();
    await nextTick();
    expect(statusOf(root)).toBe(NARROW);
    expect(document.querySelector("[role='dialog']")).toBeNull();
    expect(offered("edit", timeOf({}), "wide", t)?.label()).toBe("Modifier le groupe supervision");
  });

  it("takes « Modifier le groupe » out of the palette when the right lacks", async () => {
    const { root, router } = await mounted("/admin/groups", READER);
    await expect.poll(() => button(root, "Créer")).toBeDefined();
    await router.push("/detail/g");
    await expect.poll(() => button(root, "Modifier")).toBeDefined();
    expect(offered("edit", timeOf({}), "narrow", t)).toBeNull();
    expect(offered("edit", timeOf({}), "wide", t)).toBeNull();
  });
});
