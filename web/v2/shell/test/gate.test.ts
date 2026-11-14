// @vitest-environment happy-dom
import { queryClient, reads } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, h, nextTick, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { COLUMN_READS, limited } from "../src/data/gate.ts";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import ObjectName from "../src/views/record/ObjectName.vue";

// The one gate of the reads the shell makes row by row (task 021-11p10-a): at most
// `COLUMN_READS` in flight for the whole shell, a read given up before its turn never sent;
// the name of an object read only in view, through the gate, given up once nothing in view
// wants it; a read waiting out its `Retry-After` holds no turn (task 11p10-b).

/** The identifier of the `n`-th object. */
const idOf = (n: number) => `0b5e3c1a-0000-4000-8000-${String(n).padStart(12, "0")}`;

describe("the gate of the reads row by row", () => {
  it("read a few at a time, never a read given up meanwhile", async () => {
    let running = 0;
    let most = 0;
    const releases: (() => void)[] = [];
    const read = () =>
      new Promise<string>((resolve) => {
        running++;
        most = Math.max(most, running);
        releases.push(() => {
          running--;
          resolve("read");
        });
      });
    const gone = new AbortController();
    const outcome = (signal?: AbortSignal) =>
      limited(signal, read).then(
        () => "read",
        () => "dropped",
      );
    const all = [
      ...Array.from({ length: COLUMN_READS + 2 }, () => outcome()),
      outcome(gone.signal),
    ];
    gone.abort();
    // Given up while it waits: out of the queue at once, before any read ahead ends.
    expect(await all.at(-1)).toBe("dropped");
    for (let i = 0; i < 20 && releases.length > 0; i++) {
      releases.shift()!();
      await new Promise((resolve) => setTimeout(resolve));
    }
    const settled = await Promise.all(all);
    expect(most).toBe(COLUMN_READS);
    expect(settled.filter((s) => s === "read")).toHaveLength(COLUMN_READS + 2);
  });

  it("free the turn of a read refused by load while it waits out its Retry-After", async () => {
    vi.useFakeTimers();
    try {
      const sent: string[] = [];
      const held: (() => void)[] = [];
      let refused = false;
      setFetch((async (input: RequestInfo | URL) => {
        const url = String(input);
        sent.push(url);
        if (!refused) {
          refused = true;
          return new Response(JSON.stringify({ type: "urn:kartenn:problem:x", status: 429 }), {
            status: 429,
            headers: { "Content-Type": "application/problem+json", "Retry-After": "1" },
          });
        }
        await new Promise<void>((resolve) => held.push(resolve));
        return new Response(JSON.stringify({ id: url.split("/").at(-1), value: "192.0.2.0/24" }), {
          headers: { "Content-Type": "application/json", ETag: '"v1"' },
        });
      }) as typeof fetch);
      const fetchOf = (n: number) => {
        const read = reads.object("prefix", idOf(n), null);
        return queryClient.fetchQuery({
          ...read,
          queryFn: (context) => limited(context.signal, () => read.queryFn(context)),
        });
      };
      const all = Array.from({ length: COLUMN_READS + 1 }, (_, n) => fetchOf(n));
      // The first refused: while it waits, its turn goes to the read behind the others.
      await vi.advanceTimersByTimeAsync(10);
      expect(sent).toHaveLength(COLUMN_READS + 1);
      expect(sent.at(-1)).toContain(idOf(COLUMN_READS));
      // Its wait out, it takes its turn again once one is free, never one more in flight.
      await vi.advanceTimersByTimeAsync(1_600);
      expect(sent).toHaveLength(COLUMN_READS + 1);
      held.shift()!();
      await vi.advanceTimersByTimeAsync(10);
      expect(sent).toHaveLength(COLUMN_READS + 2);
      expect(sent.at(-1)).toContain(idOf(0));
      while (held.length > 0) {
        held.shift()!();
        await vi.advanceTimersByTimeAsync(10);
      }
      await Promise.all(all);
    } finally {
      vi.useRealTimers();
      queryClient.clear();
      resetContext();
    }
  });
});

/** An `IntersectionObserver` whose view the test sets: `seen` says whether an element is. */
class View {
  static all = new Set<View>();
  static seen: (target: Element) => boolean = () => true;
  readonly targets = new Set<Element>();
  constructor(readonly callback: IntersectionObserverCallback) {
    View.all.add(this);
  }
  observe(target: Element): void {
    this.targets.add(target);
    queueMicrotask(() => this.tell(target));
  }
  tell(target: Element): void {
    if (!this.targets.has(target)) return;
    const entry = { target, isIntersecting: View.seen(target) } as IntersectionObserverEntry;
    this.callback([entry], this as unknown as IntersectionObserver);
  }
  unobserve(): void {}
  disconnect(): void {
    this.targets.clear();
    View.all.delete(this);
  }
  /** The view moved: each element observed told whether it is in view now. */
  static moved(): void {
    for (const view of View.all) for (const target of view.targets) view.tell(target);
  }
}

describe("the name of an object referenced, row by row", () => {
  let app: App | undefined;
  let host: HTMLElement;
  /** The objects whose record was asked, and the reads in flight. */
  let sent: string[];
  let running: number;
  let most: number;
  let held: (() => void)[];

  beforeEach(() => {
    vi.stubGlobal("IntersectionObserver", View);
    sent = [];
    running = 0;
    most = 0;
    held = [];
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      const body = url.startsWith("/v1/labels")
        ? { lang: "fr", default_lang: "fr", revision: "1", spaces: {} }
        : null;
      if (body === null) {
        const id = new URL(url, "http://kartenn.test").pathname.split("/").at(-1)!;
        sent.push(id);
        running++;
        most = Math.max(most, running);
        await new Promise<void>((resolve) => held.push(resolve));
        running--;
        return new Response(JSON.stringify({ id, value: `name of ${id}` }), {
          headers: { "Content-Type": "application/json", ETag: '"v1"' },
        });
      }
      return new Response(JSON.stringify(body), {
        headers: { "Content-Type": "application/json", ETag: '"v1"' },
      });
    }) as typeof fetch);
  });
  afterEach(() => {
    app?.unmount();
    app = undefined;
    View.seen = () => true;
    vi.unstubAllGlobals();
    queryClient.clear();
    resetContext();
  });

  /** Mounts one name per identifier of `ids`, in this order. */
  async function mount(ids: string[]): Promise<void> {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: RouterView, children: frameRoutes }],
    });
    await router.push("/prefixes");
    host = document.createElement("div");
    document.body.append(host);
    app = createApp({
      render: () => ids.map((id) => h(ObjectName, { id, kind: "prefix", "data-id": id })),
    });
    app.use(createShellI18n("fr")).use(router);
    app.mount(host);
    await router.isReady();
    await settle();
  }
  const settle = async () => {
    for (let i = 0; i < 10; i++) await new Promise((resolve) => setTimeout(resolve));
    await nextTick();
  };
  /** Answers the reads in flight, one at a time, until none is left. */
  async function answerAll(): Promise<void> {
    for (let i = 0; i < 200 && held.length > 0; i++) {
      held.shift()!();
      await settle();
    }
  }
  const idOfCell = (target: Element) => target.getAttribute("data-id") ?? "";

  it("read 66 names a few at a time, none of a cell out of view", async () => {
    const ids = Array.from({ length: 66 }, (_, n) => idOf(n));
    const out = new Set(ids.slice(60));
    View.seen = (target) => !out.has(idOfCell(target));
    await mount(ids);
    expect(running).toBe(COLUMN_READS);
    await answerAll();
    expect(most).toBe(COLUMN_READS);
    expect(new Set(sent)).toEqual(new Set(ids.slice(0, 60)));
    expect(sent).toHaveLength(60);
    expect(host.textContent).toContain(`name of ${idOf(0)}`);
    expect(host.textContent).toContain(idOf(65));
    expect(host.textContent).not.toContain(`name of ${idOf(65)}`);
  });

  it("give up a name scrolled away before its turn, not one another cell still shows", async () => {
    // Eight objects, the last shown twice (the same VLAN on two rows).
    const ids = [...Array.from({ length: 8 }, (_, n) => idOf(n)), idOf(7)];
    await mount(ids);
    expect(sent).toEqual(ids.slice(0, COLUMN_READS));
    // Scrolled: the cells after the first four out of view, but the last one.
    const cells = [...host.querySelectorAll("[data-id]")];
    View.seen = (target) => {
      const at = cells.indexOf(target);
      return at < COLUMN_READS || at === cells.length - 1;
    };
    View.moved();
    await answerAll();
    expect(sent).toEqual([...ids.slice(0, COLUMN_READS), idOf(7)]);
    expect(cells.at(-1)!.textContent).toContain(`name of ${idOf(7)}`);
    // Back in view: the names given up are read then.
    View.seen = () => true;
    View.moved();
    await settle();
    await answerAll();
    expect(new Set(sent)).toEqual(new Set(ids));
    expect(most).toBe(COLUMN_READS);
  });
});
