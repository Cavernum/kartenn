import type { Collection } from "../data/collections.ts";

// The fields of a quick edit (REQ-007-25, design « Écriture »), by collection: those a
// creation takes and those a merge patch changes (contract 006, `New*` and `*Patch`).

export type Field =
  | { name: string; kind: "text"; required?: boolean }
  | { name: string; kind: "choice"; options: readonly string[] }
  | { name: string; kind: "flag" }
  /** The addressing domain, a `{ id }` reference. */
  | { name: "domain"; kind: "domain" };

const NATURES = ["network", "container"] as const;
const ADDRESS_USAGES = ["host", "gateway", "vip", "anycast", "reserved"] as const;
const RANGE_USAGES = ["static", "dhcp", "reserved"] as const;

const slug: Field = { name: "slug", kind: "text", required: true };

/** What a creation asks, in order. */
export const CREATION: Record<Collection, readonly Field[]> = {
  entities: [slug],
  sites: [slug],
  domains: [slug],
  prefixes: [
    { name: "domain", kind: "domain" },
    { name: "value", kind: "text", required: true },
    { name: "nature", kind: "choice", options: NATURES },
    { name: "point_to_point", kind: "flag" },
    { name: "anycast", kind: "flag" },
  ],
  ranges: [
    { name: "domain", kind: "domain" },
    { name: "first", kind: "text", required: true },
    { name: "last", kind: "text", required: true },
    { name: "usage", kind: "choice", options: RANGE_USAGES },
  ],
  addresses: [
    { name: "domain", kind: "domain" },
    { name: "value", kind: "text", required: true },
    { name: "usage", kind: "choice", options: ADDRESS_USAGES },
  ],
};

/** What a modification changes, in order. */
export const MODIFICATION: Record<Collection, readonly Field[]> = {
  entities: [slug],
  sites: [slug],
  domains: [slug],
  prefixes: [
    { name: "value", kind: "text", required: true },
    { name: "nature", kind: "choice", options: NATURES },
    { name: "point_to_point", kind: "flag" },
    { name: "anycast", kind: "flag" },
  ],
  ranges: [{ name: "usage", kind: "choice", options: RANGE_USAGES }],
  addresses: [{ name: "usage", kind: "choice", options: ADDRESS_USAGES }],
};

/** A value as the form holds it: text, a choice, a flag, or the id of a domain. */
export type Value = string | boolean;
export type Values = Record<string, Value>;

/** The form of `fields` filled from an object (its current values), or empty. */
export function initial(fields: readonly Field[], item: Record<string, unknown> | null): Values {
  const values: Values = {};
  for (const field of fields) {
    const current = item?.[field.name];
    switch (field.kind) {
      case "flag":
        values[field.name] = current === true;
        break;
      case "choice":
        values[field.name] = typeof current === "string" ? current : field.options[0]!;
        break;
      case "domain":
        values[field.name] =
          typeof current === "object" && current !== null && "id" in current
            ? String((current as { id: unknown }).id)
            : "";
        break;
      default:
        values[field.name] = typeof current === "string" ? current : "";
    }
  }
  return values;
}

function member(field: Field, value: Value): unknown {
  return field.kind === "domain" ? { id: value } : value;
}

/** The members of a creation. */
export function creation(fields: readonly Field[], values: Values): Record<string, unknown> {
  const body: Record<string, unknown> = {};
  for (const field of fields) body[field.name] = member(field, values[field.name] ?? "");
  return body;
}

/** The merge patch (RFC 7396) of what changed from `before`: the changed members only. */
export function patch(
  fields: readonly Field[],
  before: Values,
  values: Values,
): Record<string, unknown> {
  const body: Record<string, unknown> = {};
  for (const field of fields) {
    const value = values[field.name];
    if (value !== undefined && value !== before[field.name])
      body[field.name] = member(field, value);
  }
  return body;
}
