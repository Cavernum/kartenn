import { describe, expect, it } from "vitest";
import { createMemoryHistory } from "vue-router";
import { createShellRouter } from "./router.ts";

describe("router", () => {
  it("serves the home page at the root", async () => {
    const router = createShellRouter(createMemoryHistory());
    await router.push("/");
    expect(router.currentRoute.value.name).toBe("home");
  });

  it("opens the record of an object of the core, not of another collection", async () => {
    const router = createShellRouter(createMemoryHistory());
    await router.push("/prefixes/0193a3f2-0000-7000-8000-000000000001");
    expect(router.currentRoute.value.name).toBe("object");
    await router.push("/widgets/1");
    expect(router.currentRoute.value.name).toBe("not-found");
  });

  it("gives the views of plugins their own paths, below which they route", async () => {
    const router = createShellRouter(createMemoryHistory());
    await router.push("/x/notes/overview/a/b");
    expect(router.currentRoute.value.name).toBe("plugin-view");
    expect(router.currentRoute.value.params).toMatchObject({ plugin: "notes", id: "overview" });
  });
});
