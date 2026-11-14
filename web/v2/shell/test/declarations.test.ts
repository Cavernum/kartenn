// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, effectScope, ref } from "vue";
import { installData } from "../src/data/query.ts";
import { useDeclarationsOf } from "../src/views/data.ts";

// The declarations of some keys, each read by its key as the prefix `q` (task 021-11p7):
// the declaration of that very key kept, a key not declared without one, a refusal said.

afterEach(() => {
  setFetch(null);
  resetContext();
  queryClient.clear();
});

/** Runs a composable as a component of an application with the adapter would. */
function inApp<T>(run: () => T): T {
  const app = createApp({});
  installData(app);
  return app.runWithContext(() => effectScope().run(run))!;
}

/** The API answering each `q` with `items(q)`, or refusing with `status`. */
function answering(items: (q: string) => object[], status = 200) {
  const asked: string[] = [];
  setFetch((async (input: RequestInfo | URL) => {
    const url = new URL(String(input), "https://k");
    asked.push(url.searchParams.get("q") ?? "");
    const body =
      status === 200
        ? { items: items(url.searchParams.get("q") ?? ""), links: {} }
        : { type: "urn:kartenn:problem:forbidden", title: "", status };
    return new Response(JSON.stringify(body), {
      status,
      headers: { "Content-Type": status === 200 ? "application/json" : "application/problem+json" },
    });
  }) as typeof fetch);
  return asked;
}

const declaration = (key: string) => ({ id: `d-${key}`, key, type: "text", carriers: ["prefix"] });

describe("the declarations of some keys", () => {
  it("keeps the declaration of the very key, never another its prefix finds", async () => {
    // `q` is a prefix of the key or of a label: `core.site` finds `core.site_code` too.
    const asked = answering((q) =>
      q === "core.site" ? [declaration("core.site_code"), declaration("core.site")] : [],
    );
    const keys = ref(["core.site", "core.site"]);
    const found = inApp(() => useDeclarationsOf(() => keys.value));
    await vi.waitFor(() => expect(found.of("core.site")).toMatchObject({ id: "d-core.site" }));
    expect(found.of("core.site_code")).toBeUndefined();
    // One read for a key named twice.
    expect(asked).toEqual(["core.site"]);
    expect(found.failed.value).toBe(false);
  });

  it("has none for a key no declaration bears, and says nothing failed", async () => {
    const asked = answering(() => [declaration("custom.other")]);
    const found = inApp(() => useDeclarationsOf(() => ["custom.absent"]));
    await vi.waitFor(() => expect(asked).toEqual(["custom.absent"]));
    await vi.waitFor(() =>
      expect(queryClient.getQueryState(["shell", "declaration", "custom.absent"])?.status).toBe(
        "success",
      ),
    );
    expect(found.of("custom.absent")).toBeUndefined();
    expect(found.of(null)).toBeUndefined();
    expect(found.failed.value).toBe(false);
  });

  it("says a refused read: the key without its declaration", async () => {
    answering(() => [], 403);
    const found = inApp(() => useDeclarationsOf(() => ["core.vlan"]));
    await vi.waitFor(() => expect(found.failed.value).toBe(true), { timeout: 8_000 });
    expect(found.of("core.vlan")).toBeUndefined();
  }, 10_000);
});
