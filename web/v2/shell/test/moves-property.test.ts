// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { TreeGrid, type MoveIntent, type TreeNode } from "@kartenn/ui";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, nextTick, shallowRef, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, type Me } from "../src/rights/index.ts";
import { useMoves, type Moves } from "../src/views/edit/moves.ts";

// Property test of the moves in a tree whose parent is declared (task 021-24; REQ-021-32,
// 33, 34): a `TreeGrid` mounted with the moves of the shell against a fake server of
// entities, a fixed seed, 200 drops — to the draft or applied now, accepted by the client
// or refused by its check (a cycle), refused by the server after the client accepted them —
// and after each: while the answer is held, the node at its place, « en attente »; once
// answered, each row rendered under the parent the server gives it (the display guard of
// the component also throws on a row misplaced). No tree with a declared parent is on a
// screen of the shell yet (task 18): the journey `moves` cannot drag one.

const ID = (n: number) => `0b5e3c1a-0000-4000-8000-0000000000${String(n).padStart(2, "0")}`;
const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";

/** A generator of numbers in [0, 1), from a fixed seed (mulberry32). */
function seeded(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The ancestors of `id` in a map of parents, from its parent up. */
function ancestorsIn(parents: ReadonlyMap<string, string | null>, id: string): string[] {
  const up: string[] = [];
  for (let at = parents.get(id) ?? null; at !== null; at = parents.get(at) ?? null) up.push(at);
  return up;
}

/** The nodes of a map of parents, as a tree. */
function treeOf(parents: ReadonlyMap<string, string | null>): TreeNode[] {
  const children = (parent: string | null): TreeNode[] =>
    [...parents]
      .filter(([, p]) => p === parent)
      .map(([id]) => {
        const kids = children(id);
        return {
          id,
          label: `e${Number(id.slice(-2))}`,
          ...(kids.length ? { children: kids } : {}),
        };
      });
  return children(null);
}

/** The rows rendered, each with its visual parent (the nearest row above of level − 1). */
function rendered(root: HTMLElement): Map<string, string | null> {
  const stack: string[] = [];
  const out = new Map<string, string | null>();
  for (const row of root.querySelectorAll<HTMLElement>(
    "[role='row'][data-node]:not([data-more])",
  )) {
    const level = Number(row.getAttribute("aria-level"));
    const key = row.dataset.node!;
    stack.length = level - 1;
    out.set(key, level === 1 ? null : (stack[level - 2] ?? null));
    stack[level - 1] = key;
  }
  return out;
}

/** A drag of `from` over `to`, dropped, by the events of the platform. */
function drag(root: HTMLElement, from: string, to: string): void {
  const row = (id: string) => root.querySelector<HTMLElement>(`[role='row'][data-node="${id}"]`)!;
  const fire = (el: HTMLElement, type: string) =>
    el.dispatchEvent(new Event(type, { bubbles: true, cancelable: true }));
  fire(row(from), "dragstart");
  fire(row(to), "dragenter");
  fire(row(to), "dragover");
  fire(row(to), "drop");
  fire(row(from), "dragend");
}

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), {
    status,
    headers: {
      "Content-Type": status >= 400 ? "application/problem+json" : "application/json",
      ETag: '"1"',
    },
  });
const refusal = (rule: string, subject: string) =>
  json(422, {
    type: "urn:kartenn:problem:changeset-refused",
    title: "Refused",
    status: 422,
    violations: [{ rule, severity: "error", detail: rule, subjects: [subject], pointer: null }],
  });

describe("moves in a tree whose parent is declared: property", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
    document.body.innerHTML = "";
  });

  it("200 drops, accepted or refused: never moved before the answer, always under the server's parent", async () => {
    const random = seeded(0x2124);
    // The server: e1 { e2 { e3 }, e4 }, e5 { e6 }, e7, e8.
    const server = new Map<string, string | null>([
      [ID(1), null],
      [ID(2), ID(1)],
      [ID(3), ID(2)],
      [ID(4), ID(1)],
      [ID(5), null],
      [ID(6), ID(5)],
      [ID(7), null],
      [ID(8), null],
    ]);
    const draftOps: unknown[] = [];
    // Each write answered when the step releases it; a rule of the server refuses a share
    // of the moves the client accepts.
    let release: () => void = () => {};
    let held = false;
    let refuseNext = false;
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      const method = init?.method ?? "GET";
      const body = init?.body ? (JSON.parse(String(init.body)) as Record<string, unknown>) : {};
      if (method === "GET" && url.startsWith(`/v1/drafts/${D1}`)) {
        return json(200, { id: D1, title: "b", version: 1, operations: draftOps });
      }
      if (method === "POST" && url === "/v1/drafts") {
        draftOps.push(...(body.operations as unknown[]));
        return json(201, { id: D1, title: "b", version: 1, operations: draftOps });
      }
      if (method === "PATCH" && url === `/v1/drafts/${D1}`) {
        draftOps.splice(0, draftOps.length, ...(body.operations as unknown[]));
        return json(200, { id: D1, title: "b", version: 2, operations: draftOps });
      }
      if (method === "POST" && url === `/v1/drafts/${D1}/operations`) {
        draftOps.push(...(body.operations as unknown[]));
        return json(200, { id: D1, title: "b", version: 2, operations: draftOps });
      }
      if (method === "POST" && url.endsWith("/simulation")) {
        return json(200, {
          valid: true,
          violations: [],
          diff: [],
          sample: [],
          cascaded: [],
          sections: [],
        });
      }
      if (method === "POST" && url.startsWith("/v1/changesets")) {
        held = true;
        await new Promise<void>((done) => (release = done));
        const [op] = body.operations as { id: { id: string }; parent: { id: string } | null }[];
        const id = op!.id.id;
        const parent = op!.parent?.id ?? null;
        if (parent !== null && (parent === id || ancestorsIn(server, parent).includes(id))) {
          return refusal("core.anchor.cycle", id);
        }
        if (refuseNext) return refusal("core.entity.locked", id);
        server.set(id, parent);
        return json(201, { changeset: { id: D1 }, warnings: [], cascaded: [] });
      }
      return json(200, { items: [], links: {} });
    }) as typeof fetch);
    await loadSession(async (url) =>
      url === "/v1/me"
        ? ({
            identity: "alice",
            kind: "person",
            global_permissions: ["core.entity.edit", "core.entity.approval.self"],
            bindings: [],
          } as unknown as Me)
        : null,
    );

    // The host: the tree as last read from the server, read again once a move is applied.
    const read = shallowRef(new Map(server));
    const all = [...server.keys()];
    let moves!: Moves;
    let inFlight: Promise<string> | null = null;
    let answered = false;
    let now = false;
    const Host = defineComponent({
      setup() {
        moves = useMoves({
          subject: (n) => ({
            id: n.id,
            type: "core.entity",
            version: "v",
            parent: read.value.get(n.id) ?? null,
          }),
          ancestors: (id) => ancestorsIn(read.value, id),
        });
        const onMove = (intent: MoveIntent) => {
          inFlight = moves.move(intent, { now, motive: "property" }).then((outcome) => {
            answered = true;
            if (outcome.kind === "applied") read.value = new Map(server);
            return outcome.kind;
          });
        };
        return () =>
          h(TreeGrid, {
            label: "Entités",
            treeLabel: "Entité",
            nodes: treeOf(read.value),
            expanded: all,
            parentMode: "declared",
            canMove: moves.canMove,
            moveState: (n: TreeNode) => moves.moveState(n),
            onMove,
          });
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
    app.config.errorHandler = (error) => {
      throw error;
    };
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    await nextTick();

    const counts = {
      applied: 0,
      refusedByClient: 0,
      refusedByServer: 0,
      drafted: 0,
      unchanged: 0,
    };
    for (let step = 1; step <= 200; step++) {
      const before = new Map(server);
      const node = all[Math.floor(random() * all.length)]!;
      const target = all[Math.floor(random() * all.length)]!;
      now = random() < 0.6;
      refuseNext = random() < 0.2;
      inFlight = null;
      held = false;
      answered = false;
      moves.settle(node);
      drag(root, node, target);
      await nextTick();
      if (inFlight === null) {
        // Refused by the check of the client (a cycle: no intent), nothing sent.
        counts.refusedByClient += 1;
      } else {
        if (now) {
          // Applied now: its answer held, the node at its place, « en attente » (X33).
          await expect.poll(() => held || answered, { message: `step ${step}` }).toBe(true);
          if (held) {
            await nextTick();
            expect(moves.moveState({ id: node, label: "" }), `step ${step}`).toBe("pending");
            expect(rendered(root).get(node), `step ${step}`).toBe(before.get(node));
            release();
          }
        }
        const kind = await inFlight;
        await nextTick();
        if (kind === "unchanged") counts.unchanged += 1;
        else if (kind === "drafted") counts.drafted += 1;
        else if (kind === "applied") counts.applied += 1;
        else {
          counts.refusedByServer += 1;
          expect(moves.moveState({ id: node, label: "" }), `step ${step}`).toMatchObject({
            refused: { ok: false },
          });
        }
      }
      // The draft never moves a node of the tree; nothing moves unless the server applied.
      if (inFlight === null || (await inFlight) !== "applied") {
        expect(server, `step ${step}`).toEqual(before);
      }
      // Every row rendered under the parent the server gives it.
      expect(Object.fromEntries(rendered(root)), `step ${step}`).toEqual(
        Object.fromEntries(server),
      );
    }
    // Every kind of outcome exercised (a drop on its own parent changes nothing).
    for (const [kind, count] of Object.entries(counts)) {
      if (kind !== "unchanged") expect(count, kind).toBeGreaterThan(10);
    }
    // The simulation of each draft waits 400 ms (SIMULATION_WAIT): a minute at most.
  }, 120_000);
});
