import { describe, expect, it } from "vitest";
import { createMemoryHistory } from "vue-router";
import { createShellRouter } from "./router.ts";

describe("router", () => {
  it("serves the home page at the root", async () => {
    const router = createShellRouter(createMemoryHistory());
    await router.push("/");
    expect(router.currentRoute.value.name).toBe("home");
  });
});
