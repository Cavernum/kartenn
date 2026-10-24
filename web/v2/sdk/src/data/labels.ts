// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The labels of keys (spec 018, REQ-021-19; design « Filtres, libellés, codes »): objects
// carry keys only; `GET /v1/labels` serves the dictionary of a language, a query of the
// cache never stale until the event `labels` of the stream names another revision. A label
// is looked up in the dictionary, then in the catalogs of the shell and the plugins
// (`labels.<kind>.<key>`), else the key itself is shown. A label changing reads no object.

import type { QueryClient } from "@tanstack/query-core";
import { translate } from "../i18n.ts";
import type { Entry, Queries } from "./cache.ts";

/** A dictionary of labels, as `GET /v1/labels` serves it (018, « Dictionnaire »). */
export interface Dictionary {
  lang: string;
  default_lang: string;
  revision: string;
  valid_at?: string | null;
  /** By space: a label by key, or (closed lists) by property then by value. */
  spaces: Record<string, Record<string, string | Record<string, string>>>;
  fallback?: { lang: string; keys: string[] } | null;
}

/** The label of `key` in `space` of a dictionary; a value of a list is `<property>/<value>`. */
export function lookup(
  dictionary: Dictionary | undefined,
  space: string,
  key: string,
): string | null {
  const entries = dictionary?.spaces[space];
  if (!entries) return null;
  const direct = entries[key];
  if (typeof direct === "string") return direct;
  const slash = key.lastIndexOf("/");
  if (slash < 0) return null;
  const nested = entries[key.slice(0, slash)];
  const value = typeof nested === "object" ? nested[key.slice(slash + 1)] : undefined;
  return typeof value === "string" ? value : null;
}

export interface LabelsDeps {
  queryClient: QueryClient;
  reads: Queries;
  /** The language of the context. */
  locale: () => string;
}

export function createLabels({ queryClient, reads, locale }: LabelsDeps) {
  const query = (lang: string) =>
    reads.byUrl<Dictionary>(
      ["v1", "labels", lang],
      `/v1/labels?lang=${encodeURIComponent(lang)}`,
      "labels",
    );

  return {
    /** The query of the dictionary of a language. */
    query,

    /**
     * The label of a key: the dictionary of the language (read on first use), the catalogs,
     * else the key. The key stays the identity: a screen shows it beside or on demand.
     */
    label(space: string, key: string, lang: string = locale()): string {
      const options = query(lang);
      const entry = queryClient.getQueryData<Entry<Dictionary>>(options.queryKey);
      if (entry === undefined) void queryClient.prefetchQuery(options);
      const found = lookup(entry?.data, space, key);
      if (found !== null) return found;
      const name = `labels.${space}.${key}`;
      const caught = translate(name);
      return caught === name ? key : caught;
    },

    /** The event `labels`: the dictionaries of another revision are read again. */
    changed(revision: string): Promise<void> {
      return queryClient.invalidateQueries({
        queryKey: ["v1", "labels"],
        predicate: (q) =>
          (q.state.data as Entry<Dictionary> | undefined)?.data.revision !== revision,
      });
    },
  };
}

export type Labels = ReturnType<typeof createLabels>;
