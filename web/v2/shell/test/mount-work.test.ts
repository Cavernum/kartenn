// @vitest-environment happy-dom
import { queryClient, reads } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, defineComponent, h, nextTick, ref, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { useQuery } from "../src/data/query.ts";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import OccupancyPanel from "../src/views/record/OccupancyPanel.vue";

// The work of a record's mount (task 021-22o): a record opened from the cache is drawn
// without a request — the reads its route does not declare (the occupancy of a prefix,
// its next free value) leave once it is drawn, what the cache holds shown at once — and a
// read of a screen builds its options once at its mount, not three times.

const ID = "0b5e3c1a-0000-4000-8000-0000000000a1";
const PRESENT = { validAt: null, asOf: null };

describe("the work of a record's mount", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    document.body.replaceChildren();
  });

  /** The API answered, each URL asked kept in `seen`. */
  function served(): string[] {
    const seen: string[] = [];
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      seen.push(url);
      const body = url.endsWith("/occupancy")
        ? { prefix: { id: ID, value: "192.0.2.0/24" }, nature: "network", used: "10", total: "256" }
        : url.endsWith("/next-free")
          ? { value: "192.0.2.11", links: {} }
          : { lang: "fr", default_lang: "fr", revision: "1", spaces: {} };
      return new Response(JSON.stringify(body), {
        headers: { "Content-Type": "application/json", ETag: '"v1"' },
      });
    }) as typeof fetch);
    return seen;
  }

  /** `component` mounted under the router of the frame, at the present. */
  async function mounted(component: Parameters<typeof h>[0], props: object = {}) {
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: RouterView, children: frameRoutes }],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({ render: () => h(component, props) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    return root;
  }
  const occupancyReads = (seen: string[]) =>
    seen.filter((url) => url.endsWith("/occupancy") || url.endsWith("/next-free"));

  it("reads the occupancy of a prefix once the record is drawn, not while it mounts", async () => {
    const seen = served();
    const root = await mounted(OccupancyPanel, { id: ID });
    await nextTick();
    // Mounted, before its first frame: its skeleton, no read gone.
    expect(occupancyReads(seen)).toEqual([]);
    expect(root.querySelector(".k-skeleton-fill, [aria-busy='true']")).not.toBeNull();
    // Drawn: both read, the panel filled.
    await expect.poll(() => occupancyReads(seen).length).toBe(2);
    await expect.poll(() => root.textContent).toContain("192.0.2.11");
  });

  it("shows at once an occupancy the cache holds, without reading it again", async () => {
    const seen = served();
    await queryClient.fetchQuery(reads.sub("prefix", ID, "occupancy", null, PRESENT));
    await queryClient.fetchQuery(reads.sub("prefix", ID, "next-free", null, PRESENT));
    const before = occupancyReads(seen).length;
    const root = await mounted(OccupancyPanel, { id: ID });
    await nextTick();
    expect(root.textContent).toContain("192.0.2.11");
    expect(root.querySelector(".k-skeleton-fill")).toBeNull();
    // Fresh in the cache: drawn, still not read again.
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(occupancyReads(seen).length).toBe(before);
  });

  it("builds the options of a read once at the mount, again on each change", async () => {
    const seen = served();
    const OTHER = "0b5e3c1a-0000-4000-8000-0000000000a2";
    const id = ref(ID);
    let built = 0;
    const Reader = defineComponent({
      setup() {
        const read = useQuery(() => {
          built += 1;
          return reads.sub("prefix", id.value, "occupancy", null, PRESENT);
        });
        return () => h("span", String(read.status.value));
      },
    });
    await mounted(Reader);
    expect(built).toBe(1);
    id.value = OTHER;
    expect(built).toBe(2);
    // The observer follows: the other prefix is read.
    await expect.poll(() => seen.some((url) => url.includes(`/${OTHER}/occupancy`))).toBe(true);
    expect(built).toBe(2);
  });
});
