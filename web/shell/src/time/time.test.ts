import { context } from "@kartenn/sdk";
import { resetContext } from "@kartenn/sdk/host";
import { beforeEach, describe, expect, it } from "vitest";
import { createMemoryHistory } from "vue-router";
import { createShellRouter } from "../router.ts";
import { datedSearch, timeOf, toPresent, writesOf } from "./index.ts";

const VALID_AT = "2026-03-01T00:00:00Z";
const AS_OF = "2026-04-15T12:30:00.5Z";

beforeEach(() => resetContext());

describe("the dated context of the URL (REQ-007-22)", () => {
  it("reads both dates, the present without them", () => {
    expect(timeOf({ valid_at: VALID_AT, as_of: AS_OF })).toEqual({
      validAt: VALID_AT,
      asOf: AS_OF,
      present: false,
    });
    expect(timeOf({})).toEqual({ validAt: null, asOf: null, present: true });
  });

  it("takes a malformed or non-UTC date for absent", () => {
    for (const bad of [
      "yesterday",
      "2026-03-01",
      "2026-03-01T00:00:00+01:00",
      "2026-13-01T00:00:00Z",
    ]) {
      expect(timeOf({ valid_at: bad }).validAt, bad).toBeNull();
    }
  });

  it("names the dates for a read of the API, nothing in the present", () => {
    expect(datedSearch(timeOf({ valid_at: VALID_AT, as_of: AS_OF }))).toBe(
      `?valid_at=${encodeURIComponent(VALID_AT)}&as_of=${encodeURIComponent(AS_OF)}`,
    );
    expect(datedSearch(timeOf({}))).toBe("");
  });
});

describe("the guard of the router", () => {
  it("carries the dates along an internal navigation", async () => {
    const router = createShellRouter(createMemoryHistory());
    await router.push({ path: "/", query: { valid_at: VALID_AT, as_of: AS_OF } });
    await router.push("/about");
    expect(router.currentRoute.value.path).toBe("/about");
    expect(router.currentRoute.value.query).toEqual({ valid_at: VALID_AT, as_of: AS_OF });
  });

  it("lets a link name its own dates", async () => {
    const router = createShellRouter(createMemoryHistory());
    await router.push({ path: "/", query: { valid_at: VALID_AT, as_of: AS_OF } });
    await router.push({ path: "/about", query: { as_of: AS_OF } });
    expect(router.currentRoute.value.query).toEqual({ as_of: AS_OF });
  });

  it("returns to the present only when asked", async () => {
    const router = createShellRouter(createMemoryHistory());
    await router.push({ path: "/about", query: { valid_at: VALID_AT } });
    await toPresent(router);
    expect(router.currentRoute.value.fullPath).toBe("/about");
    await router.push("/");
    expect(router.currentRoute.value.fullPath).toBe("/");
  });

  it("pushes the context to the SDK after each navigation (REQ-007-24)", async () => {
    const router = createShellRouter(createMemoryHistory());
    await router.push({ path: "/", query: { valid_at: VALID_AT } });
    expect(context.get().time).toEqual({ validAt: VALID_AT, asOf: null, present: false });
    await toPresent(router);
    expect(context.get().time.present).toBe(true);
  });
});

describe("writes off the present (REQ-007-23)", () => {
  it("are all off under as_of, the draft only under valid_at", () => {
    expect(writesOf(timeOf({}))).toEqual({ quickEdit: true, draft: true });
    expect(writesOf(timeOf({ valid_at: VALID_AT }))).toEqual({ quickEdit: false, draft: true });
    expect(writesOf(timeOf({ as_of: AS_OF }))).toEqual({ quickEdit: false, draft: false });
  });
});
