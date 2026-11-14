// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";

// The tab « VLAN » of a group (spec 021, task 17; 026, REQ-008-19; maquette E12, un
// groupe): « Créer un VLAN », one action of the bar above the VLAN, the same button while
// the group is empty and once it holds some, refused with its permission.

const G1 = "0b5e3c1a-0000-4000-8000-0000000000b1";
const V1 = "0b5e3c1a-0000-4000-8000-0000000000b2";

describe("the tab « VLAN » of a group, its action « Créer un VLAN »", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    setFetch(null);
    document.body.innerHTML = "";
  });

  /** The tab of G1 against a fake API whose VLAN are `vlans`; the creations asked. */
  async function mount(
    vlans: object[],
    blocked: { forbidden?: string } = {},
  ): Promise<{ root: HTMLElement; created: number[] }> {
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      const body = url.startsWith("/v1/vlans") ? { items: vlans, links: {} } : { items: [] };
      return new Response(JSON.stringify(body), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch);
    const created: number[] = [];
    const { default: GroupVlansTab } = await import("../src/views/record/GroupVlansTab.vue");
    const Host = defineComponent({
      setup: () => () =>
        h(GroupVlansTab, {
          id: G1,
          name: "campus",
          blocked,
          onCreate: () => created.push(created.length),
        }),
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/", component: Host },
        { path: "/vlans/:id", component: { render: () => null } },
      ],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp(RouterView);
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    return { root, created };
  }
  const create = (root: HTMLElement) =>
    [...root.querySelectorAll("button")].filter((b) => b.textContent?.includes("Créer un VLAN"));

  /** The bar of the tab, by its name. */
  const bar = (root: HTMLElement) =>
    root.querySelector('[role="toolbar"][aria-label="Actions des VLAN du groupe campus"]');

  it("is the one action of the bar while the group is empty", async () => {
    const { root, created } = await mount([]);
    await expect.poll(() => root.textContent).toContain("Aucun VLAN dans ce groupe");
    expect(bar(root)).not.toBeNull();
    expect(create(root)).toHaveLength(1);
    create(root)[0]!.click();
    expect(created).toHaveLength(1);
  });

  it("stays the same button once the group holds a VLAN, above them", async () => {
    let vlans: object[] = [];
    const { root, created } = await mount([]);
    await expect.poll(() => root.textContent).toContain("Aucun VLAN dans ce groupe");
    const before = create(root)[0]!;
    // The first VLAN created: the list read again shows it, under the same bar and button,
    // so the focus given back to the opener of the edit panel finds it.
    vlans = [
      { id: V1, group: { id: G1 }, vid: 100, name: "serveurs", status: "active", links: {} },
    ];
    setFetch(
      (async () =>
        new Response(JSON.stringify({ items: vlans, links: {} }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        })) as typeof fetch,
    );
    await queryClient.invalidateQueries();
    await expect.poll(() => root.textContent).toContain("serveurs");
    expect(root.textContent).not.toContain("Aucun VLAN dans ce groupe");
    expect(create(root)).toEqual([before]);
    expect(before.isConnected).toBe(true);
    before.click();
    expect(created).toHaveLength(1);
  });

  it("is refused with its permission, still visible", async () => {
    const { root, created } = await mount(
      [{ id: V1, group: { id: G1 }, vid: 100, name: "serveurs", status: "active", links: {} }],
      { forbidden: "core.vlan.edit" },
    );
    await expect.poll(() => root.textContent).toContain("serveurs");
    const button = create(root)[0]!;
    expect(button.getAttribute("aria-disabled")).toBe("true");
    // Its reason reachable: the permission said by the description of the button.
    const tip = document.getElementById(button.getAttribute("aria-describedby") ?? "");
    expect(tip?.textContent).toContain("core.vlan.edit");
    button.click();
    expect(created).toHaveLength(0);
  });
});
