// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The client of the API (REQ-007-15, W8): a thin `fetch` layer typed by the contract,
// which knows the conventions of kartenn (`QUERY`, `If-Match`, `Idempotency-Key`,
// problems, cursors, dated reads).

import { DATED_PATHS } from "./dated.ts";
import { ApiProblem } from "./problem.ts";
import type { paths, queries } from "./schema";

type Method = "get" | "post" | "put" | "patch" | "delete";

/** The operation of a path for a method, `never` if the path does not answer it. */
type Op<P extends keyof paths, M extends Method> = NonNullable<paths[P][M]>;

/** The paths that answer a method. */
export type PathsOf<M extends Method> = {
  [P in keyof paths]: [Op<P, M>] extends [never] ? never : P;
}[keyof paths];

type Params<O> = O extends { parameters: infer P } ? P : never;
/** A group of parameters (`path`, `query`, `header`), `never` if the operation has none. */
type Part<O, K extends string> = K extends keyof Params<O> ? NonNullable<Params<O>[K]> : never;

type PathOption<O> = [Part<O, "path">] extends [never] ? object : { path: Part<O, "path"> };
type QueryOption<O> = [Part<O, "query">] extends [never] ? object : { query?: Part<O, "query"> };
/** `If-Match` is required by the type where the contract requires it (REQ-006-14). */
type IfMatchOption<O, H = Part<O, "header">> = [H] extends [never]
  ? object
  : "If-Match" extends keyof H
    ? object extends Pick<H, "If-Match" & keyof H>
      ? { ifMatch?: string }
      : { ifMatch: string }
    : object;
/** The JSON body, `application/json` or `application/merge-patch+json` (`PATCH`). */
type BodyOption<O> = O extends { requestBody?: { content: infer C } }
  ? { body: C[keyof C] }
  : object;

type JsonOf<R> = R extends { content: { "application/json": infer T } } ? T : undefined;
type Responses<O> = O extends { responses: infer R } ? R : never;
/** The JSON body of the success answers of an operation. */
export type Success<O> = JsonOf<Responses<O>[Extract<keyof Responses<O>, 200 | 201 | 202 | 204>]>;

export type GetOptions<P extends PathsOf<"get">> = PathOption<Op<P, "get">> &
  QueryOption<Op<P, "get">>;
export type PostOptions<P extends PathsOf<"post">> = PathOption<Op<P, "post">> &
  QueryOption<Op<P, "post">> &
  IfMatchOption<Op<P, "post">> &
  BodyOption<Op<P, "post">> & { idempotencyKey?: string };
export type PutOptions<P extends PathsOf<"put">> = PathOption<Op<P, "put">> &
  IfMatchOption<Op<P, "put">> &
  BodyOption<Op<P, "put">>;
export type PatchOptions<P extends PathsOf<"patch">> = PathOption<Op<P, "patch">> &
  IfMatchOption<Op<P, "patch">> &
  BodyOption<Op<P, "patch">>;
export type DeleteOptions<P extends PathsOf<"delete">> = PathOption<Op<P, "delete">> &
  QueryOption<Op<P, "delete">> &
  IfMatchOption<Op<P, "delete">>;

/**
 * An answer: its body, and the `ETag` to send back as `If-Match`, as is: the tag is opaque
 * (REQ-006-14, H19), never parsed by a client.
 */
export interface Answer<T> {
  data: T;
  etag: string | null;
  status: number;
}

/** A page of a list (REQ-006-7). */
export interface Page<T> {
  items: T[];
  links: { next?: string | null };
}

type Item<T> = T extends { items: (infer I)[] } ? I : never;

export interface ClientOptions {
  /** The `fetch` to call; the global one by default (looked up at each call). */
  fetch?: typeof fetch;
  /** Prefix of the paths, empty on the same origin. */
  baseUrl?: string;
  /** The dated context, for the reads of {@link DATED_PATHS} (REQ-007-24). */
  time?: () => { validAt: string | null; asOf: string | null };
  /** Called on a 401 before the problem is thrown: the shell offers to log in again. */
  onUnauthorized?: () => void | Promise<void>;
}

export interface Client {
  get<P extends PathsOf<"get">>(
    path: P,
    ...options: object extends GetOptions<P> ? [GetOptions<P>?] : [GetOptions<P>]
  ): Promise<Answer<Success<Op<P, "get">>>>;
  /** Every page of a list, following its `next` link to the last one. */
  pages<P extends PathsOf<"get">>(
    path: P,
    ...options: object extends GetOptions<P> ? [GetOptions<P>?] : [GetOptions<P>]
  ): AsyncIterable<Page<Item<Success<Op<P, "get">>>>>;
  /** A search by the `QUERY` method, its criteria in a JSON body (REQ-006-10). */
  query<P extends keyof queries>(
    path: P,
    body: queries[P]["body"],
  ): Promise<Answer<queries[P]["response"]>>;
  post<P extends PathsOf<"post">>(
    path: P,
    ...options: object extends PostOptions<P> ? [PostOptions<P>?] : [PostOptions<P>]
  ): Promise<Answer<Success<Op<P, "post">>>>;
  put<P extends PathsOf<"put">>(
    path: P,
    ...options: object extends PutOptions<P> ? [PutOptions<P>?] : [PutOptions<P>]
  ): Promise<Answer<Success<Op<P, "put">>>>;
  patch<P extends PathsOf<"patch">>(
    path: P,
    ...options: object extends PatchOptions<P> ? [PatchOptions<P>?] : [PatchOptions<P>]
  ): Promise<Answer<Success<Op<P, "patch">>>>;
  delete<P extends PathsOf<"delete">>(
    path: P,
    ...options: object extends DeleteOptions<P> ? [DeleteOptions<P>?] : [DeleteOptions<P>]
  ): Promise<Answer<Success<Op<P, "delete">>>>;
  /**
   * A request by URL, untyped, for the data layer (cache, writes, labels): the context
   * adds no date, `If-None-Match` goes as given and a 304 answers `status: 304` with no
   * data. Any other status outside 2xx throws its {@link ApiProblem}.
   */
  request(method: string, url: string, init?: RequestOptions): Promise<Answer<unknown>>;
}

/** What {@link Client.request} sends. */
export interface RequestOptions {
  body?: unknown;
  /** `merge` sends the body as `application/merge-patch+json` (RFC 7396). */
  merge?: boolean;
  ifMatch?: string;
  ifNoneMatch?: string | null;
  idempotencyKey?: string;
  signal?: AbortSignal;
}

interface RawOptions {
  path?: Record<string, string | number>;
  query?: Record<string, unknown>;
  body?: unknown;
  ifMatch?: string;
  ifNoneMatch?: string | null;
  idempotencyKey?: string;
  signal?: AbortSignal;
  merge?: boolean;
}

/** The path with its parameters, `{id}` → encoded value. */
function expand(template: string, params: Record<string, string | number> = {}): string {
  return template.replace(/\{([^}]+)\}/g, (_, name: string) => {
    const value = params[name];
    if (value === undefined) throw new TypeError(`missing path parameter ${name} of ${template}`);
    return encodeURIComponent(String(value));
  });
}

/** The query string of parameters, `undefined` and `null` left out, arrays repeated. */
export function search(query: Record<string, unknown>): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null) continue;
    for (const one of Array.isArray(value) ? value : [value]) params.append(key, String(one));
  }
  const text = params.toString();
  return text ? `?${text}` : "";
}

/** A client over a `fetch`; the SDK exports one, plugged into the shell (`api`). */
export function createClient(options: ClientOptions = {}): Client {
  const base = options.baseUrl ?? "";

  async function send(method: string, url: string, raw: RawOptions): Promise<Answer<unknown>> {
    const headers = new Headers({ Accept: "application/json" });
    let body: string | undefined;
    if (raw.body !== undefined) {
      headers.set(
        "Content-Type",
        method === "PATCH" || raw.merge ? "application/merge-patch+json" : "application/json",
      );
      body = JSON.stringify(raw.body);
    }
    if (raw.ifMatch !== undefined) headers.set("If-Match", raw.ifMatch);
    if (raw.ifNoneMatch) headers.set("If-None-Match", raw.ifNoneMatch);
    if (method === "POST")
      headers.set("Idempotency-Key", raw.idempotencyKey ?? crypto.randomUUID());
    const call = options.fetch ?? globalThis.fetch;
    const response = await call(base + url, {
      method,
      headers,
      credentials: "same-origin",
      ...(body === undefined ? {} : { body }),
      ...(raw.signal ? { signal: raw.signal } : {}),
    });
    if (response.status === 304) {
      return { data: undefined, etag: response.headers.get("ETag"), status: 304 };
    }
    if (!response.ok) {
      const problem = await ApiProblem.from(response);
      if (response.status === 401) await options.onUnauthorized?.();
      throw problem;
    }
    const data =
      response.status === 204 || !/json/.test(response.headers.get("Content-Type") ?? "")
        ? undefined
        : await response.json();
    return { data, etag: response.headers.get("ETag"), status: response.status };
  }

  /** The query of a read, with `valid_at` and `as_of` of the context on a dated path. */
  function readUrl(template: string, raw: RawOptions): string {
    const query: Record<string, unknown> = { ...raw.query };
    if (DATED_PATHS.has(template) && options.time) {
      const { validAt, asOf } = options.time();
      if (query["valid_at"] === undefined && validAt !== null) query["valid_at"] = validAt;
      if (query["as_of"] === undefined && asOf !== null) query["as_of"] = asOf;
    }
    return expand(template, raw.path) + search(query);
  }

  // Spec 024 (REQ-024-13): a write applies its changeset when its author holds `apply_own`,
  // and stays pending otherwise, unless it names `apply` itself.
  const write =
    (method: string) =>
    (path: string, raw: RawOptions = {}) =>
      send(
        method,
        expand(path, raw.path) + search({ ...raw.query, apply: raw.query?.["apply"] ?? "own" }),
        raw,
      );

  return {
    get: (path: string, raw: RawOptions = {}) => send("GET", readUrl(path, raw), raw),
    async *pages(path: string, raw: RawOptions = {}) {
      // The cursor carries the filters and the dates: `next` is followed as it is.
      let url: string | null | undefined = readUrl(path, raw);
      while (url) {
        const { data } = await send("GET", url, {});
        const page = data as Page<unknown>;
        yield page;
        url = page.links?.next;
      }
    },
    query: (path: string, body: unknown) => send("QUERY", path, { body }),
    post: write("POST"),
    put: write("PUT"),
    patch: write("PATCH"),
    delete: write("DELETE"),
    request: (method: string, url: string, init: RequestOptions = {}) => send(method, url, init),
  } as unknown as Client;
}
