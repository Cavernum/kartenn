// @vitest-environment happy-dom
import { drafts, queryClient, type Simulated } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import ClosePanel from "../src/views/edit/ClosePanel.vue";
import { closeOperation, closeVerdict } from "../src/views/edit/close.ts";
import { editRequest, openEdit } from "../src/views/edit/panel.ts";

// « Clore… » of a record (task 021-11g, A31; REQ-004-25, 26; REQ-021-37): the operation at
// the version served, its simulation alone said before it is sent — the rules that refuse
// it with their codes, the dependents a cascade closes counted by kind —, to the draft by
// the split button or Ctrl+Enter; without `<sorte>.edit`, both actions refused, named.

const A1 = "0b5e3c1a-0000-4000-8000-0000000000e1";
const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";
const V1 = "0b5e3c1a-0000-4000-8000-0000000000c1";
const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";

const simulated = (over: Partial<Simulated> = {}): Simulated => ({
  valid: true,
  violations: [],
  diff: [],
  sample: [],
  cascaded: [],
  sections: [],
  placements: [],
  placements_truncated: false,
  ...over,
});
const OUTSIDE = {
  rule: "core.address.outside_network",
  severity: "error",
  detail: "Une adresse est dans un réseau de son domaine.",
  subjects: [A1],
};

describe("the closure of an object", () => {
  it("is one operation at the version served, the cascade only when asked", () => {
    expect(closeOperation("address", A1, V1, false)).toEqual({
      op: "close",
      kind: "address",
      id: { id: A1 },
      expected: V1,
    });
    expect(closeOperation("vlan_group", A1, null, true)).toEqual({
      op: "close",
      kind: "vlan_group",
      id: { id: A1 },
      cascade: true,
    });
  });

  it("says the rules that refuse it, and whether the cascade may answer them", () => {
    const refused = simulated({ valid: false, violations: [OUTSIDE] });
    expect(closeVerdict("prefix", refused, false)).toMatchObject({
      valid: false,
      errors: [{ rule: "core.address.outside_network" }],
      dependents: true,
    });
    // Asked already, or a kind that holds nothing: the cascade is not the answer.
    expect(closeVerdict("prefix", refused, true).dependents).toBe(false);
    expect(closeVerdict("address", refused, false).dependents).toBe(false);
    // Objects of a module still refer to an address: the cascade may close or detach them.
    const referred = simulated({
      valid: false,
      violations: [{ ...OUTSIDE, rule: "core.object.dependents" }],
    });
    expect(closeVerdict("address", referred, false).dependents).toBe(true);
  });

  it("counts the closures a cascade adds by kind, its warnings apart", () => {
    const verdict = closeVerdict(
      "prefix",
      simulated({
        cascaded: [
          { kind: "address", id: A1 },
          { kind: "range", id: P1 },
          { kind: "address", id: V1 },
        ],
        violations: [{ ...OUTSIDE, severity: "warning" }],
      }),
      true,
    );
    expect(verdict.cascaded).toEqual([
      { kind: "address", count: 2 },
      { kind: "range", count: 1 },
    ]);
    expect(verdict.errors).toEqual([]);
    expect(verdict.warnings).toHaveLength(1);
  });
});

describe("the close panel of a record", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
    drafts.setActive(null);
  });

  interface Seen {
    method: string;
    url: string;
    body: Record<string, unknown> | null;
  }
  type Answer = { status?: number; body: unknown; etag?: string };

  async function mounted(
    collection: "addresses" | "prefixes",
    id: string,
    permissions: string[],
    answer: (seen: Seen) => Answer | undefined,
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
      const status = out.status ?? 200;
      return new Response(JSON.stringify(out.body), {
        status,
        headers: {
          "Content-Type": status >= 400 ? "application/problem+json" : "application/json",
          ...(out.etag ? { ETag: out.etag } : {}),
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
        const request = editRequest();
        return () =>
          request.value?.mode === "close"
            ? h(ClosePanel, { request: request.value, key: request.value.serial })
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
    openEdit({ mode: "close", collection, id });
    await nextTick();
    return { seen, root };
  }

  const object = (id: string, value: string) => ({
    body: { id, value, version: V1, domain: { id: D1, slug: "essai", href: `/v1/domains/${D1}` } },
    etag: `"${V1}"`,
  });
  const main = (root: HTMLElement) =>
    root.querySelector<HTMLButtonElement>(".k-split-button__main");
  const writes = (seen: Seen[]) =>
    seen.filter((s) => s.method !== "GET" && !s.url.includes("simulate"));

  it("closes an address to the draft at the version served, its simulation said first", async () => {
    const { seen, root } = await mounted("addresses", A1, ["core.address.edit"], (call) => {
      if (call.url === `/v1/addresses/${A1}`) return object(A1, "198.51.100.10");
      if (call.url.startsWith("/v1/changesets?simulate=true")) return { body: simulated() };
      if (call.method === "POST" && call.url === "/v1/drafts") {
        return {
          status: 201,
          body: {
            id: D1,
            title: "Brouillon",
            author: "alice",
            href: `/v1/drafts/${D1}`,
            created_at: "2026-11-01T10:00:00Z",
            updated_at: "2026-11-01T10:00:00Z",
            version: 1,
            operation_count: 1,
            operations: call.body?.operations,
          },
          etag: '"1"',
        };
      }
      return undefined;
    });
    await expect.poll(() => root.textContent).toContain("La clôture passe les règles");
    expect(root.textContent).toContain("Seul cet objet se clôt.");
    expect(root.querySelector("h2")?.textContent).toContain("198.51.100.10");
    // An address holds nothing: no cascade offered.
    expect(root.textContent).not.toContain("Clore aussi ce qui en dépend");
    // The simulation is that of the closure alone, at the version served.
    const simulation = seen.find((s) => s.url.includes("simulate"))!;
    expect(simulation.body?.operations).toEqual([
      { op: "close", kind: "address", id: { id: A1 }, expected: V1 },
    ]);
    expect(main(root)?.textContent).toContain("Clore au brouillon");
    // Ctrl+Enter runs the chosen action: to the draft.
    root
      .querySelector("form")!
      .dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, bubbles: true }));
    await expect.poll(() => writes(seen).length).toBe(1);
    expect(writes(seen)[0]).toMatchObject({
      method: "POST",
      url: "/v1/drafts",
      body: { operations: [{ op: "close", kind: "address", id: { id: A1 }, expected: V1 }] },
    });
    await expect.poll(() => editRequest().value).toBeNull();
  });

  it("says a prefix refused by a rule with its code, then the dependents a cascade closes", async () => {
    const { seen, root } = await mounted(
      "prefixes",
      P1,
      ["core.prefix.edit", "core.prefix.approval.self"],
      (call) => {
        if (call.url === `/v1/prefixes/${P1}`) return object(P1, "198.51.100.0/24");
        if (call.url.startsWith("/v1/changesets?simulate=true")) {
          const [op] = call.body?.operations as { cascade?: boolean }[];
          return {
            body: op?.cascade
              ? simulated({
                  cascaded: [
                    { kind: "address", id: A1 },
                    { kind: "range", id: V1 },
                  ],
                })
              : simulated({ valid: false, violations: [OUTSIDE] }),
          };
        }
        return undefined;
      },
    );
    await expect.poll(() => root.textContent).toContain("Clôture refusée par les règles");
    expect(root.textContent).toContain("core.address.outside_network");
    expect(root.textContent).toContain("cochez « Clore aussi ce qui en dépend »");
    const cascade = root.querySelector<HTMLInputElement>(".k-close-panel__cascade input")!;
    // The focus on the cascade, the first thing to decide.
    expect(document.activeElement).toBe(cascade);
    cascade.click();
    await expect.poll(() => root.textContent).toContain("Se closent avec lui");
    expect(root.textContent).toContain("La clôture passe les règles");
    const last = seen.filter((s) => s.url.includes("simulate")).at(-1)!;
    expect(last.body?.operations).toEqual([
      { op: "close", kind: "prefix", id: { id: P1 }, expected: V1, cascade: true },
    ]);
    expect(writes(seen)).toEqual([]);
  });

  it("refuses both actions without `<sorte>.edit`, the permission named", async () => {
    const { seen, root } = await mounted("addresses", A1, ["core.address.read"], (call) => {
      if (call.url === `/v1/addresses/${A1}`) return object(A1, "198.51.100.10");
      if (call.url.startsWith("/v1/changesets?simulate=true")) return { body: simulated() };
      return undefined;
    });
    await expect.poll(() => main(root)).not.toBeNull();
    const button = main(root)!;
    expect(button.getAttribute("aria-disabled")).toBe("true");
    const described = button.getAttribute("aria-describedby")!;
    expect(document.getElementById(described)?.textContent).toContain("core.address.edit");
    button.click();
    root
      .querySelector("form")!
      .dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey: true, bubbles: true }));
    await nextTick();
    expect(writes(seen)).toEqual([]);
    // At the keyboard, the refusal said by the status, its permission named.
    expect(root.querySelector("[role='status']:last-of-type")?.textContent).toContain(
      "il faut core.address.edit",
    );
  });
});
