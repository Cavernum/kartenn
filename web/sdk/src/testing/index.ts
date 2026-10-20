// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/sdk/testing`: the sandbox of plugin tests (REQ-007-16). It mounts the element
 * of an extension in the test document with a chosen context and a `fetch` simulated by
 * handlers typed by the contract. Needs a DOM (Vitest with `happy-dom` or `jsdom`).
 */

import type { PathsOf } from "../api/client.ts";
import type { paths } from "../api/schema";
import { setContext, type ContextUpdate, type ObjectRef } from "../context.ts";
import { expectPlugin, type Extension, type PluginHandle } from "../extensions.ts";
import { hooks } from "../hooks.ts";
import { addCatalog, type Messages } from "../i18n.ts";

type Method = "get" | "post" | "put" | "patch" | "delete";

/** `"GET /v1/prefixes/{id}"` and the like: the routes of the contract. */
export type Route = { [M in Method]: `${Uppercase<M>} ${PathsOf<M> & string}` }[Method];

type OpOf<R extends Route> = R extends `${infer M} ${infer P}`
  ? P extends keyof paths
    ? Lowercase<M> extends keyof paths[P]
      ? NonNullable<paths[P][Lowercase<M>]>
      : never
    : never
  : never;
type JsonOf<X> = X extends { content: { "application/json": infer T } } ? T : unknown;
type Reply<R extends Route> =
  OpOf<R> extends { responses: infer S } ? JsonOf<S[Extract<keyof S, 200 | 201 | 202>]> : unknown;

/** What a handler receives: the path parameters, the query and the JSON body. */
export interface FakeRequest {
  params: Record<string, string>;
  query: URLSearchParams;
  body: unknown;
  headers: Headers;
}

/** A handler answers the typed body (status 200), or a whole `Response`. */
export type Handlers = {
  [R in Route]?: (request: FakeRequest) => Reply<R> | Response | Promise<Reply<R> | Response>;
};

function matcher(template: string): RegExp {
  const pattern = template
    .split(/(\{[^}]+\})/)
    .map((part) =>
      part.startsWith("{")
        ? `(?<${part.slice(1, -1).replace(/\W/g, "_")}>[^/]+)`
        : part.replace(/[.*+?^$()|[\]\\]/g, "\\$&"),
    )
    .join("");
  return new RegExp(`^${pattern}$`);
}

/** A `fetch` that answers from the handlers, 404 for any other route. */
export function fakeFetch(handlers: Handlers): typeof fetch {
  const routes = Object.entries(handlers).map(([route, handler]) => {
    const [method, template] = route.split(" ") as [string, string];
    return { method, regex: matcher(template), handler: handler as (r: FakeRequest) => unknown };
  });
  return async (input, init) => {
    const url = new URL(String(input instanceof Request ? input.url : input), "http://sandbox");
    const method = (init?.method ?? "GET").toUpperCase();
    for (const route of routes) {
      const match = route.method === method ? route.regex.exec(url.pathname) : null;
      if (!match) continue;
      const params: Record<string, string> = {};
      for (const [k, v] of Object.entries(match.groups ?? {})) params[k] = decodeURIComponent(v);
      const text = typeof init?.body === "string" ? init.body : null;
      const answer = await route.handler({
        params,
        query: url.searchParams,
        body: text === null ? undefined : JSON.parse(text),
        headers: new Headers(init?.headers),
      });
      if (answer instanceof Response) return answer;
      return new Response(JSON.stringify(answer ?? null), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    }
    return new Response(JSON.stringify({ type: "about:blank", title: "Not Found", status: 404 }), {
      status: 404,
      headers: { "Content-Type": "application/problem+json" },
    });
  };
}

export interface MountOptions {
  /** The context of the test; members left out keep their value. */
  context?: ContextUpdate;
  /** The `object` property of the element (tab, panel, column, action). */
  object?: ObjectRef;
  /** Other properties of the element (`row` of a column…). */
  properties?: Record<string, unknown>;
  /** The `fetch` of the SDK client during the test. */
  fetch?: typeof fetch;
}

export interface Mounted<E extends HTMLElement> {
  element: E;
  /** Removes the element and gives back the global `fetch`. */
  unmount(): void;
}

/** Mounts a defined element in the test document, after setting the context. */
export function mount<E extends HTMLElement = HTMLElement>(
  tag: string,
  options: MountOptions = {},
): Mounted<E> {
  if (!customElements.get(tag)) throw new Error(`${tag} is not defined`);
  if (options.context) setContext(options.context);
  hooks.fetch = options.fetch ?? null;
  const element = document.createElement(tag) as E;
  const props: Record<string, unknown> = { ...options.properties };
  if (options.object) props["object"] = options.object;
  Object.assign(element, props);
  document.body.append(element);
  return {
    element,
    unmount() {
      element.remove();
      hooks.fetch = null;
    },
  };
}

/**
 * Announces a plugin as the shell would (manifest and catalogs), so that its module can
 * call `definePlugin`; resolves with the handle once it does.
 */
export function announcePlugin(plugin: {
  name: string;
  extensions: Extension[];
  catalogs?: Record<string, Messages>;
  integrity?: Record<string, string>;
}): Promise<PluginHandle> {
  for (const [locale, messages] of Object.entries(plugin.catalogs ?? {})) {
    addCatalog(locale, plugin.name, messages);
  }
  return expectPlugin({
    name: plugin.name,
    base: `/plugins/${plugin.name}/0.0.0/`,
    integrity: plugin.integrity ?? {},
    extensions: plugin.extensions,
  });
}
