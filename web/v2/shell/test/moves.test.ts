// @vitest-environment happy-dom
import { queryClient, reads } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import type { TreeNode } from "@kartenn/ui";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { useMoves, type Moves } from "../src/views/edit/moves.ts";

// The moves of the shell in a tree whose parent is declared (task 021-24; REQ-021-32, 33):
// `canMove` in words, with the code of the server rule; a move to the draft, marked at its
// place once its simulation holds; applied now, « en attente » at its place until the
// answer, never written in the cache of the tree; refused by the server after the client
// accepted it, left at its place with the code.

// Entities: n1 at the root, n2 and n3 under it, n4 under n3; r at the root.
const ID = (n: number) => `0b5e3c1a-0000-4000-8000-00000000000${n}`;
const PARENT: Record<string, string> = { [ID(2)]: ID(1), [ID(3)]: ID(1), [ID(4)]: ID(3) };
const node = (n: number): TreeNode => ({ id: ID(n), label: `e${n}` });
const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";
const C1 = "0b5e3c1a-0000-4000-8000-0000000000c1";

interface Seen {
  method: string;
  url: string;
  body: unknown;
}
type Answer = { status: number; body: unknown; etag?: string } | Promise<Answer>;

const LIST_KEY = reads.list("/v1/entities").queryKey;
const LISTED = {
  data: {
    items: [1, 2, 3, 4].map((n) => ({ id: ID(n), slug: `e${n}`, version: `v${n}` })),
    links: {},
  },
};

const simulated = (valid: boolean, violations: unknown[] = []) => ({
  valid,
  violations,
  diff: [],
  sample: [],
  cascaded: [],
  sections: [],
});
const draftBody = (operations: unknown[]) => ({
  id: D1,
  title: "Brouillon",
  version: 1,
  operation_count: operations.length,
  operations,
});
const applied = { changeset: { id: C1 }, warnings: [], cascaded: [] };

describe("moves in a tree whose parent is declared", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
  });

  async function mounted(permissions: string[], answer: (seen: Seen) => Answer, at = "/") {
    const seen: Seen[] = [];
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const call = {
        method: init?.method ?? "GET",
        url: String(input),
        body: init?.body ? JSON.parse(String(init.body)) : null,
      };
      seen.push(call);
      const { status, body, etag } = await answer(call);
      return new Response(JSON.stringify(body), {
        status,
        headers: {
          "Content-Type": status >= 400 ? "application/problem+json" : "application/json",
          ...(etag ? { ETag: etag } : {}),
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
    let moves!: Moves;
    const Host = defineComponent({
      setup() {
        moves = useMoves({
          subject: (n) => ({
            id: n.id,
            type: "core.entity",
            version: `v${n.id.at(-1)}`,
            parent: PARENT[n.id] ?? null,
          }),
          ancestors(id) {
            const up: string[] = [];
            for (let at = PARENT[id]; at !== undefined; at = PARENT[at]) up.push(at);
            return up;
          },
        });
        return () => h("div");
      },
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: Host }],
    });
    await router.push(at);
    app = createApp({ render: () => h(RouterView) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(document.createElement("div"));
    await router.isReady();
    await nextTick();
    // The list the tree is drawn from, in the cache: a move never writes it.
    queryClient.setQueryData(LIST_KEY, { pages: [LISTED], pageParams: [null] });
    return { moves: () => moves, seen };
  }

  const EDIT = ["core.entity.edit", "core.entity.approval.self"];
  const writes = (seen: Seen[]) => seen.filter((s) => s.method !== "GET");

  it("says each refusal in words, with the code of the server rule", async () => {
    const { moves } = await mounted(EDIT, () => ({ status: 200, body: {} }));
    expect(moves().canMove(node(2), node(3))).toEqual({ ok: true });
    expect(moves().canMove(node(1), null)).toEqual({ ok: true });
    // Onto itself, onto a descendant: a cycle, `core.anchor.cycle`.
    expect(moves().canMove(node(1), node(4))).toEqual({
      ok: false,
      reason: "Cible refusée : e4 est l'objet déplacé ou l'un de ses descendants.",
      code: "core.anchor.cycle",
    });
    expect(moves().canMove(node(3), node(3))).toMatchObject({ code: "core.anchor.cycle" });
    // A node the host does not move, or a target of no admitted type.
    const reader = await mounted(["core.entity.read"], () => ({ status: 200, body: {} }));
    expect(reader.moves().canMove(node(2), node(3))).toEqual({
      ok: false,
      reason: "Droit manquant : il faut core.entity.edit.",
    });
  });

  it("refuses every move off the present, and writes nothing", async () => {
    const { moves, seen } = await mounted(
      EDIT,
      () => ({ status: 200, body: {} }),
      "/?valid_at=2026-01-01T00:00:00Z",
    );
    expect(moves().canMove(node(2), node(3))).toEqual({
      ok: false,
      reason: "Lecture datée : revenez au présent pour changer de parent.",
    });
    const outcome = await moves().move({ node: node(2), target: node(3) });
    expect(outcome.kind).toBe("refused");
    expect(moves().moveState(node(2))).toMatchObject({ refused: { ok: false } });
    expect(writes(seen)).toEqual([]);
  });

  it("a move to the draft: its operation at the version read, marked once simulated", async () => {
    const { moves, seen } = await mounted(EDIT, (call) =>
      call.method === "POST" && call.url === "/v1/drafts"
        ? {
            status: 201,
            body: draftBody((call.body as { operations: unknown[] }).operations),
            etag: '"1"',
          }
        : call.url.endsWith("/simulation")
          ? { status: 200, body: simulated(true) }
          : { status: 200, body: {} },
    );
    const moving = moves().move({ node: node(4), target: node(2) });
    expect(moves().moveState(node(4))).toBe("pending");
    expect(await moving).toMatchObject({ kind: "drafted", valid: true });
    expect(moves().moveState(node(4))).toBeUndefined();
    expect(moves().inDraft(ID(4))).toBe(true);
    const post = writes(seen).find((s) => s.url === "/v1/drafts")!;
    expect((post.body as { operations: unknown[] }).operations).toEqual([
      { op: "move_entity", id: { id: ID(4) }, parent: { id: ID(2) }, expected: "v4" },
    ]);
    // The tree as read, untouched.
    expect(queryClient.getQueryData(LIST_KEY)).toEqual({ pages: [LISTED], pageParams: [null] });
  });

  it("a draft whose simulation refuses the move: the node refused at its place, its code", async () => {
    const cycle = {
      rule: "core.anchor.cycle",
      severity: "error",
      detail: "an entity cannot be its own ancestor",
      subjects: [ID(4)],
      operation: 0,
    };
    const { moves } = await mounted(EDIT, (call) =>
      call.method === "POST" && call.url === "/v1/drafts"
        ? { status: 201, body: draftBody([{}]), etag: '"1"' }
        : call.url.endsWith("/simulation")
          ? { status: 200, body: simulated(false, [cycle]) }
          : { status: 200, body: {} },
    );
    expect(await moves().move({ node: node(4), target: node(2) })).toMatchObject({
      kind: "drafted",
      valid: false,
    });
    expect(moves().moveState(node(4))).toEqual({
      refused: { ok: false, reason: cycle.detail, code: "core.anchor.cycle" },
    });
    expect(moves().inDraft(ID(4))).toBe(false);
  });

  it("applied now: « en attente » at its place through a slow answer, nothing written", async () => {
    let release!: () => void;
    const answered = new Promise<void>((done) => (release = done));
    const { moves, seen } = await mounted(EDIT, async (call) => {
      if (call.method === "POST" && call.url.startsWith("/v1/changesets")) {
        await answered;
        return { status: 201, body: applied };
      }
      return { status: 200, body: LISTED.data };
    });
    const moving = moves().move(
      { node: node(2), target: node(3) },
      { now: true, motive: "réorganisation" },
    );
    await expect.poll(() => writes(seen).length).toBe(1);
    // In flight: pending, at its place — the cache of the tree as read.
    expect(moves().moveState(node(2))).toBe("pending");
    expect(queryClient.getQueryData(LIST_KEY)).toEqual({ pages: [LISTED], pageParams: [null] });
    // A second intent while one is in flight changes nothing.
    expect(await moves().move({ node: node(2), target: null })).toEqual({ kind: "unchanged" });
    release();
    expect(await moving).toMatchObject({ kind: "applied", changeset: C1 });
    expect(moves().moveState(node(2))).toBeUndefined();
    expect(writes(seen)).toEqual([
      {
        method: "POST",
        url: "/v1/changesets?approval=self",
        body: {
          motive: "réorganisation",
          operations: [
            { op: "move_entity", id: { id: ID(2) }, parent: { id: ID(3) }, expected: "v2" },
          ],
        },
      },
    ]);
    // The lists are invalidated (X24): the tree is read again, never retouched.
    expect(queryClient.getQueryState(LIST_KEY)?.isInvalidated).toBe(true);
  });

  it("accepted by the client, refused by the server: at its place, refused with its code", async () => {
    const { moves } = await mounted(EDIT, (call) =>
      call.method === "POST"
        ? {
            status: 422,
            body: {
              type: "https://kartenn.example/problems/invalid",
              title: "Invalid",
              status: 422,
              violations: [
                {
                  rule: "core.anchor.cycle",
                  severity: "error",
                  detail: "an entity cannot be its own ancestor",
                  subjects: [ID(1)],
                  pointer: "/operations/0",
                },
              ],
            },
          }
        : { status: 200, body: {} },
    );
    // e1 under r (unknown to the client's ancestors): accepted here.
    const r: TreeNode = { id: ID(9), label: "r" };
    expect(moves().canMove(node(1), r)).toEqual({ ok: true });
    expect(
      await moves().move({ node: node(1), target: r }, { now: true, motive: "m" }),
    ).toMatchObject({ kind: "invalid" });
    expect(moves().moveState(node(1))).toEqual({
      refused: {
        ok: false,
        reason: "an entity cannot be its own ancestor",
        code: "core.anchor.cycle",
      },
    });
    expect(queryClient.getQueryData(LIST_KEY)).toEqual({ pages: [LISTED], pageParams: [null] });
  });

  it("a move onto the parent it has changes nothing", async () => {
    const { moves, seen } = await mounted(EDIT, () => ({ status: 200, body: {} }));
    expect(await moves().move({ node: node(2), target: node(1) })).toEqual({ kind: "unchanged" });
    expect(moves().moveState(node(2))).toBeUndefined();
    expect(writes(seen)).toEqual([]);
  });
});
