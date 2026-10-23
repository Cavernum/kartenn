// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The model of the filters of a list (design « Filtres, libellés, codes », REQ-021-18):
// the parameters the contract declares (`api/filters.ts`, generated) and, at run time, a
// filter per declared property that the objects of the list carry (003, 016). Filters live
// in the URL as the query of the route: a link is a request of the API (REQ-007-22).

import { FILTERS, type Filter } from "../api/filters.ts";
import type { components } from "../api/schema";
import { normalize, type Filters } from "./keys.ts";
import { qualify, typeOfCollection } from "./types.ts";

/** A declaration of a property, as `GET /v1/property-declarations` lists it. */
export type Declaration = Pick<
  components["schemas"]["DeclarationItem"],
  "key" | "type" | "carriers"
>;

/** A filter a list offers, concrete: a property filter names its key. */
export interface FilterField {
  readonly name: string;
  /** `string`, `integer`, `number`, `boolean`. */
  readonly type: string;
  readonly enum?: readonly string[];
  readonly pattern?: string;
  /** The list its values come from, for the completion. */
  readonly collection?: string;
  /** The property it filters on, and how. */
  readonly property?: { readonly key: string; readonly test: "equals" | "between" | "exists" };
}

/** The type of a filter on a property of a declared type (003, `TypeSpec::id`). */
const VALUE_TYPE: Record<string, string> = { int: "integer", bool: "boolean" };
/** The declared types whose values are ordered: `:between` applies to them. */
const ORDERED = new Set(["int", "date"]);

function concrete(filter: Filter): FilterField {
  return {
    name: filter.name,
    type: filter.type,
    ...(filter.enum ? { enum: filter.enum } : {}),
    ...(filter.pattern ? { pattern: filter.pattern } : {}),
    ...(filter.collection ? { collection: filter.collection } : {}),
  };
}

function propertyFields(template: Filter, declarations: readonly Declaration[], type: string) {
  const [, suffix = ""] = /^prop\.\{key\}(:.*)?$/.exec(template.name) ?? [];
  const test = suffix === ":between" ? "between" : suffix === ":exists" ? "exists" : "equals";
  const pattern = new RegExp(template.template!.key.pattern);
  return declarations
    .filter((d) => pattern.test(d.key))
    .filter((d) => d.carriers.length === 0 || d.carriers.some((c) => qualify(c) === type))
    .filter((d) => test !== "between" || ORDERED.has(d.type))
    .map((d): FilterField => ({
      name: `prop.${d.key}${suffix}`,
      type:
        test === "exists"
          ? "boolean"
          : test === "between"
            ? "string"
            : (VALUE_TYPE[d.type] ?? "string"),
      ...(test === "equals" && d.type === "enum"
        ? { collection: `/v1/property-declarations/${d.key}/values` }
        : {}),
      ...(test === "between" ? { pattern: "\\.\\." } : {}),
      property: { key: d.key, test },
    }));
}

/** The filters a list offers: those of its route, and those of the declared properties. */
export function filterModel(
  path: string,
  declarations: readonly Declaration[] = [],
): readonly FilterField[] {
  const declared = FILTERS[path];
  if (!declared) throw new TypeError(`${path} is not a list of the contract`);
  const type = typeOfCollection(path);
  return declared.flatMap((filter) =>
    filter.template
      ? type === null
        ? []
        : propertyFields(filter, declarations, type)
      : [concrete(filter)],
  );
}

/** Why a value of the URL was set aside. */
export interface Rejected {
  readonly name: string;
  readonly value: string;
  readonly reason: "unknown" | "enum" | "type" | "pattern";
}

function check(field: FilterField, value: string): Rejected["reason"] | null {
  if (field.enum && !field.enum.includes(value)) return "enum";
  if (field.type === "boolean" && value !== "true" && value !== "false") return "type";
  if (field.type === "integer" && !/^-?\d+$/.test(value)) return "type";
  if (field.pattern && !new RegExp(field.pattern).test(value)) return "pattern";
  return null;
}

/**
 * The filters of a URL for a list, checked against its model: unknown names and values
 * out of their domain are set aside and reported, never sent.
 */
export function parseFilters(
  model: readonly FilterField[],
  query: URLSearchParams | string,
): { filters: Filters; rejected: readonly Rejected[] } {
  const params = typeof query === "string" ? new URLSearchParams(query) : query;
  const fields = new Map(model.map((field) => [field.name, field]));
  const filters: Record<string, string | string[]> = {};
  const rejected: Rejected[] = [];
  for (const [name, value] of params) {
    if (name === "valid_at" || name === "as_of" || name === "cursor" || name === "page_size") {
      continue;
    }
    const field = fields.get(name);
    const reason = field ? check(field, value) : "unknown";
    if (reason) {
      rejected.push({ name, value, reason });
      continue;
    }
    const known = filters[name];
    filters[name] = known === undefined ? value : [...[known].flat(), value];
  }
  return { filters, rejected };
}

/** The query of a URL for filters, in the canonical order of the keys of the cache. */
export function filterSearch(filters: Filters): string {
  const params = new URLSearchParams();
  for (const [name, value] of Object.entries(normalize(filters))) {
    for (const one of typeof value === "string" ? [value] : value) params.append(name, one);
  }
  return params.toString();
}
