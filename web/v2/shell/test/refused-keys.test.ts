// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView, type Router } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { play, runnable } from "../src/shortcuts/view.ts";
import { timeOf, writesOf } from "../src/time/index.ts";
import { closeEdit, editRequest } from "../src/views/edit/panel.ts";

// `c` and `e` refused say why by the polite status of the view (task 021-12k, REQ-021-37;
// relevés des 12c et 12d): under `as_of`, without the right, without a domain — the list
// (which has no « Créer » to carry the reason), the Topography, the record. And the cell of
// a content grid names what « Modifier » changes under a validity date: the record or the
// node chosen, not its row.

const AS_OF = "2026-06-30T12:00:00Z";
const VALID_AT = "2026-01-01T00:00:00Z";
const DOMAIN = "0b5e3c1a-0000-4000-8000-0000000000d1";
const PREFIX = "0b5e3c1a-0000-4000-8000-000000000101";
const CHILD = "0b5e3c1a-0000-4000-8000-000000000102";
const page = (items: object[]) => ({ items, links: {} });
const NODE = {
  kind: "prefix",
  id: PREFIX,
  value: "192.0.2.0/24",
  nature: "network",
  version: "1",
  href: `/v1/prefixes/${PREFIX}`,
};

let app: App | undefined;
afterEach(() => {
  closeEdit(false);
  app?.unmount();
  app = undefined;
  document.body.replaceChildren();
  queryClient.clear();
  resetContext();
  setFetch(null);
});

const t = (key: string) => (key === "edit.why.past" ? "<past>" : key);

/** The shell's views mounted at `at` for an account with `permissions`, the API answered by `answer`. */
async function mounted(
  at: string,
  permissions: string[],
  answer: (url: string) => object,
  host?: ReturnType<typeof defineComponent>,
): Promise<{ root: HTMLElement; router: Router }> {
  setFetch((async (input: RequestInfo | URL) => {
    const url = String(input);
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
    routes: host
      ? [{ path: "/", component: host }]
      : [{ path: "/", component: RouterView, children: frameRoutes }],
  });
  await router.push(at);
  const root = document.body.appendChild(document.createElement("div"));
  app = createApp({ render: () => h(RouterView) });
  app.use(createShellI18n("fr")).use(router);
  app.mount(root);
  await router.isReady();
  return { root, router };
}

/** The period of the route now. */
const timeIn = (router: Router) => writesOf(timeOf(router.currentRoute.value.query), "wide");
/** What the polite statuses of the view say now. */
const statuses = (root: HTMLElement) =>
  [...root.querySelectorAll('[role="status"]')].map((s) => s.textContent?.trim()).join(" | ");

describe("`c` and `e` refused say why (task 021-12k)", () => {
  it("a list: under `as_of` `c` says the present; for a reader, the permission", async () => {
    const author = ["core.prefix.read", "core.prefix.edit"];
    const { root, router } = await mounted(`/prefixes?as_of=${AS_OF}`, author, () => page([]));
    await expect.poll(() => root.querySelector('[role="status"]')).not.toBeNull();
    expect(runnable("create", timeIn(router))).toBeNull();
    expect(play("create", timeIn(router), t)).toBe(true);
    await expect.poll(() => statuses(root)).toContain("<past>");
    expect(editRequest().value).toBeNull();
    // `e` with no row of the focus: nothing to modify, nothing said, the key let go.
    expect(play("edit", timeIn(router), t)).toBe(false);
  });

  it("a list for a reader: `c` says the permission it lacks, the palette has no « Créer »", async () => {
    const { root, router } = await mounted("/prefixes", ["core.prefix.read"], () => page([]));
    await expect.poll(() => root.querySelector('[role="status"]')).not.toBeNull();
    expect(runnable("create", timeIn(router))).toBeNull();
    expect(play("create", timeIn(router), t)).toBe(true);
    await expect.poll(() => statuses(root)).toContain("Lecture seule : il faut core.prefix.edit");
    expect(editRequest().value).toBeNull();
  });

  it("the Topography: no domain, then a reader's node: `c` and `e` say why", async () => {
    const empty = await mounted("/topology", ["core.prefix.read"], () => page([]));
    await expect.poll(() => empty.root.textContent).toContain("Aucun domaine");
    expect(play("create", timeIn(empty.router), t)).toBe(true);
    await expect
      .poll(() => statuses(empty.root))
      .toContain("Ajout impossible : aucun domaine que vous puissiez lire.");
    // Off the present too, no domain before the past: the lasting reason first (task 021-21h).
    await empty.router.push(`/topology?as_of=${AS_OF}`);
    expect(play("create", timeIn(empty.router), t)).toBe(true);
    await expect
      .poll(() => statuses(empty.root))
      .toContain("Ajout impossible : aucun domaine que vous puissiez lire.");
    expect(statuses(empty.root)).not.toContain("<past>");
    app?.unmount();
    app = undefined;
    queryClient.clear();

    const { root, router } = await mounted(
      `/topology?node=${PREFIX}`,
      ["core.prefix.read"],
      (url) =>
        url.startsWith("/v1/domains?") || url === "/v1/domains"
          ? page([{ id: DOMAIN, slug: "production", links: { self: `/v1/domains/${DOMAIN}` } }])
          : url.startsWith(`/v1/domains/${DOMAIN}/roots`)
            ? page([NODE])
            : url.startsWith(`/v1/prefixes/${PREFIX}`) && !url.includes("/children")
              ? { ...NODE, domain: DOMAIN, links: {} }
              : page([]),
    );
    await expect
      .poll(() => root.querySelector("[data-k-tree]")?.textContent)
      .toContain("192.0.2.0/24");
    await expect
      .poll(() => (play("edit", timeIn(router), t), statuses(root)))
      .toContain("Lecture seule : il faut core.prefix.edit");
    expect(editRequest().value).toBeNull();
    // `c` adds in the network chosen: an address first, as its « + » says.
    expect(play("create", timeIn(router), t)).toBe(true);
    await expect.poll(() => statuses(root)).toContain("Lecture seule : il faut core.address.edit");
    expect(editRequest().value).toBeNull();
    // Under `as_of`, a domain read: `c` and `e` say the permission, before the present
    // (task 021-21h: the lasting reason first).
    await router.push(`/topology?node=${PREFIX}&as_of=${AS_OF}`);
    await expect
      .poll(() => (play("create", timeIn(router), t), statuses(root)))
      .toContain("Lecture seule : il faut core.address.edit");
    await expect
      .poll(() => (play("edit", timeIn(router), t), statuses(root)))
      .toContain("Lecture seule : il faut core.prefix.edit");
    // Each key taken, as it was under the past.
    expect(play("create", timeIn(router), t)).toBe(true);
    expect(play("edit", timeIn(router), t)).toBe(true);
    expect(statuses(root)).not.toContain("<past>");
    expect(editRequest().value).toBeNull();
  });

  it("the record: under `as_of`, `e` says the present by its status", async () => {
    const { root, router } = await mounted(
      `/prefixes/${PREFIX}?as_of=${AS_OF}`,
      ["core.prefix.read", "core.prefix.edit"],
      (url) =>
        url.startsWith(`/v1/prefixes/${PREFIX}/properties`)
          ? { properties: {}, masked: [], explanation: {} }
          : url.startsWith(`/v1/prefixes/${PREFIX}`) && !url.includes(`${PREFIX}/`)
            ? { ...NODE, domain: DOMAIN, properties: {}, links: {} }
            : page([]),
    );
    await expect.poll(() => root.querySelector("h1")?.textContent).toContain("192.0.2.0/24");
    await expect.poll(() => (play("edit", timeIn(router), t), statuses(root))).toContain("<past>");
    expect(editRequest().value).toBeNull();
  });
});

describe("the cell of a content grid under a validity date (task 021-12k)", () => {
  it("names what « Modifier » changes: the prefix of the content, not its row", async () => {
    const { default: ContentTab } = await import("../src/views/record/ContentTab.vue");
    const Host = defineComponent({
      setup: () => () => h(ContentTab, { kind: "prefix", id: PREFIX, name: "192.0.2.0/24" }),
    });
    const { root } = await mounted(
      `/?valid_at=${VALID_AT}`,
      ["core.prefix.read", "core.prefix.edit"],
      (url) =>
        url.startsWith(`/v1/prefixes/${PREFIX}/children`)
          ? page([
              {
                kind: "prefix",
                id: CHILD,
                value: "192.0.2.0/25",
                nature: "network",
                version: "1",
                href: `/v1/prefixes/${CHILD}`,
              },
            ])
          : page([]),
      Host,
    );
    const cell = () =>
      root.querySelector<HTMLElement>(`[data-row="${CHILD}"][data-column="detail"]`);
    await expect.poll(() => cell()?.getAttribute("aria-describedby")).toBeTruthy();
    const described = cell()!.getAttribute("aria-describedby")!.split(" ");
    const words = described.map((id) => document.getElementById(id)?.textContent).join(" ");
    expect(words).toContain(
      "« Modifier » ouvre le brouillon daté de 192.0.2.0/24, non de cette ligne.",
    );
    // At the foot of the grid, the same words once the cell has the focus.
    cell()!.click();
    cell()!.focus();
    await expect
      .poll(() => root.textContent)
      .toContain("« Modifier » ouvre le brouillon daté de 192.0.2.0/24, non de cette ligne.");
  });

  it("beside the tree of the Topography: the key `e` and « Ouvrir la fiche », no « Modifier » there", async () => {
    const { root } = await mounted(
      `/topology?node=${PREFIX}&valid_at=${VALID_AT}`,
      ["core.prefix.read", "core.prefix.edit"],
      (url) =>
        url.startsWith("/v1/domains?") || url === "/v1/domains"
          ? page([{ id: DOMAIN, slug: "production", links: { self: `/v1/domains/${DOMAIN}` } }])
          : url.startsWith(`/v1/domains/${DOMAIN}/roots`)
            ? page([NODE])
            : url.startsWith(`/v1/prefixes/${PREFIX}/children`)
              ? page([
                  {
                    kind: "prefix",
                    id: CHILD,
                    value: "192.0.2.0/25",
                    nature: "network",
                    version: "1",
                    href: `/v1/prefixes/${CHILD}`,
                  },
                ])
              : url.startsWith(`/v1/prefixes/${PREFIX}`)
                ? { ...NODE, domain: DOMAIN, links: {} }
                : page([]),
    );
    const cell = () =>
      root.querySelector<HTMLElement>(
        `[data-k-content] [data-row="${CHILD}"][data-column="detail"]`,
      );
    await expect.poll(() => cell()?.getAttribute("aria-describedby")).toBeTruthy();
    const described = cell()!.getAttribute("aria-describedby")!.split(" ");
    const words = described.map((id) => document.getElementById(id)?.textContent).join(" ");
    expect(words).toContain(
      "la touche e, ou « Ouvrir la fiche » puis « Modifier », ouvre le brouillon daté de 192.0.2.0/24, non de cette ligne.",
    );
  });
});

describe("a status sentence that no longer holds (task 021-12k)", () => {
  it("a refusal under `as_of`, emptied once back at the present", async () => {
    const author = ["core.prefix.read", "core.prefix.edit"];
    const { root, router } = await mounted(`/prefixes?as_of=${AS_OF}`, author, () => page([]));
    await expect.poll(() => root.querySelector('[role="status"]')).not.toBeNull();
    expect(play("create", timeIn(router), t)).toBe(true);
    await expect.poll(() => statuses(root)).toContain("<past>");
    await router.push("/prefixes");
    await expect.poll(() => statuses(root)).not.toContain("<past>");
  });

  it("the record: the refusal of `e` emptied at the present, and for another object", async () => {
    const { root, router } = await mounted(
      `/prefixes/${PREFIX}?as_of=${AS_OF}`,
      ["core.prefix.read", "core.prefix.edit"],
      (url) =>
        url.includes("/properties")
          ? { properties: {}, masked: [], explanation: {} }
          : url.startsWith("/v1/prefixes/") && url.split("?")[0]!.split("/").length === 4
            ? { ...NODE, domain: DOMAIN, links: {} }
            : page([]),
    );
    await expect.poll(() => (play("edit", timeIn(router), t), statuses(root))).toContain("<past>");
    await router.push(`/prefixes/${PREFIX}`);
    await expect.poll(() => statuses(root)).not.toContain("<past>");
  });
});
