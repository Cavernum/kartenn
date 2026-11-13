import type { components } from "@kartenn/sdk";

/** A deviation from an expected value, as `GET /v1/property-gaps` lists it (016, 021/A36). */
export type Gap = components["schemas"]["Gap"];

/**
 * What the members of a gap tell of its values: a gap evaluated since its values are
 * stored always carries `source` and `expected`; masked, both values are left out
 * (`expected` too); stored before them (migration 0075), `source` as well. `resolved: null`
 * alone says nothing. Read by the list of the gaps and by the notice of a record.
 */
export type GapDetail = "values" | "masked" | "unevaluated";

export function gapDetail(gap: Gap): GapDetail {
  return gap.source == null ? "unevaluated" : gap.expected == null ? "masked" : "values";
}

/** No value: `null`, or an empty set. */
export function noValue(value: unknown): boolean {
  return value == null || (Array.isArray(value) && value.length === 0);
}
