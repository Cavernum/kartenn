// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";

// The tab « Historique » of a record (task 021-11p5; REQ-021-9; maquette E2): each version
// with the motive of its changeset, a link to it, and its author as the server names it —
// a pseudonymised person « personne #… », never a name kept by the client —, the history
// read again at each use (never frozen, Q-021-29).

const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";
const C1 = "0b5e3c1a-0000-4000-8000-0000000000c1";
const C2 = "0b5e3c1a-0000-4000-8000-0000000000c2";
const I1 = "0b5e3c1a-0000-4000-8000-0000000000e1";
const I2 = "0b5e3c1a-0000-4000-8000-0000000000e2";

const version = (
  changeset: string,
  value: string,
  from: string,
  to: string | null,
  motive: string,
  author: { id: string; name: string },
) => ({
  object: { id: P1, value, links: {} },
  changeset: { id: changeset, href: `/v1/changesets/${changeset}` },
  motive,
  author: { ...author, href: `/v1/identities/${author.id}` },
  valid_from: from,
  valid_to: to,
  asserted_from: from,
  asserted_to: null,
});

describe("the tab « Historique », its motives and its authors", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    localStorage.clear();
    setFetch(null);
    document.body.innerHTML = "";
  });

  /** The tab of P1 against a fake API whose history answers `history()`; the URLs read. */
  async function mount(history: () => unknown): Promise<{ root: HTMLElement; seen: string[] }> {
    const seen: string[] = [];
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      seen.push(url);
      const body = url.startsWith(`/v1/prefixes/${P1}/history`)
        ? history()
        : { items: [], links: {} };
      return new Response(JSON.stringify(body), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }) as typeof fetch);
    await loadSession(async (url) =>
      url === "/v1/me"
        ? ({
            identity: "alice",
            kind: "person",
            global_permissions: [],
            bindings: [],
          } as unknown as Me)
        : null,
    );
    const { default: HistoryTab } = await import("../src/views/record/HistoryTab.vue");
    const Host = defineComponent({
      setup: () => () => h(HistoryTab, { kind: "prefix", id: P1, name: "192.0.2.0/25" }),
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: "/", component: Host },
        { path: "/changesets/:id", component: { render: () => null } },
      ],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp(RouterView);
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    return { root, seen };
  }
  /** The text a cell shows, its spaces folded. */
  const text = (root: HTMLElement, row: string, column: string) =>
    root
      .querySelector(`[data-row="${row}"][data-column="${column}"]`)
      ?.textContent?.replace(/\s+/g, " ")
      .trim();
  const rowOf = (changeset: string, from: string) => `${changeset}|${from}|${from}`;

  it("names the motive of each version, a link to its changeset, and its author", async () => {
    const t1 = "2026-03-01T08:00:00Z";
    const t2 = "2026-06-01T09:30:00Z";
    const { root } = await mount(() => ({
      items: [
        version(C2, "192.0.2.0/25", t2, null, "Renumérotation de la salle B", {
          id: I1,
          name: "Alice Exemple",
        }),
        version(C1, "192.0.2.0/24", t1, t2, "Import initial", { id: I2, name: "Bob Exemple" }),
      ],
      links: {},
    }));
    await expect
      .poll(() => text(root, rowOf(C2, t2), "motive"))
      .toBe("Renumérotation de la salle B");
    expect(text(root, rowOf(C2, t2), "author")).toBe("Alice Exemple");
    expect(text(root, rowOf(C1, t1), "motive")).toBe("Import initial");
    expect(text(root, rowOf(C1, t1), "author")).toBe("Bob Exemple");
    expect(text(root, rowOf(C1, t1), "value")).toBe("192.0.2.0/24");
    const link = root.querySelector(`[data-row="${rowOf(C2, t2)}"][data-column="motive"] a`);
    expect(link?.getAttribute("href")).toBe(`/changesets/${C2}`);
    // The headers say the columns.
    const headers = [...root.querySelectorAll('[role="columnheader"]')].map((h) =>
      h.textContent?.trim(),
    );
    expect(headers).toEqual(expect.arrayContaining(["Valeur", "Motif", "Auteur"]));
  });

  it("shows a pseudonymised author as served, and reads the history again at each use", async () => {
    let name = "Alice Exemple";
    const t1 = "2026-03-01T08:00:00Z";
    const served = () => ({
      items: [version(C1, "192.0.2.0/25", t1, null, "", { id: I1, name })],
      links: {},
    });
    const first = await mount(served);
    await expect.poll(() => text(first.root, rowOf(C1, t1), "author")).toBe("Alice Exemple");
    // A blank motive: the link names the changeset, never an empty link.
    expect(text(first.root, rowOf(C1, t1), "motive")).toBe(`Changeset ${C1}`);
    const id = first.root.querySelector(
      `[data-row="${rowOf(C1, t1)}"][data-column="motive"] a .k-mono`,
    );
    expect(id?.textContent).toBe(C1);
    app?.unmount();
    app = undefined;
    document.body.innerHTML = "";

    // The person pseudonymised meanwhile: the tab shown again reads the history again, at
    // once (never kept fresh), and says the pseudonym the server serves, nothing else.
    name = "personne #3f2a";
    const again = await mount(served);
    await expect.poll(() => text(again.root, rowOf(C1, t1), "author")).toBe("personne #3f2a");
    expect(again.root.textContent).not.toContain("Alice Exemple");
    const reads = again.seen.filter((url) => url.startsWith(`/v1/prefixes/${P1}/history`));
    // Its pages still in the cache, read again all the same (fresh for 30 s otherwise).
    expect(queryClient.getQueryCache().findAll({ queryKey: ["v1", "prefix", P1] })).not.toEqual([]);
    expect(reads.length).toBeGreaterThanOrEqual(1);
    expect(JSON.stringify(localStorage)).not.toContain("Alice");
    expect(JSON.stringify(sessionStorage)).not.toContain("Alice");
  });
});
