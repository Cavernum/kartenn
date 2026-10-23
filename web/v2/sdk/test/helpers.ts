// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { createClient } from "../src/api/client.ts";
import { createQueries, createQueryClient } from "../src/data/cache.ts";

/** A request as the fake server saw it. */
export interface Seen {
  method: string;
  url: string;
  headers: Headers;
  body: unknown;
}

export type Handler = (request: Seen) => Response | Promise<Response>;

/** A JSON answer, with its `ETag` if given. */
export function json(body: unknown, init: { status?: number; etag?: string } = {}): Response {
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (init.etag) headers["ETag"] = init.etag;
  return new Response(JSON.stringify(body), { status: init.status ?? 200, headers });
}

/** A problem (RFC 9457). */
export function problem(status: number, slug: string, extra: Record<string, unknown> = {}) {
  return new Response(
    JSON.stringify({ type: `urn:kartenn:problem:${slug}`, title: slug, status, ...extra }),
    { status, headers: { "Content-Type": "application/problem+json" } },
  );
}

/** A 304, with the `ETag` it confirms. */
export function notModified(etag: string): Response {
  return new Response(null, { status: 304, headers: { ETag: etag } });
}

/** A `fetch` answered by a handler, which records each request. */
export function fakeServer(handler: Handler) {
  const seen: Seen[] = [];
  const fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const text = init?.body as string | undefined;
    const request: Seen = {
      method: init?.method ?? "GET",
      url: String(input),
      headers: new Headers(init?.headers),
      body: text === undefined ? undefined : JSON.parse(text),
    };
    seen.push(request);
    return handler(request);
  }) as typeof globalThis.fetch;
  return { fetch, seen };
}

/** A cache over a fake server, isolated from the singletons of the SDK. */
export function fakeCache(handler: Handler) {
  const server = fakeServer(handler);
  const client = createClient({ fetch: server.fetch });
  const queryClient = createQueryClient();
  const reads = createQueries({ client, queryClient });
  return { ...server, client, queryClient, reads };
}

export const P1 = "00000000-0000-4000-8000-000000000001";
export const P2 = "00000000-0000-4000-8000-000000000002";
export const D1 = "00000000-0000-4000-8000-0000000000d1";
