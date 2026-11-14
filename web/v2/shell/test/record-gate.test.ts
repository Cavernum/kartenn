// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, h, nextTick, type App, type Component } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { setFrameWidth } from "../src/frame/width.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import AnomalyGrid from "../src/views/anomalies/AnomalyGrid.vue";
import { closeEdit, editRequest } from "../src/views/edit/panel.ts";
import { NETWORK_VLAN_EDITS } from "../src/views/edit/vlan.ts";
import NetworkVlanTab from "../src/views/record/NetworkVlanTab.vue";

// Editing from 1024 px wide on the record, the anomalies and the Topography (task 021-21f;
// REQ-021-29, Q-021-12): under it, « Modifier » of a record, the « + » of the tree, «
// Acquitter » of an anomaly and « Poser le VLAN » of a network are refused with the width,
// the permission missing said first; the gate read again as each acts, a dialog or a form
// left open keeping what it holds; wide again, each acts.

const NARROW = "Modification à partir de 1024 px de large : élargissez la fenêtre pour modifier.";
const DOMAIN = "0b5e3c1a-0000-4000-8000-0000000000d1";
const PREFIX = "0b5e3c1a-0000-4000-8000-000000000101";
const ANOMALY = "0b5e3c1a-0000-4000-8000-0000000000a1";
const page = (items: object[]) => ({ items, links: {} });

interface Seen {
  method: string;
  url: string;
}

let app: App | undefined;
afterEach(() => {
  closeEdit(false);
  app?.unmount();
  app = undefined;
  queryClient.clear();
  resetContext();
  setFetch(null);
  setFrameWidth("wide");
  document.body.replaceChildren();
});

/** The shell at `at` (or `component` in it), for a reader with `permissions`. */
async function mounted(
  at: string,
  permissions: string[],
  answer: (url: string) => object,
  component?: Component,
  props: Record<string, unknown> = {},
) {
  const seen: Seen[] = [];
  setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input);
    seen.push({ method: init?.method ?? "GET", url });
    const body = url.startsWith("/v1/labels")
      ? { lang: "fr", default_lang: "fr", revision: "1", spaces: {} }
      : answer(url);
    return new Response(JSON.stringify(body), {
      headers: { "Content-Type": "application/json", ETag: '"v1"' },
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
  const root = document.body.appendChild(document.createElement("div"));
  app = createApp({ render: () => (component ? h(component, props) : h(RouterView)) });
  app.use(createShellI18n("fr")).use(router);
  app.mount(root);
  await router.isReady();
  const writes = () => seen.filter((s) => s.method !== "GET");
  return { root, writes };
}

/** What describes an element: its tooltip, the reason of a refused action. */
const described = (element: Element) =>
  (element.getAttribute("aria-describedby")?.split(" ") ?? [])
    .map((id) => document.getElementById(id)?.textContent ?? "")
    .join(" ");
const button = (root: ParentNode, name: string) =>
  [...root.querySelectorAll<HTMLButtonElement>("button")].find((b) =>
    b.textContent?.trim().startsWith(name),
  );
const typed = (input: HTMLInputElement, text: string) => {
  input.value = text;
  input.dispatchEvent(new Event("input", { bubbles: true }));
};
const submit = (form: HTMLFormElement) =>
  form.dispatchEvent(new Event("submit", { cancelable: true }));

describe("the record under 1024 px (task 021-21f)", () => {
  const record = (url: string) =>
    url === `/v1/prefixes/${PREFIX}`
      ? {
          id: PREFIX,
          value: "192.0.2.0/24",
          nature: "network",
          version: "1",
          links: { self: `/v1/prefixes/${PREFIX}` },
        }
      : url.startsWith(`/v1/prefixes/${PREFIX}/properties`)
        ? { properties: {}, masked: [], explanation: {} }
        : page([]);

  it("« Modifier » says the width, opens nothing; wide again, it opens the edit panel", async () => {
    const { root } = await mounted(
      `/prefixes/${PREFIX}`,
      ["core.prefix.read", "core.prefix.edit"],
      record,
    );
    await expect.poll(() => button(root, "Modifier")).toBeDefined();
    setFrameWidth("narrow");
    await nextTick();
    const modify = button(root, "Modifier")!;
    expect(modify.getAttribute("aria-disabled")).toBe("true");
    expect(described(modify)).toContain(NARROW);
    modify.click();
    expect(editRequest().value).toBeNull();

    setFrameWidth("standard");
    await nextTick();
    expect(modify.getAttribute("aria-disabled")).not.toBe("true");
    modify.click();
    expect(editRequest().value).toMatchObject({ mode: "edit", collection: "prefixes" });
  });

  it("the permission missing is said before the width", async () => {
    const { root } = await mounted(`/prefixes/${PREFIX}`, ["core.prefix.read"], record);
    await expect.poll(() => button(root, "Modifier")).toBeDefined();
    setFrameWidth("compact");
    await nextTick();
    const modify = button(root, "Modifier")!;
    expect(described(modify)).toContain("core.prefix.edit");
    expect(described(modify)).not.toContain(NARROW);
  });
});

describe("the « + » of the Topography under 1024 px (task 021-21f)", () => {
  const tree = (url: string) =>
    url.startsWith("/v1/domains?") || url === "/v1/domains"
      ? page([{ id: DOMAIN, slug: "production", links: { self: `/v1/domains/${DOMAIN}` } }])
      : url.startsWith(`/v1/domains/${DOMAIN}/roots`)
        ? page([
            {
              kind: "prefix",
              id: PREFIX,
              value: "192.0.2.0/24",
              nature: "network",
              version: "1",
              href: `/v1/prefixes/${PREFIX}`,
            },
          ])
        : page([]);
  const plus = (root: HTMLElement) =>
    root.querySelector<HTMLButtonElement>(
      '.k-tree-grid__add[aria-label="Ajouter dans 192.0.2.0/24"]',
    );

  it("refused with the width, opens nothing; wide again, it adds", async () => {
    const { root } = await mounted(
      "/topology",
      ["core.prefix.read", "core.prefix.edit", "core.address.edit", "core.range.edit"],
      tree,
    );
    await expect.poll(() => plus(root)).not.toBeNull();
    setFrameWidth("narrow");
    await expect.poll(() => plus(root)?.getAttribute("aria-disabled")).toBe("true");
    expect(described(plus(root)!)).toContain(NARROW);
    plus(root)!.click();
    expect(editRequest().value).toBeNull();

    setFrameWidth("wide");
    await expect.poll(() => plus(root)?.getAttribute("aria-disabled")).not.toBe("true");
    plus(root)!.click();
    await expect
      .poll(() => editRequest().value)
      .toMatchObject({ mode: "create", collection: "addresses", within: "192.0.2.0/24" });
  });

  it("the permission missing is said before the width", async () => {
    const { root } = await mounted("/topology", ["core.prefix.read"], tree);
    await expect.poll(() => plus(root)).not.toBeNull();
    setFrameWidth("narrow");
    await nextTick();
    expect(described(plus(root)!)).toContain("core.address.edit");
    expect(described(plus(root)!)).not.toContain(NARROW);
  });
});

describe("« Acquitter » of an anomaly under 1024 px (task 021-21f)", () => {
  // A layout for the grid, which happy-dom does not compute (as `anomalies.test.ts`).
  beforeEach(() => {
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(600);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement,
    ) {
      const top = this.dataset.index === undefined ? 0 : 40 + Number(this.dataset.index) * 32;
      return DOMRect.fromRect({ x: 0, y: top, width: 800, height: top ? 32 : 600 });
    });
  });
  afterEach(() => vi.restoreAllMocks());

  const anomalies = [
    {
      id: ANOMALY,
      rule: "core.addr.documentation",
      message: "une adresse de documentation",
      object: { id: PREFIX, kind: "prefix", href: `/v1/prefixes/${PREFIX}` },
      opened_at: "2026-01-01T00:00:00Z",
      links: { acknowledgement: `/v1/anomalies/${ANOMALY}/acknowledgement` },
    },
  ];
  const grid = (permissions: string[]) =>
    mounted("/", permissions, () => ({}), AnomalyGrid, { label: "Anomalies", anomalies });

  it("refused with the width; a dialog left open keeps its motive and sends nothing", async () => {
    const { root, writes } = await grid(["core.anomaly.acknowledge"]);
    await expect.poll(() => button(root, "Acquitter")).toBeDefined();
    button(root, "Acquitter")!.click();
    await expect.poll(() => document.querySelector("[role='dialog'] input")).not.toBeNull();
    const dialog = document.querySelector<HTMLElement>("[role='dialog']")!;
    const motive = dialog.querySelector<HTMLInputElement>("input")!;
    typed(motive, "Une plage de test.");
    const confirm = () => button(dialog, "Acquitter")!;

    setFrameWidth("narrow");
    await nextTick();
    expect(described(button(root, "Acquitter")!)).toContain(NARROW);
    expect(described(confirm())).toContain(NARROW);
    confirm().click();
    // Enter in the motive sends the form anyway: the width said in the dialog.
    submit(dialog.querySelector("form")!);
    await nextTick();
    expect(writes()).toEqual([]);
    expect(motive.value).toBe("Une plage de test.");
    const said = dialog.querySelector(".k-inline-alert[role='status']")!;
    expect(said.textContent).toContain(NARROW);
    // Enter again: the alert inserted anew, read again.
    submit(dialog.querySelector("form")!);
    await nextTick();
    const again = dialog.querySelector(".k-inline-alert[role='status']")!;
    expect(again).not.toBe(said);
    expect(again.textContent).toContain(NARROW);

    setFrameWidth("standard");
    await nextTick();
    expect(dialog.querySelector(".k-inline-alert")).toBeNull();
    confirm().click();
    await expect
      .poll(() => writes()[0]?.url)
      .toMatch(new RegExp(`^/v1/anomalies/${ANOMALY}/acknowledgement`));
  });

  it("the permission missing is said before the width", async () => {
    const { root } = await grid([]);
    await expect.poll(() => button(root, "Acquitter")).toBeDefined();
    setFrameWidth("narrow");
    await nextTick();
    expect(described(button(root, "Acquitter")!)).toContain("core.anomaly.acknowledge");
    expect(described(button(root, "Acquitter")!)).not.toContain(NARROW);
  });
});

describe("« Poser le VLAN » of a network under 1024 px (task 021-21f)", () => {
  const network = (url: string) =>
    url.startsWith(`/v1/prefixes/${PREFIX}/properties`)
      ? { properties: {}, masked: [], explanation: {} }
      : url.startsWith("/v1/property-declarations")
        ? page([{ id: "0b5e3c1a-0000-4000-8000-0000000000f1", key: "core.vlan", type: "ref" }])
        : page([]);
  const tab = (permissions: string[]) =>
    mounted("/", permissions, network, NetworkVlanTab, {
      id: PREFIX,
      name: "192.0.2.0/24",
      nature: "network",
    });

  it("refused with the width; Enter in the field says it in the form, nothing sent", async () => {
    const { root, writes } = await tab([...NETWORK_VLAN_EDITS]);
    await expect.poll(() => root.querySelector("form")).not.toBeNull();
    await expect
      .poll(() => button(root, "Poser le VLAN")?.getAttribute("aria-disabled"))
      .toBeNull();
    setFrameWidth("narrow");
    await nextTick();
    const set = button(root, "Poser le VLAN")!;
    expect(set.getAttribute("aria-disabled")).toBe("true");
    expect(described(set)).toContain(NARROW);
    // Refused, it is not the submit button: Enter in the field sends the form, which says why.
    expect(set.type).toBe("button");
    submit(root.querySelector("form")!);
    await nextTick();
    expect(root.querySelector("form .k-inline-alert")?.textContent).toContain(NARROW);
    expect(writes()).toEqual([]);

    setFrameWidth("standard");
    await nextTick();
    expect(root.querySelector("form .k-inline-alert")).toBeNull();
    expect(button(root, "Poser le VLAN")!.type).toBe("submit");
  });

  it("the permission missing is said before the width", async () => {
    const { root } = await tab([]);
    await expect.poll(() => button(root, "Poser le VLAN")).toBeDefined();
    setFrameWidth("narrow");
    await nextTick();
    expect(described(button(root, "Poser le VLAN")!)).toContain(NETWORK_VLAN_EDITS[0]!);
    expect(described(button(root, "Poser le VLAN")!)).not.toContain(NARROW);
    // Enter in the field says it in the form too, the permission first.
    submit(root.querySelector("form")!);
    await nextTick();
    expect(root.querySelector("form .k-inline-alert")?.textContent).toContain(
      NETWORK_VLAN_EDITS[0]!,
    );
  });
});
