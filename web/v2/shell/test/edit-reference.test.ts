// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import EditPanel from "../src/views/edit/EditPanel.vue";
import { closeEdit, editRequest, openEdit, type EditRequest } from "../src/views/edit/panel.ts";

// A reference of the edit panel beyond the first page (task 021-12j, REQ-021-36): the domain,
// the parent, the group or the anchor is a `Combobox` whose suggestions are asked of its
// collection by `q` as the text is typed, never taken from a first page read; a value given
// beforehand is named by its object read; a text typed and nothing taken is refused, nothing
// guessed.

const ALPHA = "0b5e3c1a-0000-4000-8000-0000000000d1";
const ZETA = "0b5e3c1a-0000-4000-8000-0000000000d9";
const C1 = "0b5e3c1a-0000-4000-8000-0000000000c1";

interface Seen {
  method: string;
  url: string;
  body: Record<string, unknown> | null;
  /** The read was cancelled before its answer (an answer `delay`ed). */
  aborted?: boolean;
}
type Answer = { status?: number; body: unknown; delay?: number };

/** The domains of the fake API: a first page of one, `zeta` found by `q` only. */
function domains(call: Seen): Answer | undefined {
  if (call.method !== "GET") return undefined;
  const url = new URL(call.url, "http://kartenn.invalid");
  if (url.pathname === `/v1/domains/${ZETA}`) {
    return { body: { id: ZETA, slug: "zeta", version: "1", href: `/v1/domains/${ZETA}` } };
  }
  if (url.pathname !== "/v1/domains") return undefined;
  const q = url.searchParams.get("q");
  const items =
    q === null
      ? [{ id: ALPHA, slug: "alpha", href: `/v1/domains/${ALPHA}` }]
      : "zeta".startsWith(q)
        ? [{ id: ZETA, slug: "zeta", href: `/v1/domains/${ZETA}` }]
        : [];
  return { body: { items, links: q === null ? { next: "/v1/domains?cursor=2" } : {} } };
}

describe("a reference of the edit panel, beyond the first page", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    closeEdit(false);
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
  });

  async function panel(
    request: EditRequest,
    answer: (call: Seen) => Answer | undefined = domains,
    permissions = ["core.prefix.edit", "core.prefix.approval.self"],
  ) {
    const seen: Seen[] = [];
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const call: Seen = {
        method: init?.method ?? "GET",
        url: String(input),
        body: init?.body ? JSON.parse(String(init.body)) : null,
      };
      seen.push(call);
      const out = call.url.startsWith("/v1/labels")
        ? { body: { lang: "fr", default_lang: "fr", revision: "1", spaces: {} } }
        : (answer(call) ?? { status: 404, body: { type: "about:blank", status: 404 } });
      if ("delay" in out && out.delay) {
        await new Promise((done) => setTimeout(done, out.delay));
        call.aborted = init?.signal?.aborted === true;
        if (call.aborted) throw new DOMException("cancelled", "AbortError");
      }
      const status = out.status ?? 200;
      return new Response(JSON.stringify(out.body), {
        status,
        headers: {
          "Content-Type": status >= 400 ? "application/problem+json" : "application/json",
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
        const shown = editRequest();
        return () =>
          shown.value && shown.value.mode !== "close"
            ? h(EditPanel, { request: shown.value, key: shown.value.serial })
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
    openEdit(request);
    await expect.poll(() => root.querySelector("form")).not.toBeNull();
    return { root, seen };
  }
  const field = (root: HTMLElement, label: string) => {
    const id = [...root.querySelectorAll("label")].find((l) =>
      l.textContent?.trim().startsWith(label),
    )!.htmlFor;
    return document.getElementById(id) as HTMLInputElement;
  };
  const type = async (input: HTMLInputElement, text: string) => {
    input.value = text;
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await nextTick();
  };
  const options = () => [...document.querySelectorAll<HTMLElement>("[role='option']")];
  const ctrlEnter = (root: HTMLElement) =>
    root
      .querySelector("form")!
      .dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, bubbles: true }));
  /** « Appliquer » chosen in the menu of the split button. */
  async function chooseApply(root: HTMLElement): Promise<void> {
    const chevron = root.querySelector<HTMLButtonElement>(
      ".k-split-button [aria-haspopup='menu']",
    )!;
    chevron.focus();
    chevron.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown", bubbles: true }));
    await expect.poll(() => document.querySelectorAll("[role='menuitem']").length).toBe(2);
    document.querySelectorAll<HTMLElement>("[role='menuitem']")[1]!.click();
    await expect
      .poll(() => root.querySelector(".k-split-button__main")!.textContent)
      .toMatch(/Appliquer/);
  }

  it("suggests the domains by `q` of their collection, one beyond the first page taken", async () => {
    const { root, seen } = await panel({ mode: "create", collection: "prefixes" }, (call) =>
      call.method === "POST" && call.url === "/v1/changesets?approval=self"
        ? { status: 201, body: { changeset: { id: C1 }, warnings: [], cascaded: [] } }
        : domains(call),
    );
    const domain = field(root, "Domaine");
    expect(domain.getAttribute("role")).toBe("combobox");
    await type(domain, "ze");
    // Asked of the collection by its text, not filtered from the first page read.
    await expect.poll(() => options().map((o) => o.textContent?.trim())).toEqual(["zeta"]);
    const asked = seen.filter((s) => s.url.startsWith("/v1/domains?"));
    expect(
      asked.map((s) => new URL(s.url, "http://kartenn.invalid").searchParams.get("q")),
    ).toEqual(["ze"]);
    options()[0]!.click();
    await nextTick();
    expect(domain.value).toBe("zeta");
    await type(field(root, "Valeur"), "198.51.100.0/24");
    await chooseApply(root);
    ctrlEnter(root);
    await expect
      .poll(() => seen.find((s) => s.method === "POST")?.body?.operations)
      .toMatchObject([{ op: "create_prefix", domain: { id: ZETA }, prefix: "198.51.100.0/24" }]);
  });

  it("names a domain given beforehand by its object read, even beyond the first page", async () => {
    const { root, seen } = await panel({
      mode: "create",
      collection: "prefixes",
      preset: { domain: ZETA },
    });
    await expect.poll(() => field(root, "Domaine").value).toBe("zeta");
    expect(seen.some((s) => s.url === `/v1/domains/${ZETA}`)).toBe(true);
    // Nothing listed until the field asks: no first page read to choose from.
    expect(seen.some((s) => s.url.startsWith("/v1/domains?"))).toBe(false);
  });

  it("refuses a text typed and no suggestion taken: nothing guessed, nothing sent", async () => {
    const { root, seen } = await panel({ mode: "create", collection: "prefixes" });
    const domain = field(root, "Domaine");
    await type(domain, "zeta");
    await expect.poll(() => options().length).toBe(1);
    await type(field(root, "Valeur"), "198.51.100.0/24");
    ctrlEnter(root);
    await expect
      .poll(() => document.getElementById(domain.getAttribute("aria-describedby")!.split(" ")[0]!))
      .not.toBeNull();
    await expect.poll(() => domain.getAttribute("aria-invalid")).toBe("true");
    const described = domain
      .getAttribute("aria-describedby")!
      .split(" ")
      .map((id) => document.getElementById(id)?.textContent)
      .join(" ");
    expect(described).toContain("choisissez un objet parmi les suggestions");
    expect(seen.some((s) => s.method !== "GET")).toBe(false);
  });

  it("offers « Aucun » to an optional parent, and the parents found by `q`", async () => {
    const E9 = "0b5e3c1a-0000-4000-8000-0000000000e9";
    const { root, seen } = await panel(
      { mode: "create", collection: "entities" },
      (call) => {
        const url = new URL(call.url, "http://kartenn.invalid");
        if (url.pathname !== "/v1/entities") return undefined;
        return {
          body: {
            items: url.searchParams.get("q")
              ? [{ id: E9, slug: "atelier-nord", href: `/v1/entities/${E9}` }]
              : [],
            links: {},
          },
        };
      },
      ["core.entity.edit"],
    );
    const parent = field(root, "Parent");
    await type(parent, "a");
    await expect
      .poll(() => options().map((o) => o.textContent?.trim()))
      .toEqual(["Aucun", "atelier-nord"]);
    expect(seen.some((s) => s.url.startsWith("/v1/entities?") && s.url.includes("q=a"))).toBe(true);
  });

  it("reads the suggestions by a key of their own: one page, the ancestors' read never cancelled", async () => {
    const E1 = "0b5e3c1a-0000-4000-8000-0000000000e1";
    const E2 = "0b5e3c1a-0000-4000-8000-0000000000e2";
    const entity = (id: string, slug: string) => ({
      id,
      slug,
      parent: null,
      version: "1",
      href: `/v1/entities/${id}`,
    });
    const { root, seen } = await panel(
      { mode: "move", collection: "entities", id: E1 },
      (call) => {
        const url = new URL(call.url, "http://kartenn.invalid");
        if (url.pathname === `/v1/entities/${E1}`) return { body: entity(E1, "essai-a") };
        if (url.pathname !== "/v1/entities") return undefined;
        // The suggestions: a page of their own, at once.
        if (url.searchParams.has("page_size")) {
          return { body: { items: [entity(E2, "essai-b")], links: {} } };
        }
        // The ancestors of the move: three pages, slow.
        const page = Number(url.searchParams.get("cursor") ?? "1");
        return {
          delay: 80,
          body: {
            items: [entity(`${E2.slice(0, -2)}f${page}`, `noeud-${page}`)],
            links: page < 3 ? { next: `/v1/entities?cursor=${page + 1}` } : {},
          },
        };
      },
      ["core.entity.edit"],
    );
    const parent = field(root, "Parent");
    // The field emptied while the ancestors are read: the first page of suggestions asked,
    // then replaced by a newer text, which cancels it.
    await type(parent, "");
    await new Promise((done) => setTimeout(done, 200));
    await type(parent, "e");
    await expect.poll(() => options().map((o) => o.textContent?.trim())).toEqual(["essai-b"]);
    const nodes = () =>
      seen.filter((s) => s.url.startsWith("/v1/entities") && !s.url.includes("page_size"));
    await expect.poll(() => nodes().filter((s) => s.aborted === false).length).toBe(3);
    expect(nodes().some((s) => s.aborted)).toBe(false);
    const suggested = seen.filter((s) => s.url.includes("page_size"));
    expect(suggested.length).toBeGreaterThan(0);
    // One page each: never a cursor followed for a suggestion.
    expect(suggested.every((s) => !s.url.includes("cursor"))).toBe(true);
  });
});
