// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { DATED, SCHEMA, generate, readContract } from "../scripts/gen.mjs";
import { createClient } from "../src/api/client.ts";
import { ApiProblem } from "../src/api/problem.ts";

interface Call {
  url: string;
  method: string;
  headers: Headers;
  body: string | undefined;
}

/** A `fetch` that records its calls and answers from a list, in order. */
function recording(...answers: Response[]) {
  const calls: Call[] = [];
  const fetch = vi.fn(async (url: RequestInfo | URL, init?: RequestInit) => {
    calls.push({
      url: String(url),
      method: init?.method ?? "GET",
      headers: new Headers(init?.headers),
      body: init?.body as string | undefined,
    });
    const answer = answers.shift();
    if (!answer) throw new Error(`unexpected call ${String(url)}`);
    return answer;
  });
  return { fetch: fetch as unknown as typeof globalThis.fetch, calls };
}

const json = (body: unknown, init: ResponseInit = {}) =>
  new Response(JSON.stringify(body), {
    ...init,
    headers: { "Content-Type": "application/json", ...init.headers },
  });

const problem = (status: number, type: string, requestId: string) =>
  new Response(JSON.stringify({ type, title: "t", status, detail: "d" }), {
    status,
    headers: { "Content-Type": "application/problem+json", "Request-Id": requestId },
  });

describe("client of the API (REQ-007-15, 24)", () => {
  it("searches with the QUERY method and a JSON body", async () => {
    const { fetch, calls } = recording(json({ items: [], links: { next: null } }));
    await createClient({ fetch }).query("/v1/addresses", { filter: { kind: "eq" } } as never);
    expect(calls[0]?.method).toBe("QUERY");
    expect(calls[0]?.url).toBe("/v1/addresses");
    expect(calls[0]?.headers.get("Content-Type")).toBe("application/json");
    expect(JSON.parse(calls[0]?.body ?? "")).toEqual({ filter: { kind: "eq" } });
  });

  it("sends If-Match, and the ETag of a read comes back", async () => {
    const { fetch, calls } = recording(
      json({ id: "a" }, { headers: { ETag: '"v1"' } }),
      json({ id: "a" }, { headers: { ETag: '"v2"' } }),
    );
    const api = createClient({ fetch });
    const read = await api.get("/v1/addresses/{id}", { path: { id: "a" } });
    expect(read.etag).toBe('"v1"');
    await api.patch("/v1/addresses/{id}", {
      path: { id: "a" },
      ifMatch: read.etag ?? "",
      body: { reason: "r" } as never,
    });
    expect(calls[1]?.method).toBe("PATCH");
    expect(calls[1]?.headers.get("If-Match")).toBe('"v1"');
    expect(calls[1]?.headers.get("Content-Type")).toBe("application/merge-patch+json");
  });

  it("requires If-Match by its type where the contract requires it", () => {
    const api = createClient({ fetch: recording().fetch });
    // Type tests only: never called.
    const typed = () => {
      // @ts-expect-error: `ifMatch` is required on a PATCH of an object.
      void api.patch("/v1/addresses/{id}", { path: { id: "a" }, body: {} as never });
      // @ts-expect-error: and on its DELETE.
      void api.delete("/v1/addresses/{id}", { path: { id: "a" } });
      // @ts-expect-error: the path parameters are required.
      void api.get("/v1/addresses/{id}");
      // @ts-expect-error: an unknown path.
      void api.get("/v1/nowhere");
    };
    expect(typed).toBeTypeOf("function");
  });

  it("generates an idempotency key, or reuses the given one", async () => {
    const { fetch, calls } = recording(json({}, { status: 202 }), json({}, { status: 202 }));
    const api = createClient({ fetch });
    await api.post("/v1/changesets", { body: {} as never });
    await api.post("/v1/changesets", { body: {} as never, idempotencyKey: "k-1" });
    expect(calls[0]?.headers.get("Idempotency-Key")).toMatch(/^[0-9a-f-]{36}$/);
    expect(calls[1]?.headers.get("Idempotency-Key")).toBe("k-1");
  });

  it("throws typed problems carrying their type and Request-Id (412, 422)", async () => {
    const { fetch } = recording(
      problem(412, "urn:kartenn:problem:precondition-failed", "r-412"),
      new Response(
        JSON.stringify({
          type: "urn:kartenn:problem:validation",
          title: "Invalid",
          status: 422,
          violations: [{ rule: "core.x", severity: "error", detail: "no", pointer: "/a" }],
        }),
        {
          status: 422,
          headers: { "Content-Type": "application/problem+json", "Request-Id": "r-422" },
        },
      ),
    );
    const api = createClient({ fetch });
    const stale = api.delete("/v1/addresses/{id}", { path: { id: "a" }, ifMatch: '"v0"' });
    await expect(stale).rejects.toMatchObject({
      status: 412,
      type: "urn:kartenn:problem:precondition-failed",
      requestId: "r-412",
    });
    const invalid = await api
      .post("/v1/changesets", { body: {} as never })
      .catch((e: unknown) => e);
    expect(invalid).toBeInstanceOf(ApiProblem);
    expect((invalid as ApiProblem).requestId).toBe("r-422");
    expect((invalid as ApiProblem).violations[0]?.rule).toBe("core.x");
  });

  it("asks the shell to log in again on a 401, then throws", async () => {
    const onUnauthorized = vi.fn();
    const { fetch } = recording(problem(401, "urn:kartenn:problem:unauthenticated", "r"));
    await expect(createClient({ fetch, onUnauthorized }).get("/v1/me")).rejects.toMatchObject({
      status: 401,
    });
    expect(onUnauthorized).toHaveBeenCalledOnce();
  });

  it("follows the pages to the last one", async () => {
    const { fetch, calls } = recording(
      json({ items: [{ id: "1" }], links: { next: "/v1/domains?cursor=c2" } }),
      json({ items: [{ id: "2" }], links: { next: "/v1/domains?cursor=c3" } }),
      json({ items: [{ id: "3" }], links: { next: null } }),
    );
    const ids: string[] = [];
    for await (const page of createClient({ fetch }).pages("/v1/domains")) {
      ids.push(...page.items.map((d) => d.id));
    }
    expect(ids).toEqual(["1", "2", "3"]);
    expect(calls.map((c) => c.url)).toEqual([
      "/v1/domains",
      "/v1/domains?cursor=c2",
      "/v1/domains?cursor=c3",
    ]);
  });

  it("adds valid_at and as_of of the context on a dated path only", async () => {
    const { fetch, calls } = recording(json({}), json({}), json({}));
    const time = () => ({ validAt: "2026-03-01T00:00:00Z", asOf: null });
    const api = createClient({ fetch, time });
    await api.get("/v1/prefixes/{id}", { path: { id: "p/1" } });
    await api.get("/v1/prefixes/{id}", {
      path: { id: "p" },
      query: { valid_at: "2025-01-01T00:00:00Z" },
    });
    await api.get("/v1/me");
    expect(calls[0]?.url).toBe("/v1/prefixes/p%2F1?valid_at=2026-03-01T00%3A00%3A00Z");
    expect(calls[1]?.url).toBe("/v1/prefixes/p?valid_at=2025-01-01T00%3A00%3A00Z");
    expect(calls[2]?.url).toBe("/v1/me");
  });
});

// Generating the types of the whole contract takes seconds, more with each module mounted
// and on a loaded machine: well beyond the default 5 s.
describe("types drawn from the contract (REQ-007-15)", { timeout: 60_000 }, () => {
  it("the versioned files are those the contract generates", async () => {
    const fresh = await generate(readContract());
    expect(fresh.schema).toBe(readFileSync(SCHEMA, "utf8"));
    expect(fresh.dated).toBe(readFileSync(DATED, "utf8"));
  });

  it("a contract changed without regeneration fails the comparison", async () => {
    const changed = readContract();
    (changed.paths as Record<string, unknown>)["/v1/vlans"] = {
      get: { operationId: "listVlans", responses: { "200": { description: "ok" } } },
    };
    const fresh = await generate(changed);
    expect(fresh.schema).not.toBe(readFileSync(SCHEMA, "utf8"));
  });
});
