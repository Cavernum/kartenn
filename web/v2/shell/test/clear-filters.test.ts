// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";

// « Retirer les filtres », the way out of an empty list (task 021-11h): the filters taken
// away, the focus on « Ajouter un filtre » through the method `FilterPicker` exposes, no
// exception. Its root is renderless (the popover's), its `$el` a text node: a lookup of the
// bar from it threw in ListView and ChangesetsView.

let app: App | undefined;
const errors: unknown[] = [];
const onError = (error: unknown) => errors.push(error);
afterEach(() => {
  app?.unmount();
  app = undefined;
  queryClient.clear();
  resetContext();
  setFetch(null);
  document.body.innerHTML = "";
  process.off("unhandledRejection", onError);
  errors.length = 0;
});

/** The shell mounted at `at`, every list of the API empty. */
async function mounted(at: string, permissions: string[]) {
  setFetch((async (input: RequestInfo | URL) => {
    const body = String(input).startsWith("/v1/labels")
      ? { lang: "fr", default_lang: "fr", revision: "1", spaces: {} }
      : { counts: [], items: [], links: {}, sections: [] };
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
  app = createApp({ render: () => h(RouterView) });
  app.config.errorHandler = onError;
  app.use(createShellI18n("fr")).use(router);
  app.mount(root);
  await router.isReady();
  return { root, router };
}

const button = (root: HTMLElement, name: string) =>
  [...root.querySelectorAll<HTMLButtonElement>("button")].find(
    (b) => b.textContent?.trim() === name,
  );

describe("« Retirer les filtres » of an empty list (task 021-11h)", () => {
  it.each([
    ["ListView", "/addresses?usage=host", ["core.address.read"]],
    ["ChangesetsView", "/changesets?state=pending", ["core.changeset.read"]],
    ["AuditView", "/admin/audit?object=0190a000-0000-7000-8000-000000000001", ["core.audit.read"]],
  ])("%s: the filters taken away, the focus on « Ajouter un filtre »", async (_, at, rights) => {
    process.on("unhandledRejection", onError);
    const { root, router } = await mounted(at, rights);
    await expect.poll(() => button(root, "Retirer les filtres")).toBeDefined();
    button(root, "Retirer les filtres")!.click();
    await expect.poll(() => Object.keys(router.currentRoute.value.query)).toEqual([]);
    await nextTick();
    await expect
      .poll(() => document.activeElement?.classList.contains("k-filter-picker__add"))
      .toBe(true);
    // Its words, then the key of the view that opens it (`/`).
    expect(document.activeElement?.textContent?.trim()).toMatch(/^Ajouter un filtre/);
    // A rejection of the focus step would come after it: one more turn of the loop.
    await new Promise((done) => setTimeout(done, 0));
    expect(errors).toEqual([]);
  });
});
