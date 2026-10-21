import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createMemoryHistory } from "vue-router";
import { NO_RIGHTS, Rights } from "../rights/permissions.ts";
import type { Session } from "../rights/index.ts";
import { createShellRouter } from "../router.ts";
import {
  clearSensitive,
  installSession,
  internalPath,
  ISSUER_KEY,
  loginAt,
  RETURN_KEY,
  rememberReturn,
  takeReturn,
} from "./index.ts";

/** A `Storage` over a map, the test environment having none. */
class MemoryStorage implements Storage {
  private readonly items = new Map<string, string>();
  get length(): number {
    return this.items.size;
  }
  clear(): void {
    this.items.clear();
  }
  getItem(key: string): string | null {
    return this.items.get(key) ?? null;
  }
  key(index: number): string | null {
    return [...this.items.keys()][index] ?? null;
  }
  removeItem(key: string): void {
    this.items.delete(key);
  }
  setItem(key: string, value: string): void {
    this.items.set(key, value);
  }
}

beforeEach(() => {
  vi.stubGlobal("localStorage", new MemoryStorage());
  vi.stubGlobal("sessionStorage", new MemoryStorage());
});
afterEach(() => vi.unstubAllGlobals());

const SIGNED_IN: Session = {
  me: {
    identity: "0193a3f2-0000-7000-8000-000000000009",
    kind: "person",
    name: "Alice",
    groups: [],
    roles: [],
    bindings: [],
    global_permissions: [],
  },
  rights: new Rights([]),
};
const SIGNED_OUT: Session = { me: null, rights: NO_RIGHTS };

describe("the URL to come back to (REQ-007-28)", () => {
  it("stays on the origin and off the login page", () => {
    expect(internalPath("/prefixes/1?valid_at=2026-01-01T00:00:00Z")).toBe(
      "/prefixes/1?valid_at=2026-01-01T00:00:00Z",
    );
    for (const outside of ["//evil.example/", "/\\evil.example", "https://evil.example/", "x"]) {
      expect(internalPath(outside)).toBeNull();
    }
    expect(internalPath("/login?return=/")).toBeNull();
    expect(internalPath("/v1/auth/logout")).toBeNull();
    expect(internalPath(["/a"])).toBeNull();
  });

  it("is read once", () => {
    rememberReturn("/sites/1");
    expect(takeReturn()).toBe("/sites/1");
    expect(takeReturn()).toBeNull();
    rememberReturn("//evil.example/");
    expect(sessionStorage.getItem(RETURN_KEY)).toBeNull();
  });

  it("goes with the issuer, which is remembered", () => {
    const go = vi.fn();
    loginAt("corp idp", "/addresses/2", go);
    expect(go).toHaveBeenCalledWith("/v1/auth/login?issuer=corp%20idp");
    expect(localStorage.getItem(ISSUER_KEY)).toBe("corp idp");
    expect(takeReturn()).toBe("/addresses/2");
  });
});

describe("the logout (REQ-007-28)", () => {
  it("clears what is sensitive, keeps language, theme and issuer", async () => {
    localStorage.setItem("kartenn.locale", "fr");
    localStorage.setItem("kartenn.theme", "dark");
    localStorage.setItem(ISSUER_KEY, "dev");
    localStorage.setItem("kartenn.recent.searches", '["10.0.0.1"]');
    localStorage.setItem("other.app", "kept");
    sessionStorage.setItem(RETURN_KEY, "/sites/1");
    await clearSensitive();
    expect(localStorage.getItem("kartenn.locale")).toBe("fr");
    expect(localStorage.getItem("kartenn.theme")).toBe("dark");
    expect(localStorage.getItem(ISSUER_KEY)).toBe("dev");
    expect(localStorage.getItem("kartenn.recent.searches")).toBeNull();
    expect(localStorage.getItem("other.app")).toBe("kept");
    expect(sessionStorage.getItem(RETURN_KEY)).toBeNull();
  });
});

describe("the guard of the session", () => {
  it("sends to the login page without a session, the page kept for after", async () => {
    const router = createShellRouter(createMemoryHistory());
    installSession(
      router,
      async () => undefined,
      () => SIGNED_OUT,
    );
    await router.push("/prefixes/1?valid_at=2026-01-01T00:00:00Z");
    expect(router.currentRoute.value.name).toBe("login");
    expect(router.currentRoute.value.query.return).toBe(
      "/prefixes/1?valid_at=2026-01-01T00:00:00Z",
    );
    await router.push("/about");
    expect(router.currentRoute.value.name).toBe("about");
  });

  it("goes back to the page kept after a login, once", async () => {
    rememberReturn("/prefixes/1?valid_at=2026-01-01T00:00:00Z");
    const router = createShellRouter(createMemoryHistory());
    const load = vi.fn(async () => undefined);
    installSession(router, load, () => SIGNED_IN);
    await router.push("/");
    expect(router.currentRoute.value.fullPath).toBe("/prefixes/1?valid_at=2026-01-01T00:00:00Z");
    // Taken once: the next page stays itself (the dates carried by the time guard).
    await router.push("/");
    expect(router.currentRoute.value.path).toBe("/");
    expect(load).toHaveBeenCalledTimes(1);
  });
});
