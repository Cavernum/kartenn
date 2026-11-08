// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createApp, h, nextTick, ref, type App } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { frameRoutes } from "../src/frame/routes.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { useFocusWhenEmptied } from "../src/views/anomalies/anomaly.ts";
import AnomaliesTab from "../src/views/record/AnomaliesTab.vue";

// The focus after the last anomaly of a list is acknowledged (task 021-11i, 2.4.3): the
// grid replaced by the empty state, the focus would fall to the body; armed by the
// acknowledgement, the next rows read, if empty, put it on the empty state.

const settle = async () => {
  await nextTick();
  await nextTick();
};

afterEach(() => {
  document.body.replaceChildren();
});

function setup() {
  const rows = ref<unknown[]>([{ id: "a" }]);
  const empty = document.createElement("div");
  empty.tabIndex = -1;
  document.body.append(empty);
  const target = ref<HTMLElement | null>(empty);
  const { arm } = useFocusWhenEmptied(() => rows.value, target);
  return { rows, empty, arm };
}

describe("useFocusWhenEmptied", () => {
  it("armed, the list read again empty: the focus on the empty state", async () => {
    const { rows, empty, arm } = setup();
    arm();
    rows.value = [];
    await settle();
    expect(document.activeElement).toBe(empty);
  });

  it("not armed, an empty list leaves the focus where it is", async () => {
    const { rows, empty } = setup();
    rows.value = [];
    await settle();
    expect(document.activeElement).not.toBe(empty);
  });

  it("rows left: disarmed, a later empty list takes no focus", async () => {
    const { rows, empty, arm } = setup();
    arm();
    rows.value = [{ id: "b" }];
    await settle();
    rows.value = [];
    await settle();
    expect(document.activeElement).not.toBe(empty);
  });

  it("the focus already somewhere: kept there", async () => {
    const { rows, empty, arm } = setup();
    const button = document.createElement("button");
    document.body.append(button);
    button.focus();
    arm();
    rows.value = [];
    await settle();
    expect(document.activeElement).toBe(button);
    expect(document.activeElement).not.toBe(empty);
  });
});

// The Anomalies tab of a record (tasks 021-11j, 11k): an object's own by its filter `object`
// (A37), within a domain or outside any, never its domain's pages; the next page as the
// grid's window nears the end; a refused cursor starts them again from the first, as the
// lists do (`list.ts`), never resent; a next page refused, the rows kept under its alert.
describe("the Anomalies tab of a record", () => {
  const PREFIX = "0b5e3c1a-0000-4000-8000-0000000000a1";
  const SITE = "0b5e3c1a-0000-4000-8000-0000000000b1";
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
  });

  const anomaly = (id: string, object: string) => ({
    id,
    rule: "core.addr.documentation",
    message: `message ${id}`,
    object: { id: object, kind: "prefix", href: `/v1/prefixes/${object}` },
    opened_at: "2026-01-01T00:00:00Z",
    links: { acknowledgement: `/v1/anomalies/${id}/acknowledgement` },
  });
  const page = (items: object[], next?: string) => ({ items, links: next ? { next } : {} });
  const problem = (status: number, code: string) =>
    new Response(JSON.stringify({ type: `urn:kartenn:problem:${code}`, title: "", status }), {
      status,
      headers: { "Content-Type": "application/problem+json" },
    });

  // A layout for the grid, which happy-dom does not compute: a scroller of 600 pixels, each
  // row in it, so that the window reaches the end of the rows read and asks the next page.
  beforeEach(() => {
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(600);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
      this: HTMLElement,
    ) {
      const top = this.dataset.index === undefined ? 0 : 40 + Number(this.dataset.index) * 32;
      return DOMRect.fromRect({ x: 0, y: top, width: 800, height: top ? 32 : 600 });
    });
  });
  afterEach(() => vi.restoreAllMocks());

  /** The tab of the object `id` of `kind`, the API answered by `answer`. */
  async function mounted(
    answer: (url: string) => object | Response,
    { kind, id }: { kind: string; id: string } = { kind: "prefix", id: PREFIX },
  ) {
    const seen: string[] = [];
    setFetch((async (input: RequestInfo | URL) => {
      const url = String(input);
      seen.push(url);
      const body = url.startsWith("/v1/labels")
        ? { lang: "fr", default_lang: "fr", revision: "1", spaces: {} }
        : url.startsWith("/v1/anomalies")
          ? answer(url)
          : page([]);
      if (body instanceof Response) return body;
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
      render: () => h(AnomaliesTab, { kind, id, name: "net" }),
    });
    app.use(createShellI18n("fr")).use(router);
    document.body.append(root);
    app.mount(root);
    await router.isReady();
    const asked = (cursor: string) => seen.filter((url) => url.includes(`cursor=${cursor}`));
    const first = () =>
      seen.filter((url) => url.startsWith("/v1/anomalies") && !url.includes("cursor="));
    return { root, asked, first };
  }

  it("an object of a domain: its own by its filter, never its domain's", async () => {
    const { root, first } = await mounted(() => page([anomaly("a1", PREFIX)]));
    await expect.poll(() => root.textContent, { timeout: 8_000 }).toContain("message a1");
    expect(first()).toHaveLength(1);
    const url = new URL(first()[0]!, "http://localhost");
    expect(url.searchParams.get("object")).toBe(PREFIX);
    expect(url.searchParams.has("domain")).toBe(false);
    // The object is the record's: no column of the object.
    const headers = [...root.querySelectorAll("[role='columnheader']")].map((el) => el.textContent);
    expect(headers.length).toBeGreaterThan(0);
    expect(headers.some((text) => text?.includes("Objet"))).toBe(false);
  }, 10_000);

  it("an object outside any domain (a site): its own, read", async () => {
    const { root, first } = await mounted(() => page([anomaly("a1", SITE)]), {
      kind: "site",
      id: SITE,
    });
    await expect.poll(() => root.textContent, { timeout: 8_000 }).toContain("message a1");
    expect(new URL(first()[0]!, "http://localhost").searchParams.get("object")).toBe(SITE);
  }, 10_000);

  it("none read (an object unknown or unreadable): the empty state", async () => {
    const { root } = await mounted(() => page([]));
    await expect
      .poll(() => root.textContent, { timeout: 8_000 })
      .toContain("Aucune anomalie ouverte sur cet objet");
  }, 10_000);

  it("the next page as the window nears the end", async () => {
    const { root, asked } = await mounted((url) =>
      url.includes("cursor=c2")
        ? page([anomaly("a2", PREFIX)])
        : page([anomaly("a1", PREFIX)], "/v1/anomalies?cursor=c2"),
    );
    await expect.poll(() => root.textContent, { timeout: 8_000 }).toContain("message a2");
    expect(root.textContent).toContain("message a1");
    expect(asked("c2")).toHaveLength(1);
  }, 10_000);

  it("a refused cursor: the pages read again from the first, never resent, and said", async () => {
    let restarted = false;
    const { root, asked, first } = await mounted((url) => {
      if (url.includes("cursor=c2")) {
        restarted = true;
        return problem(400, "invalid-cursor");
      }
      if (url.includes("cursor=c3")) return page([anomaly("a2", PREFIX)]);
      return page(
        [anomaly("a1", PREFIX)],
        restarted ? "/v1/anomalies?cursor=c3" : "/v1/anomalies?cursor=c2",
      );
    });
    await expect.poll(() => root.textContent, { timeout: 8_000 }).toContain("message a2");
    expect(first()).toHaveLength(2);
    expect(asked("c2")).toHaveLength(1);
    expect(root.textContent).not.toContain("La suite de la liste n'a pas pu être lue");
    const said = [...root.querySelectorAll("[role='status']")].map((el) => el.textContent);
    expect(said.some((text) => text?.includes("la liste repart du début"))).toBe(true);
  }, 10_000);

  it("a cursor refused again: no loop, the alert starts the pages again", async () => {
    const { root, asked, first } = await mounted((url) =>
      url.includes("cursor=c2")
        ? problem(400, "invalid-cursor")
        : page([anomaly("a1", PREFIX)], "/v1/anomalies?cursor=c2"),
    );
    await expect
      .poll(() => root.textContent, { timeout: 8_000 })
      .toContain("La suite de la liste n'a pas pu être lue");
    await new Promise((resolve) => setTimeout(resolve, 100));
    expect(first()).toHaveLength(2);
    expect(asked("c2")).toHaveLength(2);
    expect(root.textContent).toContain("message a1");
    // « Réessayer » starts from the first page, not the cursor refused.
    [...root.querySelectorAll("button")].find((b) => b.textContent?.includes("Réessayer"))!.click();
    await expect.poll(() => first().length, { timeout: 8_000 }).toBe(3);
  }, 10_000);

  it("a next page refused: the rows kept, the refusal above, never asked again", async () => {
    const { root, asked, first } = await mounted((url) =>
      url.includes("cursor=c2")
        ? problem(403, "forbidden")
        : page([anomaly("a1", PREFIX)], "/v1/anomalies?cursor=c2"),
    );
    await expect
      .poll(() => root.textContent, { timeout: 8_000 })
      .toContain("La suite de la liste n'a pas pu être lue");
    await new Promise((resolve) => setTimeout(resolve, 100));
    expect(root.textContent).toContain("message a1");
    expect(root.textContent).not.toContain("Aucune anomalie ouverte sur cet objet");
    expect(first()).toHaveLength(1);
    expect(asked("c2")).toHaveLength(1);
  }, 10_000);
});
