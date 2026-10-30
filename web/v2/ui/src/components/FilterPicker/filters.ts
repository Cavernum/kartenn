// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { t, type MessageKey } from "../../i18n.ts";
import type { Suggest } from "../Combobox/combobox.ts";
import type { ListOption } from "../Select/select.ts";

/**
 * Typed filters (design « Inventaire », line 6; REQ-020-20): the model of a list (its
 * properties, their type, their operators and their domain), a filter, its check, and its
 * place in the URL. The parameters of the URL are those of the list route, as they are (a
 * link is a request of the API, 021/X18, REQ-007-22), in the grammar of the 006
 * (« Filtres ») : `key=value` (repeated: one of), `key:between=a..b` (an open bound left
 * empty), `key:exists=true|false`, any other operator after the key and `:`
 * (`key:contains=…`); a property may give an operator a parameter of its own
 * (`within=<CIDR>` of the core).
 */

/** The type of the values of a property: it chooses the field and the check. */
export type FilterType = "text" | "enum" | "number" | "prefix" | "address" | "date";

/**
 * The operators: `eq` takes one value or more (one of), `between` two bounds (either may
 * be empty, not both), `exists` and `absent` none, the others one.
 */
export type FilterOperator =
  "eq" | "ne" | "contains" | "within" | "before" | "after" | "between" | "exists" | "absent";

/** A property a list can be filtered on (a parameter of its route, a declared property). */
export interface FilterProperty {
  /** The parameter of the route (`domain`, `usage`, `prop.policy.environment`). */
  key: string;
  /** The label in the language; absent, the key is shown, in fixed pitch (REQ-020-21). */
  label?: string | undefined;
  type: FilterType;
  /** The operators the route serves for it; the first one is chosen by default. */
  operators: readonly FilterOperator[];
  /** A closed domain known to the host: its values (key and label). */
  options?: readonly ListOption[] | undefined;
  /** A closed domain read from the API (its cache): the suggestions of a text. */
  suggest?: Suggest | undefined;
  /** The label of a value of the domain that `options` do not hold (read from the URL). */
  valueLabel?: ((value: string) => string | undefined) | undefined;
  /** A parameter of its own for an operator (`{ within: "within" }`). */
  params?: Partial<Record<FilterOperator, string>> | undefined;
}

/** The properties of a list, in the order of the picker. */
export type FilterModel = readonly FilterProperty[];

/** A filter: a property, an operator and its values. */
export interface Filter {
  key: string;
  operator: FilterOperator;
  values: string[];
  /** Being written, not applied: shown dashed, never in the URL nor sent. */
  draft?: boolean | undefined;
}

/** The operators that take no value. */
const NO_VALUE: readonly FilterOperator[] = ["exists", "absent"];

/** How many values an operator takes: none, one, two bounds, or one and more. */
export function arity(operator: FilterOperator): 0 | 1 | 2 | "many" {
  if (NO_VALUE.includes(operator)) return 0;
  if (operator === "between") return 2;
  return operator === "eq" ? "many" : 1;
}

const OPERATOR_WORD: Record<FilterOperator, MessageKey> = {
  eq: "filterIs",
  ne: "filterIsNot",
  contains: "filterContains",
  within: "filterWithin",
  before: "filterBefore",
  after: "filterAfter",
  between: "filterBetween",
  exists: "filterExists",
  absent: "filterAbsent",
};

/** The word of an operator in the language (« est », « est l'un de » for several values). */
export function operatorLabel(operator: FilterOperator, count = 1): string {
  if (operator === "eq" && count > 1) return t("filterIsOneOf");
  return t(OPERATOR_WORD[operator]);
}

/** The property of a key in a model. */
export const propertyOf = (model: FilterModel, key: string): FilterProperty | undefined =>
  model.find((property) => property.key === key);

/**
 * The type of the values an operator takes: `within` (« dans le préfixe ») a prefix,
 * whatever its property holds (an address, a range, a prefix); the others, the property's.
 */
export const valueType = (property: FilterProperty, operator?: FilterOperator): FilterType =>
  operator === "within" ? "prefix" : property.type;

/** The values are keys, addresses, prefixes, numbers or dates: fixed pitch. */
export const monoValues = (
  property: FilterProperty | undefined,
  operator?: FilterOperator,
): boolean => !!property && valueType(property, operator) !== "text";

/**
 * The label of a value of a closed domain, else nothing (the key is then shown); an option
 * labelled by its own key has none.
 */
export function valueLabel(
  property: FilterProperty | undefined,
  value: string,
): string | undefined {
  const option = property?.options?.find((o) => o.value === value);
  if (option) return option.label === option.value ? undefined : option.label;
  return property?.valueLabel?.(value);
}

/** A sentence of the library with its `{name}` holes filled. */
function fill(key: MessageKey, values: Record<string, string | number> = {}): string {
  return t(key).replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ""));
}

// Addresses (RFC 791, RFC 4291 § 2.2) as integers, for the check of a prefix (its host
// bits, 006: a canonical value is required) and the canonical text of RFC 5952.

function parseIPv4(text: string): bigint | null {
  const parts = text.split(".");
  if (parts.length !== 4) return null;
  let value = 0n;
  for (const part of parts) {
    // No leading zero: « 010 » is octal for some readers.
    if (!/^(0|[1-9]\d{0,2})$/.test(part) || Number(part) > 255) return null;
    value = (value << 8n) | BigInt(part);
  }
  return value;
}

function parseIPv6(text: string): bigint | null {
  if (!/^[0-9a-fA-F:.]+$/.test(text) || text.split("::").length > 2) return null;
  let body = text;
  // An IPv4 address in the last 32 bits (RFC 4291 § 2.2, form 3): two groups of hex.
  const dotted = /:(\d+\.\d+\.\d+\.\d+)$/.exec(text);
  if (dotted) {
    const tail = parseIPv4(dotted[1]!);
    if (tail === null) return null;
    const hex = (shift: bigint) => ((tail >> shift) & 0xffffn).toString(16);
    body = `${text.slice(0, dotted.index + 1)}${hex(16n)}:${hex(0n)}`;
  } else if (text.includes(".")) return null;
  const [head, rest] = body.split("::") as [string, string | undefined];
  const groups = (part: string) => (part === "" ? [] : part.split(":"));
  const left = groups(head);
  const right = rest === undefined ? [] : groups(rest);
  const missing = 8 - left.length - right.length;
  if (rest === undefined ? missing !== 0 : missing < 1) return null;
  const all = [...left, ...Array<string>(rest === undefined ? 0 : missing).fill("0"), ...right];
  let value = 0n;
  for (const group of all) {
    if (!/^[0-9a-fA-F]{1,4}$/.test(group)) return null;
    value = (value << 16n) | BigInt(Number.parseInt(group, 16));
  }
  return value;
}

function formatIPv4(value: bigint): string {
  return [24n, 16n, 8n, 0n].map((shift) => String((value >> shift) & 0xffn)).join(".");
}

/** RFC 5952 § 4: lower case, no leading zero, the longest run of two zeros or more as `::`. */
function formatIPv6(value: bigint): string {
  const groups = Array.from({ length: 8 }, (_, i) =>
    Number((value >> BigInt(112 - 16 * i)) & 0xffffn),
  );
  let best = { at: -1, length: 1 };
  for (let i = 0; i < 8;) {
    let j = i;
    while (j < 8 && groups[j] === 0) j += 1;
    if (j - i > best.length) best = { at: i, length: j - i };
    i = j === i ? i + 1 : j;
  }
  const hex = groups.map((g) => g.toString(16));
  if (best.at < 0) return hex.join(":");
  return `${hex.slice(0, best.at).join(":")}::${hex.slice(best.at + best.length).join(":")}`;
}

/** An address: its family, its integer value. */
function parseAddress(text: string): { bits: 32 | 128; value: bigint } | null {
  const v4 = parseIPv4(text);
  if (v4 !== null) return { bits: 32, value: v4 };
  const v6 = text.includes(":") ? parseIPv6(text) : null;
  return v6 === null ? null : { bits: 128, value: v6 };
}

const DATE = /^(\d{4})-(\d{2})-(\d{2})$/;

/**
 * Why a value does not fit the type of its property, in a sentence of the language; nothing
 * when it fits. A closed domain known to the host (`options`) holds its values; one read
 * from the API is checked by the route.
 */
export function checkValue(
  property: FilterProperty,
  value: string,
  operator?: FilterOperator,
): string | undefined {
  const text = value.trim();
  if (text === "") return t("filterValueMissing");
  switch (valueType(property, operator)) {
    case "text":
      return undefined;
    case "enum":
      if (property.options && !property.options.some((o) => o.value === text))
        return fill("filterValueUnknown", { value: text });
      return undefined;
    case "number":
      return /^-?\d+(\.\d+)?$/.test(text) ? undefined : t("filterValueNumber");
    case "address":
      return parseAddress(text) ? undefined : t("filterValueAddress");
    case "prefix": {
      const [address, length, extra] = text.split("/");
      const parsed = address === undefined ? null : parseAddress(address);
      if (!parsed || length === undefined || extra !== undefined || !/^\d{1,3}$/.test(length))
        return t("filterValuePrefix");
      const bits = Number(length);
      if (bits > parsed.bits) return fill("filterValuePrefixLength", { max: parsed.bits });
      const host = (1n << BigInt(parsed.bits - bits)) - 1n;
      if ((parsed.value & host) !== 0n) {
        const network = parsed.value & ~host;
        const canonical = parsed.bits === 32 ? formatIPv4(network) : formatIPv6(network);
        return fill("filterValuePrefixHost", { prefix: `${canonical}/${bits}` });
      }
      return undefined;
    }
    case "date": {
      const match = DATE.exec(text);
      if (!match) return t("filterValueDate");
      const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
      if (month < 1 || month > 12) return t("filterValueMonth");
      const days = new Date(Date.UTC(year, month, 0)).getUTCDate();
      if (day < 1 || day > days) return fill("filterValueDay", { days });
      return undefined;
    }
  }
}

/** Whether a bound is lower than or equal to the other, for the numbers and the dates. */
function ordered(type: FilterType, low: string, high: string): boolean {
  if (type === "number") return Number(low) <= Number(high);
  if (type === "date") return low <= high;
  return true;
}

/**
 * Why a filter cannot be applied (an unknown property, an operator the route does not
 * serve, a missing or ill-typed value), in a sentence; nothing when it can. A filter read
 * from the URL keeps its values as they are: its chip says it is invalid and why.
 */
export function checkFilter(model: FilterModel, filter: Filter): string | undefined {
  const property = propertyOf(model, filter.key);
  if (!property) return fill("filterPropertyUnknown", { key: filter.key });
  if (!property.operators.includes(filter.operator)) return t("filterOperatorUnserved");
  const count = arity(filter.operator);
  if (count === 0) {
    const [flag] = filter.values;
    return flag === undefined || flag === (filter.operator === "exists" ? "true" : "false")
      ? undefined
      : t("filterExistsValue");
  }
  if (count === 2) {
    if (filter.values.length !== 2) return t("filterBoundsForm");
    const [low = "", high = ""] = filter.values;
    if (low.trim() === "" && high.trim() === "") return t("filterBoundsMissing");
    for (const bound of [low, high])
      if (bound.trim() !== "") {
        const reason = checkValue(property, bound);
        if (reason) return reason;
      }
    if (low.trim() !== "" && high.trim() !== "" && !ordered(property.type, low, high))
      return t("filterBoundsOrder");
    return undefined;
  }
  if (filter.values.length === 0) return t("filterValueMissing");
  if (count === 1 && filter.values.length > 1) return t("filterValueOne");
  for (const value of filter.values) {
    const reason = checkValue(property, value, filter.operator);
    if (reason) return reason;
  }
  return undefined;
}

/** The parameter of the URL of an operator of a property. */
export function paramOf(key: string, operator: FilterOperator, property?: FilterProperty): string {
  const own = property?.params?.[operator];
  if (own) return own;
  if (operator === "eq") return key;
  if (operator === "absent") return `${key}:exists`;
  return `${key}:${operator}`;
}

/**
 * The filters as parameters of the list route, in their order, drafts left out. A filter
 * that does not check keeps its values as they are (the URL holds what the user wrote);
 * the host sends only those `checkFilter` accepts.
 */
export function filtersToParams(
  model: FilterModel,
  filters: readonly Filter[],
  params: URLSearchParams = new URLSearchParams(),
): URLSearchParams {
  for (const filter of filters) {
    if (filter.draft) continue;
    const name = paramOf(filter.key, filter.operator, propertyOf(model, filter.key));
    const [first = "", second = ""] = filter.values;
    if (filter.operator === "eq") for (const value of filter.values) params.append(name, value);
    else if (filter.operator === "between")
      params.append(name, filter.values.length === 2 ? `${first}..${second}` : first);
    else if (filter.operator === "exists") params.append(name, filter.values[0] ?? "true");
    else if (filter.operator === "absent") params.append(name, filter.values[0] ?? "false");
    else params.append(name, first);
  }
  return params;
}

/**
 * The filters a query holds, read with the model of the list, and the parameters that are
 * none of its filters (`valid_at`, the sort…), kept as they are. Repeated, `key=` is one
 * filter of several values; `key:exists` is `exists` or `absent` after its value; any other
 * parameter of a filter is one filter per value. A value that does not fit is kept:
 * `checkFilter` says why.
 */
export function filtersFromParams(
  model: FilterModel,
  params: URLSearchParams,
): { filters: Filter[]; rest: URLSearchParams } {
  const known = new Map<string, { key: string; operators: FilterOperator[] }>();
  for (const property of model)
    for (const operator of property.operators) {
      const name = paramOf(property.key, operator, property);
      const entry = known.get(name);
      if (!entry) known.set(name, { key: property.key, operators: [operator] });
      else if (entry.key === property.key) entry.operators.push(operator);
    }
  const filters: Filter[] = [];
  const rest = new URLSearchParams();
  const many = new Map<string, Filter>();
  for (const [name, value] of params) {
    const entry = known.get(name);
    if (!entry) {
      rest.append(name, value);
      continue;
    }
    const { key, operators } = entry;
    if (operators.includes("eq")) {
      const open = many.get(name);
      if (open) open.values.push(value);
      else {
        const filter: Filter = { key, operator: "eq", values: [value] };
        many.set(name, filter);
        filters.push(filter);
      }
    } else if (operators.includes("exists") || operators.includes("absent")) {
      if (value === "true" && operators.includes("exists"))
        filters.push({ key, operator: "exists", values: [] });
      else if (value === "false" && operators.includes("absent"))
        filters.push({ key, operator: "absent", values: [] });
      else filters.push({ key, operator: operators[0]!, values: [value] });
    } else if (operators[0] === "between") {
      const at = value.indexOf("..");
      const values = at < 0 ? [value] : [value.slice(0, at), value.slice(at + 2)];
      filters.push({ key, operator: "between", values });
    } else filters.push({ key, operator: operators[0]!, values: [value] });
  }
  return { filters, rest };
}

/** The words of a chip: the property, the operator and the values, labels before keys. */
export interface FilterWords {
  property: string;
  /** No label: the key is shown, in fixed pitch. */
  propertyMono: boolean;
  operator: string;
  value: string | undefined;
  valueMono: boolean;
}

/** How a filter reads, in the language (REQ-020-21: a label, else the key). */
export function filterWords(model: FilterModel, filter: Filter): FilterWords {
  const property = propertyOf(model, filter.key);
  const shown = filter.values.map((value) => valueLabel(property, value) ?? value);
  const labelled = filter.values.some((value) => valueLabel(property, value) !== undefined);
  const words: FilterWords = {
    property: property?.label ?? filter.key,
    propertyMono: !property?.label,
    operator: operatorLabel(filter.operator, filter.values.length),
    value: shown.join(", ") || undefined,
    valueMono: monoValues(property, filter.operator) && !labelled,
  };
  if (filter.operator === "between" && filter.values.length === 2) {
    const [low = "", high = ""] = filter.values;
    if (low && high) words.value = `${low} ${t("filterAnd")} ${high}`;
    else {
      words.operator = t(low ? "filterFrom" : "filterUpTo");
      words.value = low || high || undefined;
    }
  }
  if (arity(filter.operator) === 0 && !checkFilter(model, filter)) words.value = undefined;
  return words;
}

/** The sentence of a filter (« Environnement est Production »), to name it. */
export function filterSentence(model: FilterModel, filter: Filter): string {
  const words = filterWords(model, filter);
  return [words.property, words.operator, words.value].filter(Boolean).join(" ");
}

/** The parameter of a filter: two filters on the same one cannot both stand (one replaces). */
export const filterParam = (model: FilterModel, filter: Filter): string =>
  paramOf(filter.key, filter.operator, propertyOf(model, filter.key));
