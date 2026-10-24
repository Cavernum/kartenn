// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, describe, expect, it, vi } from "vitest";
import * as host from "../src/host.ts";
import * as sdk from "../src/index.ts";
import { announcePlugin, fakeFetch, mount } from "../src/testing/index.ts";

afterEach(() => host.resetContext());

/** A plain Custom Element, without a framework, that reads the SDK. */
class Notes extends HTMLElement {
  object: sdk.ObjectRef | null = null;
  async connectedCallback() {
    const id = this.object?.id ?? "";
    const { data } = await sdk.api.get("/v1/prefixes/{id}", { path: { id } });
    this.textContent = `${(data as { value: string }).value}`;
  }
}

const extension = (element: string) => ({
  point: "tab" as const,
  id: element,
  element,
  object_types: ["prefix"],
});

describe("context (REQ-007-14)", () => {
  it("notifies its listeners and tells the present from a dated view", () => {
    const seen: (string | null)[] = [];
    const stop = sdk.context.subscribe((c) => seen.push(c.time.validAt));
    host.setContext({ time: { validAt: "2026-03-01T00:00:00Z", asOf: null } });
    expect(sdk.context.get().time.present).toBe(false);
    stop();
    host.setContext({ time: { validAt: null, asOf: null } });
    expect(seen).toEqual(["2026-03-01T00:00:00Z"]);
    expect(sdk.context.get().time.present).toBe(true);
  });

  it("is the same object for every importer of the SDK", async () => {
    const again = await import("../src/index.ts");
    const fromTesting = await import("../src/context.ts");
    host.setContext({ locale: "fr" });
    expect(again.context).toBe(sdk.context);
    expect(fromTesting.context.get()).toBe(sdk.context.get());
    expect(again.context.get().locale).toBe("fr");
  });

  it("gives the user and their permissions as the shell sums them up", () => {
    host.setContext({
      user: { identity: "u1", name: "Ada", kind: "person", has: (p) => p === "core.prefix.read" },
    });
    expect(sdk.context.get().user.has("core.prefix.read")).toBe(true);
    expect(sdk.context.get().user.has("core.address.edit")).toBe(false);
  });
});

describe("definePlugin (REQ-007-13)", () => {
  it("defines the declared elements and resolves the shell's wait", async () => {
    const loaded = announcePlugin({ name: "p-ok", extensions: [extension("p-ok-notes")] });
    sdk.definePlugin({ name: "p-ok", elements: { "p-ok-notes": class extends HTMLElement {} } });
    await expect(loaded).resolves.toHaveProperty("t");
    expect(customElements.get("p-ok-notes")).toBeDefined();
  });

  it("refuses an element its manifest does not declare, and defines none", () => {
    void announcePlugin({ name: "p-bad", extensions: [extension("p-bad-notes")] });
    expect(() =>
      sdk.definePlugin({
        name: "p-bad",
        elements: {
          "p-bad-notes": class extends HTMLElement {},
          "p-bad-other": class extends HTMLElement {},
        },
      }),
    ).toThrow(/not declared/);
    expect(customElements.get("p-bad-notes")).toBeUndefined();
  });

  it("refuses an element already defined", () => {
    customElements.define("p-dup-notes", class extends HTMLElement {});
    void announcePlugin({ name: "p-dup", extensions: [extension("p-dup-notes")] });
    expect(() =>
      sdk.definePlugin({
        name: "p-dup",
        elements: { "p-dup-notes": class extends HTMLElement {} },
      }),
    ).toThrow(/already defined/);
  });

  it("refuses an unannounced plugin, and ignores one the shell gave up on", () => {
    expect(() => sdk.definePlugin({ name: "p-ghost", elements: {} })).toThrow(/not announced/);
    void announcePlugin({ name: "p-late", extensions: [extension("p-late-notes")] });
    host.abandonPlugin("p-late");
    sdk.definePlugin({
      name: "p-late",
      elements: { "p-late-notes": class extends HTMLElement {} },
    });
    expect(customElements.get("p-late-notes")).toBeUndefined();
  });

  it("translates under the namespace of the plugin", async () => {
    const loaded = announcePlugin({
      name: "p-i18n",
      extensions: [],
      catalogs: {
        en: { tab: { notes: "Notes of {who}" } },
        fr: { tab: { notes: "Notes de {who}" } },
      },
    });
    const handle = sdk.definePlugin({ name: "p-i18n", elements: {} });
    await loaded;
    expect(handle.t("tab.notes", { who: "Ada" })).toBe("Notes of Ada");
    host.setContext({ locale: "fr" });
    expect(handle.t("tab.notes", { who: "Ada" })).toBe("Notes de Ada");
    const shell = vi.fn((key: string) => `[${key}]`);
    host.setTranslator(shell);
    expect(handle.t("tab.notes")).toBe("[p-i18n.tab.notes]");
    host.setTranslator(null);
  });
});

describe("sandbox (REQ-007-16)", () => {
  it("mounts an element with a context and a simulated fetch typed by the contract", async () => {
    void announcePlugin({ name: "p-box", extensions: [extension("p-box-notes")] });
    sdk.definePlugin({ name: "p-box", elements: { "p-box-notes": Notes } });
    const requests: URLSearchParams[] = [];
    const { element, unmount } = mount<Notes>("p-box-notes", {
      context: { time: { validAt: "2026-03-01T00:00:00Z", asOf: null } },
      object: { type: "prefix", id: "p1", href: "/prefixes/p1" },
      fetch: fakeFetch({
        "GET /v1/prefixes/{id}": ({ params, query }) => {
          requests.push(query);
          return { value: `10.0.0.0/8 (${params["id"]})` } as never;
        },
      }),
    });
    await vi.waitFor(() => expect(element.textContent).toBe("10.0.0.0/8 (p1)"));
    expect(requests[0]?.get("valid_at")).toBe("2026-03-01T00:00:00Z");
    unmount();
    expect(element.isConnected).toBe(false);
  });

  it("answers 404 as a problem outside the handlers", async () => {
    const fetch = fakeFetch({});
    const answer = await fetch("/v1/me");
    expect(answer.status).toBe(404);
  });
});

describe("draft", () => {
  it("adds to the active draft of the tab, created when there is none, and tells the shell", async () => {
    const D = "00000000-0000-4000-8000-0000000000a1";
    const body = (count: number, version: number) => ({
      id: D,
      href: `/v1/drafts/${D}`,
      author: D,
      title: "t",
      operation_count: count,
      version,
      created_at: "2026-10-24T00:00:00Z",
      updated_at: "2026-10-24T00:00:00Z",
      reason: {},
      operations: Array.from({ length: count }, () => ({ op: "close" })),
    });
    const reply = (count: number, version: number, status = 200) =>
      new Response(JSON.stringify(body(count, version)), {
        status,
        headers: { "Content-Type": "application/json", ETag: `"${version}"` },
      });
    const seen: { method: string; ifMatch: string | null }[] = [];
    host.setFetch(
      fakeFetch({
        "POST /v1/drafts": () => reply(1, 1, 201),
        "POST /v1/drafts/{id}/operations": ({ headers }) => {
          seen.push({ method: "add", ifMatch: headers.get("If-Match") });
          return reply(2, 2);
        },
        "GET /v1/drafts": () => ({ items: [] }),
      }),
    );
    sessionStorage.clear();
    const opened = vi.fn();
    host.draftHost.onAdd(opened);
    try {
      expect(sdk.draft.active()).toBeNull();
      const first = await sdk.draft.add({ op: "close" } as never);
      expect(first.kind).toBe("saved");
      expect(sdk.draft.active()).toBe(D);
      const second = await sdk.draft.add({ op: "close" } as never);
      expect(second).toMatchObject({ kind: "saved", draft: { operation_count: 2 } });
      expect(seen).toEqual([{ method: "add", ifMatch: '"1"' }]);
      expect(opened).toHaveBeenCalledTimes(2);
    } finally {
      host.draftHost.onAdd(null);
      host.setFetch(null);
      sessionStorage.clear();
    }
  });
});
