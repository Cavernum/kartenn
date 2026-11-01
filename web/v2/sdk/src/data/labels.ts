// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The labels of keys (spec 018, REQ-021-19; design « Filtres, libellés, codes »): objects
// carry keys only; `GET /v1/labels` serves the dictionary of a language, a query of the
// cache never stale until the event `labels` of the stream names another revision. A label
// is looked up in the dictionary, then in the catalogs of the shell and the plugins
// (`labels.<kind>.<key>`), else the key itself is shown. A label changing reads no object.

import type { QueryClient } from "@tanstack/query-core";
import type { components } from "../api/schema";
import { translate } from "../i18n.ts";
import type { Entry, Queries, ReadOptions } from "./cache.ts";

/** The labels of a space: a label by key, or (closed lists) by property then by value. */
export type Space = Record<string, string | Record<string, string>>;

/**
 * A dictionary of labels, as `GET /v1/labels` serves it (018, « Dictionnaire »; schema
 * `Labels`), its spaces typed: the contract leaves them an open map, which `isDictionary`
 * checks on every answer.
 */
export type Dictionary = Omit<components["schemas"]["Labels"], "spaces"> & {
  spaces: Record<string, Space>;
};

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const isLabels = (value: unknown) =>
  isObject(value) && Object.values(value).every((label) => typeof label === "string");

/** Whether a body is a dictionary: its revision, and spaces of labels or lists of labels. */
export function isDictionary(body: unknown): body is Dictionary {
  if (!isObject(body) || typeof body["revision"] !== "string" || !isObject(body["spaces"])) {
    return false;
  }
  return Object.values(body["spaces"]).every(
    (space) =>
      isObject(space) &&
      Object.values(space).every((entry) => typeof entry === "string" || isLabels(entry)),
  );
}

/** The key of the dictionary of a language: by language only, its revision in its body. */
export type DictionaryKey = readonly ["v1", "labels", string];

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
  // `lang` prevails over `Accept-Language` (018); the present, whatever the dates of the
  // context: the key is the language alone (design 021).
  // The body checked, the entry kept as it is (a 304 gives back the same object).
  const query = (lang: string): ReadOptions<Dictionary, DictionaryKey> => {
    const read = reads.keyed(["v1", "labels", lang] as const, "/v1/labels", {
      query: { lang },
      family: "labels",
    });
    return {
      ...read,
      queryFn: async (context) => {
        const entry: Entry<unknown> = await read.queryFn(context);
        if (!isDictionary(entry.data)) throw new TypeError("GET /v1/labels: not a dictionary");
        return entry as Entry<Dictionary>;
      },
    };
  };

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
