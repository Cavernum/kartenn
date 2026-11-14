// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView, type Router } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { runnable } from "../src/shortcuts/view.ts";
import { timeOf, writesOf } from "../src/time/index.ts";
import { closeEdit, editRequest } from "../src/views/edit/panel.ts";

// Creating under a validity date alone (task 021-12d; REQ-021-5, X5): « Créer » (`c`) of a
// list, `e` of its row, and the « + » of the Topography (with `c` and `e` of the tree) open
// the edit panel — to the dated draft, as « Modifier » of the record —; nothing under
// `as_of`, where the « + » stays visible and says why.

const VALID_AT = "2026-01-01T00:00:00Z";
const AS_OF = "2026-06-30T12:00:00Z";
const DOMAIN = "0b5e3c1a-0000-4000-8000-0000000000d1";
const PREFIX = "0b5e3c1a-0000-4000-8000-000000000101";
const page = (items: object[]) => ({ items, links: {} });

let app: App | undefined;
afterEach(() => {
  closeEdit(false);
  app?.unmount();
  app = undefined;
  queryClient.clear();
  resetContext();
  setFetch(null);
});

/** The shell mounted at `at` for an author of the prefixes, the API answered by `answer`. */
async function mounted(
  at: string,
  answer: (url: string) => object,
): Promise<{
  root: HTMLElement;
  router: Router;
}> {
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
          global_permissions: [
            "core.prefix.read",
            "core.prefix.edit",
            "core.address.edit",
            "core.range.edit",
          ],
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
  app = createApp({ render: () => h(RouterView) });
  app.use(createShellI18n("fr")).use(router);
  app.mount(root);
  await router.isReady();
  return { root, router };
}

/** The period of the route now. */
const timeIn = (router: Router) => writesOf(timeOf(router.currentRoute.value.query), "wide");

describe("creating under a validity date alone (task 021-12d)", () => {
  it("a list: `c` and `e` open the edit panel under `valid_at` alone, never under `as_of`", async () => {
    const { router } = await mounted(`/prefixes?valid_at=${VALID_AT}`, () => page([]));
    await expect.poll(() => runnable("create", timeIn(router))?.label()).toBe("Créer : Préfixe");
    runnable("create", timeIn(router))!.run();
    expect(editRequest().value).toMatchObject({ mode: "create", collection: "prefixes" });
    closeEdit(false);
    await router.push(`/prefixes?as_of=${AS_OF}`);
    await nextTick();
    expect(runnable("create", timeIn(router))).toBeNull();
    await router.push(`/prefixes?valid_at=${VALID_AT}&as_of=${AS_OF}`);
    await nextTick();
    expect(runnable("create", timeIn(router))).toBeNull();
    await router.push("/prefixes");
    await nextTick();
    expect(runnable("create", timeIn(router))).not.toBeNull();
  });

  it("the Topography: the « + » and `c` add under `valid_at` alone; under `as_of` the « + » says why", async () => {
    const { root, router } = await mounted(`/topology?valid_at=${VALID_AT}`, (url) =>
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
          : page([]),
    );
    const plus = () =>
      root.querySelector<HTMLButtonElement>(
        '.k-tree-grid__add[aria-label="Ajouter dans 192.0.2.0/24"]',
      );
    await expect.poll(() => plus()).not.toBeNull();
    expect(plus()!.getAttribute("aria-disabled")).not.toBe("true");
    plus()!.click();
    await expect
      .poll(() => editRequest().value)
      .toMatchObject({ mode: "create", collection: "addresses", within: "192.0.2.0/24" });
    closeEdit(false);
    expect(runnable("create", timeIn(router))).not.toBeNull();
    // The node chosen: `e` modifies it, the « + » of its content adds in it.
    await router.push(`/topology?valid_at=${VALID_AT}&node=${PREFIX}`);
    await expect.poll(() => runnable("edit", timeIn(router))).not.toBeNull();
    const toolbarAdd = () =>
      [...root.querySelectorAll<HTMLButtonElement>(".k-toolbar button")].find((b) =>
        b.textContent?.trim().startsWith("Ajouter dans 192.0.2.0/24"),
      );
    await expect.poll(() => toolbarAdd()).toBeDefined();
    expect(toolbarAdd()!.getAttribute("aria-disabled")).not.toBe("true");

    await router.push(`/topology?as_of=${AS_OF}&node=${PREFIX}`);
    await expect.poll(() => plus()?.getAttribute("aria-disabled")).toBe("true");
    expect(runnable("create", timeIn(router))).toBeNull();
    expect(runnable("edit", timeIn(router))).toBeNull();
    await expect.poll(() => toolbarAdd()?.getAttribute("aria-disabled")).toBe("true");
    const described = plus()!.getAttribute("aria-describedby")?.split(" ") ?? [];
    const description = described.map((id) => document.getElementById(id)?.textContent).join(" ");
    expect(description).toContain("revenez au présent");
  });
});
