// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import ExplanationBody from "../src/views/record/ExplanationBody.vue";
import { provenanceOf } from "../src/views/record/provenance.ts";

// The provenance in the explanation of a value (task 021-13d, REQ-003-14; ADR 0026,
// amendment 14): the three forms the server stores, `input`, `source:<id>@<version>` and
// `module:<module>#<motif>`; a value a module derived is said so, not « set directly ».

describe("the provenance of an assignment, read", () => {
  it("typed: input", () => {
    expect(provenanceOf("input")).toEqual({ kind: "input" });
  });

  it("the default of a source: its form kept, the source and its revision read", () => {
    expect(provenanceOf("source:s1@3")).toEqual({
      kind: "source",
      code: "source:s1@3",
      source: "s1",
      revision: "3",
    });
    // The detail after `#` is the source's own (REQ-016-5): not part of the revision.
    expect(provenanceOf("source:cmdb@r12#row:7@b")).toEqual({
      kind: "source",
      code: "source:cmdb@r12#row:7@b",
      source: "cmdb",
      revision: "r12",
    });
    expect(provenanceOf("source:cmdb")).toEqual({
      kind: "source",
      code: "source:cmdb",
      source: "cmdb",
      revision: null,
    });
    expect(provenanceOf("source:@3")).toEqual({ kind: "other", code: "source:@3" });
  });

  it("derived by kube from a label: the label's key", () => {
    expect(provenanceOf("module:kube#label:app.example/tier")).toEqual({
      kind: "label",
      module: "kube",
      key: "app.example/tier",
    });
  });

  it("a motif unknown, of kube or another module: raw, named by its module", () => {
    expect(provenanceOf("module:kube#annotation:x")).toEqual({
      kind: "module",
      module: "kube",
      motif: "annotation:x",
    });
    expect(provenanceOf("module:dhcp#label:x")).toEqual({
      kind: "module",
      module: "dhcp",
      motif: "label:x",
    });
  });

  it("none or malformed: nothing, or the stored form as is", () => {
    expect(provenanceOf(null)).toBeNull();
    expect(provenanceOf(undefined)).toBeNull();
    expect(provenanceOf("module:kube")).toEqual({ kind: "other", code: "module:kube" });
    expect(provenanceOf("module:#x")).toEqual({ kind: "other", code: "module:#x" });
    expect(provenanceOf("module:kube#label:")).toEqual({
      kind: "module",
      module: "kube",
      motif: "label:",
    });
  });
});

describe("the explanation of a value", () => {
  const OBJECT = "0b5e3c1a-0000-4000-8000-0000000000a1";
  const PARENT = "0b5e3c1a-0000-4000-8000-0000000000a2";
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    document.body.replaceChildren();
  });

  /** The explanation of `custom.tier` on the object, as the server gives `facts`. */
  async function mounted(facts: object) {
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      const body = url.includes("/explanation")
        ? { property: "custom.tier", value: "web", masked: false, chain: [], links: {}, ...facts }
        : url.startsWith("/v1/labels")
          ? { lang: "fr", default_lang: "fr", revision: "1", spaces: {} }
          : { id: PARENT, name: "parent", links: {} };
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
    app = createApp({
      render: () => h(ExplanationBody, { kind: "prefix", id: OBJECT, property: "custom.tier" }),
    });
    app.use(createShellI18n("fr")).use(router);
    document.body.append(root);
    app.mount(root);
    await router.isReady();
    await expect
      .poll(() => root.querySelector(".k-explanation p")?.textContent ?? "", { timeout: 8_000 })
      .not.toBe("");
    return root;
  }
  const text = (root: HTMLElement) => root.textContent?.replace(/\s+/gu, " ") ?? "";
  const own = { origin: { id: OBJECT, kind: "prefix" } };

  it("typed on the object: set directly", async () => {
    const root = await mounted({ ...own, provenance: "input" });
    expect(text(root)).toContain("Posée directement sur cet objet.");
  });

  it("the default of a source on the object: set directly, as before", async () => {
    const root = await mounted({ ...own, provenance: "source:s1@3" });
    expect(text(root)).toContain("Posée directement sur cet objet.");
    expect(text(root)).not.toContain("module");
  });

  it("derived by kube from a label: the label named, never « set directly »", async () => {
    const root = await mounted({ ...own, provenance: "module:kube#label:app.example/tier" });
    expect(text(root)).toContain("Tirée par le module kube du label");
    expect(root.querySelector("code")?.textContent).toBe("app.example/tier");
    expect(text(root)).not.toContain("Posée directement");
  });

  it("a motif of another module: raw, named by its module", async () => {
    const root = await mounted({ ...own, provenance: "module:dhcp#lease:option-12" });
    expect(text(root)).toContain("Dérivée par le module dhcp :");
    expect(root.querySelector("code")?.textContent).toBe("lease:option-12");
    expect(text(root)).not.toContain("Posée directement");
  });

  it("inherited from a value a module derived: both said", async () => {
    const root = await mounted({
      origin: { id: PARENT, kind: "prefix" },
      provenance: "module:kube#label:tier",
    });
    expect(text(root)).toContain("Héritée de");
    expect(text(root)).toContain("Tirée par le module kube du label");
    expect(root.querySelector("code")?.textContent).toBe("tier");
  });

  it("the origin withheld, a module's provenance: said in words, not as a rule", async () => {
    const root = await mounted({ origin: null, provenance: "module:kube#label:tier" });
    expect(text(root)).toContain("Tirée par le module kube du label");
    expect(text(root)).not.toContain("Inférée par la règle");
  });
});
