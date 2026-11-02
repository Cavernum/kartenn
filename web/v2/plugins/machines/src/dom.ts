// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The pages of the plugin and its links in the page of the shell (design 021 « Liens d'un
// plugin »): the handle given back by `definePlugin`, the paths of its records, the dates of
// the context carried by a link, and a link followed as the shell would follow a click.

import { context, type PluginHandle } from "@kartenn/sdk";

/** What `definePlugin` gives back, once the elements are defined (at the end of index.ts). */
export const defined: { handle?: PluginHandle } = {};

/** A text of the plugin, under its namespace; the key itself before the definition. */
export function t(key: string, params?: Record<string, unknown>): string {
  return defined.handle ? defined.handle.t(key, params) : key;
}

/** The dates of the context in a URL of the shell, so a link opened apart keeps them (W9). */
export function hrefOf(path: string, query: Record<string, string> = {}): string {
  const { validAt, asOf } = context.get().time;
  const search = new URLSearchParams(query);
  if (validAt) search.set("valid_at", validAt);
  if (asOf) search.set("as_of", asOf);
  const text = search.toString();
  return text ? `${path}?${text}` : path;
}

/**
 * Whether the page about to be drawn was reached by a link of the plugin: its element then
 * takes the focus on its heading, the link clicked being gone with the former page.
 */
export const arrival = { byLink: false };

/**
 * Goes to `href` as a link of the plugin would: a click on a link in the element, which the
 * shell follows with its router (design 021 « API des plugins », the SDK has no navigation
 * of its own). Without the shell, the browser follows it.
 */
export function followLink(place: Node, href: string): void {
  const anchor = document.createElement("a");
  anchor.href = href;
  anchor.hidden = true;
  place.appendChild(anchor);
  anchor.click();
  anchor.remove();
}

/** The pages of the plugin, under `/x/machines/<view>`. */
export const PAGES = {
  machine: (id: string) => `/x/machines/machines/${encodeURIComponent(id)}`,
  cluster: (id: string) => `/x/machines/clusters/${encodeURIComponent(id)}`,
  interface: (id: string) => `/x/machines/interfaces/${encodeURIComponent(id)}`,
};

/** The identifier after `/x/machines/<view>/` in the address of the page, if any. */
export function pageId(view: string): string | null {
  const prefix = `/x/machines/${view}/`;
  const path = location.pathname;
  if (!path.startsWith(prefix)) return null;
  const rest = path.slice(prefix.length).split("/")[0] ?? "";
  return rest ? decodeURIComponent(rest) : null;
}
