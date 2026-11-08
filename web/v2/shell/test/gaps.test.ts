// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import GapList from "../src/views/anomalies/GapList.vue";

// A gap composed by the screen (task 021-11m, 021/A36): its values by their labels, its
// source and revision, its rule by its code, never the server's sentence; a resolved
// `null` is « no value » only beside an expected value — masked, or stored before its
// values were (0075), the gap says so instead.

const KEY = "custom.environment";
const RULE = "prop.resolution.expected_mismatch";
const PREFIX = "0b5e3c1a-0000-4000-8000-0000000000a1";
const NO_VALUE = "non renseigné";

const gap = (id: string, members: Record<string, unknown>) => ({
  id: `0b5e3c1a-0000-4000-8000-00000000${id}`,
  property: KEY,
  object: { id: PREFIX, kind: "prefix", href: `/v1/prefixes/${PREFIX}` },
  rule: RULE,
  resolved: null,
  expected: null,
  source: null,
  revision: null,
  message: `${KEY}: server sentence ${id}`,
  opened_at: "2026-01-01T00:00:00Z",
  ...members,
});

const labels = {
  lang: "fr",
  default_lang: "fr",
  revision: "1",
  spaces: {
    property: { [KEY]: "Environnement" },
    value: { [KEY]: { prod: "Production", dev: "Développement" } },
    rule: { [RULE]: "Écart à la source" },
  },
};
const declaration = {
  key: KEY,
  type: "enum",
  cardinality: "single",
  carriers: ["prefix"],
  labels: { fr: "Environnement" },
};
const page = (items: object[]) => ({ items, links: {} });

describe("the gaps, each composed by the screen", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    document.body.replaceChildren();
  });

  /** The list of the gaps, served `gaps`; the text of each row by the id of its gap. */
  async function mounted(gaps: object[]) {
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      const body = url.startsWith("/v1/labels")
        ? labels
        : url.startsWith("/v1/property-gaps")
          ? page(gaps)
          : url.startsWith("/v1/property-declarations")
            ? page([declaration])
            : page([]);
      return new Response(JSON.stringify(body), {
        headers: { "Content-Type": "application/json", ETag: '"v1"' },
      });
    }) as typeof fetch);
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: RouterView, children: frameRoutes }],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({ render: () => h(GapList) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    const row = (text: string) =>
      [...root.querySelectorAll("[role='row']")].find((r) => r.textContent?.includes(text));
    return { root, row };
  }

  it("evaluated: the values by their labels, the source and its revision, the rule by its code", async () => {
    const { root, row } = await mounted([
      gap("0001", { expected: "prod", resolved: "dev", source: "policy.inference", revision: "7" }),
    ]);
    await expect.poll(() => row("Production")?.textContent, { timeout: 8_000 }).toBeDefined();
    const text = row("Production")!.textContent!;
    expect(text).toContain("Développement");
    expect(text).toContain("Environnement");
    expect(text).toContain(KEY);
    expect(text).toContain("policy.inference");
    expect(text).toContain("révision 7");
    expect(text).toContain(RULE);
    expect(text).not.toContain(NO_VALUE);
    expect(root.textContent).not.toContain("server sentence");
  }, 10_000);

  it("evaluated without a resolved value: « no value » beside the expected one", async () => {
    const { row } = await mounted([
      gap("0002", { expected: "prod", source: "policy.inference", revision: "7" }),
    ]);
    await expect.poll(() => row("Production")?.textContent, { timeout: 8_000 }).toBeDefined();
    expect(row("Production")!.textContent).toContain(NO_VALUE);
  }, 10_000);

  it("masked: both values said masked, never « no value »", async () => {
    const { root, row } = await mounted([
      gap("0003", { source: "policy.inference", revision: "7" }),
    ]);
    await expect.poll(() => row("Masquée")?.textContent, { timeout: 8_000 }).toBeDefined();
    const text = row("Masquée")!.textContent!;
    expect(text.split("Masquée")).toHaveLength(3);
    expect(text).toContain("policy.inference");
    expect(text).not.toContain(NO_VALUE);
    expect(root.textContent).not.toContain("server sentence");
  }, 10_000);

  it("stored before its values: said not detailed, never « no value »", async () => {
    const { root, row } = await mounted([gap("0004", {})]);
    await expect.poll(() => row("Non détaillée")?.textContent, { timeout: 8_000 }).toBeDefined();
    const text = row("Non détaillée")!.textContent!;
    expect(text.split("Non détaillée")).toHaveLength(3);
    expect(text).not.toContain("Masquée");
    expect(root.textContent).not.toContain("server sentence");
  }, 10_000);
});
