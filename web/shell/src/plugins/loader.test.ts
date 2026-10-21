import { describe, expect, it, vi } from "vitest";
import { CAPABILITY_ATTEMPTS, readPlugins, startPlugins } from "./loader.ts";
import { usePlugins, usePluginsUnavailable } from "./registry.ts";

/** A `fetch` that answers each call with the next response of `answers`. */
function answering(...answers: (Response | Error)[]) {
  const calls: string[] = [];
  const fetcher = vi.fn((input: RequestInfo | URL) => {
    calls.push(String(input));
    const next = answers.shift() ?? new Error("no more answers");
    return next instanceof Error ? Promise.reject(next) : Promise.resolve(next);
  });
  return { fetcher: fetcher as unknown as typeof fetch, calls };
}

const json = (status: number, body: unknown, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json", ...headers },
  });

const noWait = () => Promise.resolve();

describe("reading the capabilities (REQ-007-9)", () => {
  it("returns the published plugins", async () => {
    const { fetcher } = answering(json(200, { ui_plugins: [] }));
    expect(await readPlugins(fetcher, noWait)).toEqual([]);
  });

  it("reads again after a refusal for load, honouring Retry-After", async () => {
    const waits: number[] = [];
    const { fetcher, calls } = answering(
      json(503, { type: "urn:kartenn:problem:overloaded" }, { "retry-after": "2" }),
      json(429, { type: "urn:kartenn:problem:rate-limited" }),
      new TypeError("network"),
      json(200, { ui_plugins: [] }),
    );
    const list = await readPlugins(fetcher, (ms) => {
      waits.push(ms);
      return Promise.resolve();
    });
    expect(list).toEqual([]);
    expect(calls).toHaveLength(4);
    expect(waits[0]).toBe(2_000);
  });

  it("gives up after its attempts: null, never a TypeError", async () => {
    const answers = Array.from({ length: CAPABILITY_ATTEMPTS }, () =>
      json(503, { type: "urn:kartenn:problem:overloaded" }),
    );
    const { fetcher, calls } = answering(...answers);
    expect(await readPlugins(fetcher, noWait)).toBeNull();
    expect(calls).toHaveLength(CAPABILITY_ATTEMPTS);
  });

  it("takes a body without a list of plugins for a failure, not for no plugin", async () => {
    const { fetcher } = answering(json(200, { modules: [] }));
    expect(await readPlugins(fetcher, noWait)).toBeNull();
  });

  it("does not read again after a refusal that is not about load", async () => {
    const { fetcher, calls } = answering(json(500, {}));
    expect(await readPlugins(fetcher, noWait)).toBeNull();
    expect(calls).toHaveLength(1);
  });

  it("says the plugins are unavailable, then clears it once read", async () => {
    vi.spyOn(console, "warn").mockImplementation(() => {});
    const failing = answering(json(500, {}));
    await startPlugins(["fr"], () => {}, failing.fetcher);
    expect(usePluginsUnavailable().value).toBe(true);

    const working = answering(json(200, { ui_plugins: [] }));
    await startPlugins(["fr"], () => {}, working.fetcher);
    expect(usePluginsUnavailable().value).toBe(false);
    expect(usePlugins().value).toEqual([]);
  });
});
