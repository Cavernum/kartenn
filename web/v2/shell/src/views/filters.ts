import {
  collectionOf,
  filterModel,
  parseFilters,
  queryClient,
  reads,
  type Declaration,
  type FilterField,
  type Filters,
  type ListPath,
} from "@kartenn/sdk";
import {
  checkFilter,
  filterOptions,
  filtersFromParams,
  filtersToParams,
  type Filter,
  type FilterModel,
  type FilterOperator,
  type FilterProperty,
  type FilterType,
  type ListOption,
  type Suggest,
} from "@kartenn/ui";
import { shallowReactive } from "vue";
import type { LocationQuery, LocationQueryRaw } from "vue-router";
import { STATES } from "../changesets/model.ts";
import { COLLECTIONS } from "./collections.ts";

// The typed filters of a list (REQ-021-18; design « Filtres, libellés, codes »; 020
// decision A7): one model per list, for the bar (`FilterPicker`) and the filters of the
// columns of its `DataGrid`, drawn from the contract (`filters.ts` of the SDK, generated
// like `dated.ts`) and from the declared properties the objects of the list carry. The URL
// holds the parameters of the list route as they are: a link is a request of the API
// (REQ-007-22); a value out of its type is read back, marked invalid, and never sent.

/** A declaration as the model reads it: its type, its labels and its parameters. */
export type DeclarationOfModel = Declaration & {
  readonly labels?: Readonly<Record<string, string>>;
  readonly params?: Readonly<Record<string, unknown>>;
};

/** The words the model needs, in the language of the moment. */
export interface FilterWords {
  /** The label of a parameter of the route (`domain`), `undefined` for its key alone. */
  field(name: string): string | undefined;
  /** The label of a value of a closed list of the core (`usage`, `host`). */
  value(name: string, value: string): string | undefined;
  /** The label of a declared property (dictionary of the 018, then its declaration). */
  property(key: string): string | undefined;
  /** The label of a value of a closed list of a declared property. */
  propertyValue(key: string, value: string): string | undefined;
}

/** What reads the suggestions: the first page of a list for a text (the cache of the SDK). */
export type SuggestRead = (
  path: ListPath,
  filters: Filters,
  signal: AbortSignal,
) => Promise<readonly Record<string, unknown>[]>;

/**
 * The size of the page of a suggestion read, the contract's default written out: it gives
 * the suggestions a key (and a URL) of their own, apart from the list a view or a panel
 * reads of the same collection — so that they read one page, never the ten pages such a
 * list keeps once stale, and a newer text cancels their read alone, never that list's.
 */
const SUGGESTION_PAGE = "100";

/** Through the cache of the SDK, at the page's dates; `signal` cancels the read. */
export const cacheSuggest: SuggestRead = async (path, filters, signal) => {
  const options = reads.list(path, { ...filters, page_size: SUGGESTION_PAGE } as never);
  const cancel = () => void queryClient.cancelQueries({ queryKey: options.queryKey, exact: true });
  signal.addEventListener("abort", cancel, { once: true });
  try {
    const data = await queryClient.fetchInfiniteQuery(options);
    return (data.pages[0]?.data.items ?? []) as readonly Record<string, unknown>[];
  } finally {
    signal.removeEventListener("abort", cancel);
  }
};

/** The parameters of a route that are a value of an object, by the type of their values. */
const VALUE_TYPES: Readonly<Record<string, Readonly<Record<string, FilterType>>>> = {
  "/v1/prefixes": { value: "prefix" },
  "/v1/addresses": { value: "address" },
  "/v1/ranges": { first: "address" },
};
/** The collections whose filters take a slug: their completion lists their objects. */
const FILTERS_OF_COLLECTION = new Set([
  "/v1/domains",
  "/v1/entities",
  "/v1/sites",
  "/v1/vlan-groups",
]);
/**
 * The parameters that take the identifier of an object (the contract says a string): its
 * completion lists the objects of the collection, the value is their identifier.
 */
const BY_ID: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  "/v1/anomalies": { domain: "/v1/domains", entity: "/v1/entities" },
  "/v1/property-gaps": { domain: "/v1/domains", entity: "/v1/entities" },
  "/v1/audit-events": { actor: "/v1/identities" },
};
/** The parameters that name a declared property (`property` of the gaps). */
const PROPERTY_KEYS: Readonly<Record<string, string>> = { "/v1/property-gaps": "property" };
/**
 * The parameters a closed list types though the contract says a string: the states of the
 * journal (spec 004) on the changesets.
 */
const CLOSED: Readonly<Record<string, Readonly<Record<string, readonly string[]>>>> = {
  "/v1/changesets": { state: STATES },
};
/**
 * The parameters that are instants though typed as days in the bar (`date`, AAAA-MM-JJ):
 * sent as the first instant of the day (`start`) or its last (`end`), in the browser's
 * zone, RFC 3339.
 */
const INSTANTS: Readonly<Record<string, Readonly<Record<string, "start" | "end">>>> = {
  "/v1/changesets": { from: "start", to: "end" },
  "/v1/audit-events": { from: "start", to: "end" },
};

/** A day of the bar (AAAA-MM-JJ) as the instant a list takes (RFC 3339, UTC). */
export function instantOfDay(day: string, bound: "start" | "end"): string {
  const local = new Date(`${day}T${bound === "start" ? "00:00:00.000" : "23:59:59.999"}`);
  return Number.isNaN(local.getTime()) ? day : local.toISOString();
}
/** The parameters a list keeps out of its bar: `q` is the search of the palette. */
const LEFT_OUT = new Set(["q"]);
/** The declared types whose values the model types (003, `TypeSpec::id`). */
const DECLARED: Readonly<Record<string, FilterType>> = {
  int: "number",
  date: "date",
  enum: "enum",
  ref: "enum",
  bool: "enum",
};

const text = (value: unknown) => (typeof value === "string" ? value : String(value ?? ""));
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** What reads an object a filter holds by its identifier, to name its chip. */
export type ObjectRead = (
  path: ListPath,
  id: string,
) => Promise<Readonly<Record<string, unknown>> | undefined>;

/** Through the cache of the SDK (`GET /v1/entities/{id}`); none for an unknown collection. */
export const cacheObject: ObjectRead = async (path, id) => {
  const kind = COLLECTIONS.find((collection) => collection.path === path)?.kind;
  if (kind === undefined) return undefined;
  return (await queryClient.fetchQuery(reads.object<Record<string, unknown>>(kind, id))).data;
};

/** How a reference names its object: its slug or value in fixed pitch, else its name. */
function referenceLabel(item: Readonly<Record<string, unknown>>): {
  label: string;
  mono: boolean;
} {
  const coded = item.slug ?? item.value;
  // A name in words (a person), a slug or a value in fixed pitch.
  return {
    label: text(coded ?? item.name ?? item.id),
    mono: coded !== undefined || item.name === undefined,
  };
}

/** The names of the objects the reference filters of a model hold, by identifier. */
interface ReferenceNames {
  keep(id: string, label: string): void;
  /** Its name, `undefined` until known; an identifier never suggested is read once. */
  of(path: ListPath, id: string): string | undefined;
}

/**
 * Those the suggestions listed, and those of a link opened afresh, read by their
 * identifier: a chip says the same words in any session (REQ-021-18, the link copied).
 * Reactive: the chip follows when the name arrives.
 */
function referenceNames(object: ObjectRead): ReferenceNames {
  const names = shallowReactive(new Map<string, string>());
  const asked = new Set<string>();
  return {
    keep: (id, label) => void names.set(id, label),
    of(path, id) {
      const known = names.get(id);
      if (known === undefined && UUID.test(id) && !asked.has(id)) {
        asked.add(id);
        object(path, id).then(
          (item) => {
            if (item && !names.has(id)) names.set(id, referenceLabel(item).label);
          },
          () => undefined,
        );
      }
      return known;
    },
  };
}

const options = (values: readonly string[], label: (value: string) => string | undefined) =>
  values.map((value): ListOption => ({ value, label: label(value) ?? value }));

/** Whether a list of the contract takes `q` (the start of a value, a slug or a name). */
function searchable(path: ListPath): boolean {
  try {
    return filterModel(path).some((field) => field.name === "q");
  } catch {
    return false;
  }
}

/**
 * The suggestions of a list for a text: `q` to the lists whose contract declares it; the
 * others read their first page and keep what the text matches.
 */
async function suggestFrom(
  path: ListPath,
  query: string,
  read: SuggestRead,
  signal: AbortSignal,
  option: (item: Record<string, unknown>) => ListOption,
): Promise<ListOption[]> {
  const q = query.trim();
  const asked = searchable(path);
  const items = await read(path, q !== "" && asked ? { q } : {}, signal);
  const found = items.map(option);
  return q === "" || asked ? found : filterOptions(found, q);
}

/** The suggestions of a collection whose filter takes a slug (`domain`, `parent`). */
function slugSuggest(path: ListPath, read: SuggestRead): Suggest {
  return (query, signal) =>
    suggestFrom(path, query, read, signal, (item) => ({
      value: text(item.slug),
      label: text(item.slug),
      mono: true,
    }));
}

/**
 * The suggestions of a reference to a kind of object (`core.owner` → entities): its value
 * is the identifier of the object, its label the slug (or value, or name: an identity) the
 * list shows of it.
 */
function referenceSuggest(path: ListPath, read: SuggestRead, names: ReferenceNames): Suggest {
  return (query, signal) =>
    suggestFrom(path, query, read, signal, (item) => {
      const { label, mono } = referenceLabel(item);
      names.keep(text(item.id), label);
      return { value: text(item.id), label, detail: text(item.id), mono };
    });
}

/** The values of a closed list of a declared property (`/values`, 016). */
function valuesSuggest(key: string, read: SuggestRead, words: FilterWords): Suggest {
  const path = `/v1/property-declarations/${encodeURIComponent(key)}/values` as ListPath;
  return async (query, signal) => {
    const items = await read(path, query.trim() === "" ? {} : { q: query.trim() }, signal);
    return items.map((item) => {
      const value = text(item.key);
      return { value, label: words.propertyValue(key, value) ?? value };
    });
  };
}

/** The namespace of the core's own properties (`core.site`, `core.owner`). */
const CORE_KEYS = "core.";

/**
 * The suggestions of a property key (the gaps' `property`): the declarations whose key or a
 * label starts with the text (`q`, a prefix, task 021-11p3), their first page; a text
 * without a dot reads too the keys of the core it starts (`site` → `core.site`), which
 * a prefix of the key alone would never reach. Each by its label, its key beside it.
 */
function keySuggest(read: SuggestRead, words: FilterWords): Suggest {
  const path = "/v1/property-declarations" as ListPath;
  return async (query, signal) => {
    const q = query.trim();
    const asked = q === "" ? [{}] : q.includes(".") ? [{ q }] : [{ q }, { q: CORE_KEYS + q }];
    const pages = await Promise.all(asked.map((filters) => read(path, filters, signal)));
    const keys = [...new Set(pages.flat().map((item) => text(item.key)))];
    return keys.map((key) => {
      const label = words.property(key);
      return label ? { value: key, label, detail: key } : { value: key, label: key, mono: true };
    });
  };
}

/** The filter of a declared property: one entry for its key, its tests as operators. */
function propertyFilter(
  key: string,
  fields: readonly FilterField[],
  declaration: DeclarationOfModel | undefined,
  words: FilterWords,
  read: SuggestRead,
  names: ReferenceNames,
): FilterProperty {
  const tests = new Set(fields.map((field) => field.property!.test));
  const operators: FilterOperator[] = [];
  if (tests.has("equals")) operators.push("eq");
  if (tests.has("between")) operators.push("between");
  if (tests.has("exists")) operators.push("exists", "absent");
  const declared = declaration?.type ?? "text";
  const type = DECLARED[declared] ?? "text";
  const property: FilterProperty = {
    key: `prop.${key}`,
    label: words.property(key),
    type,
    operators,
  };
  if (declared === "bool") {
    property.options = options(["true", "false"], (value) => words.value("boolean", value));
  } else if (declared === "enum") {
    property.suggest = valuesSuggest(key, read, words);
    property.valueLabel = (value) => words.propertyValue(key, value);
  } else if (declared === "ref") {
    const target = collectionOf(text(declaration?.params?.target));
    if (target) {
      property.suggest = referenceSuggest(target as ListPath, read, names);
      property.valueLabel = (value) => names.of(target as ListPath, value);
    } else property.type = "text";
  }
  return property;
}

/**
 * The model of the filters of a list: the parameters of its route (the contract), then the
 * declared properties its objects carry, each with its type, its operators and its domain.
 */
export function listFilterModel(
  path: ListPath,
  declarations: readonly DeclarationOfModel[],
  words: FilterWords,
  read: SuggestRead = cacheSuggest,
  object: ObjectRead = cacheObject,
): FilterModel {
  const fields = filterModel(path, declarations);
  const names = referenceNames(object);
  const values = VALUE_TYPES[path] ?? {};
  const within = fields.some((field) => field.name === "within");
  const valued = Object.keys(values).find((name) => fields.some((field) => field.name === name));
  const model: FilterProperty[] = [];
  const byKey = new Map<string, FilterField[]>();
  for (const field of fields) {
    if (field.property) {
      const known = byKey.get(field.property.key);
      if (known) known.push(field);
      else byKey.set(field.property.key, [field]);
      continue;
    }
    if (LEFT_OUT.has(field.name)) continue;
    // `within` is an operator of the value of the list (« Adresse dans 192.0.2.0/24 »).
    if (field.name === "within" && valued) continue;
    const label = words.field(field.name);
    const property: FilterProperty = { key: field.name, label, type: "text", operators: ["eq"] };
    if (values[field.name]) {
      property.type = values[field.name]!;
      if (within) {
        property.operators = ["eq", "within"];
        property.params = { within: "within" };
      }
    } else if (field.name === "within") property.type = "prefix";
    else if (INSTANTS[path]?.[field.name]) property.type = "date";
    else if (BY_ID[path]?.[field.name]) {
      property.type = "enum";
      const target = BY_ID[path][field.name] as ListPath;
      property.suggest = referenceSuggest(target, read, names);
      property.valueLabel = (value) => names.of(target, value);
    } else if (PROPERTY_KEYS[path] === field.name) {
      // Suggested by the server (`q`), never drawn from a catalog read whole.
      property.type = "enum";
      property.suggest = keySuggest(read, words);
      property.valueLabel = (value) => words.property(value);
    } else if (field.enum ?? CLOSED[path]?.[field.name]) {
      property.type = "enum";
      property.options = options(field.enum ?? CLOSED[path]![field.name]!, (value) =>
        words.value(field.name, value),
      );
    } else if (field.type === "boolean") {
      property.type = "enum";
      property.options = options(["true", "false"], (value) => words.value("boolean", value));
    } else if (field.type === "integer" || field.type === "number") property.type = "number";
    else if (field.collection && FILTERS_OF_COLLECTION.has(field.collection)) {
      property.type = "enum";
      property.suggest = slugSuggest(field.collection as ListPath, read);
      property.valueLabel = (value) => value;
    }
    model.push(property);
  }
  const declared = new Map(declarations.map((d) => [d.key, d]));
  for (const [key, keyed] of byKey) {
    model.push(propertyFilter(key, keyed, declared.get(key), words, read, names));
  }
  return model;
}

/** The query of a route as parameters, a repeated one kept repeated. */
export function paramsOf(query: LocationQuery): URLSearchParams {
  const params = new URLSearchParams();
  for (const [name, value] of Object.entries(query)) {
    for (const one of Array.isArray(value) ? value : [value]) {
      if (typeof one === "string") params.append(name, one);
    }
  }
  return params;
}

/** The filters of a query, read with the model: applied, or invalid (out of their type). */
export function filtersOfQuery(model: FilterModel, query: LocationQuery): Filter[] {
  return filtersFromParams(model, paramsOf(query)).filters;
}

/**
 * The query of a route for filters: the parameters of the filters (drafts left out), and
 * those of the query that are none of them (the dates of the page) kept as they are.
 */
export function queryOfFilters(
  model: FilterModel,
  filters: readonly Filter[],
  query: LocationQuery,
  /** Parameters outside the model that chips stand for (written back from the chips). */
  replaced: ReadonlySet<string> = new Set(),
): LocationQueryRaw {
  const { rest } = filtersFromParams(model, paramsOf(query));
  for (const name of replaced) rest.delete(name);
  const params = filtersToParams(model, filters, rest);
  const out: Record<string, string | string[]> = {};
  for (const [name, value] of params) {
    const known = out[name];
    out[name] = known === undefined ? value : [...[known].flat(), value];
  }
  return out;
}

/**
 * The filters a list sends: those the model accepts (an invalid one stays in the URL and on
 * its chip, never in the request), checked again against the contract by the SDK.
 */
export function sentFilters(
  path: ListPath,
  model: FilterModel,
  filters: readonly Filter[],
  declarations: readonly Declaration[],
): Filters {
  const accepted = filters.filter((filter) => !filter.draft && !checkFilter(model, filter));
  const params = filtersToParams(model, accepted);
  for (const [name, bound] of Object.entries(INSTANTS[path] ?? {})) {
    const day = params.get(name);
    if (day !== null) params.set(name, instantOfDay(day, bound));
  }
  return parseFilters(filterModel(path, declarations), params).filters;
}
