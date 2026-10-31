import { api, ApiProblem } from "@kartenn/sdk";
import { setContext } from "@kartenn/sdk/host";
import { computed, type ComputedRef } from "vue";
import {
  useRoute,
  type LocationQuery,
  type RouteLocationNormalized,
  type RouteLocationRaw,
  type Router,
} from "vue-router";

// The dated context (REQ-021-5, 007/W9; repris de web/shell/src/time): `valid_at` and
// `as_of` live in the query of the URL, so that a link shows what its author saw; the
// guard of time (`beforeEach`) writes the dates of the page left into a target that names
// neither, before the guard of prefetch (`beforeResolve`, prefetch/intent.ts) reads the
// target: a target left without dates there is the explicit return to the present.

export interface Time {
  validAt: string | null;
  asOf: string | null;
  present: boolean;
}

/** A period as the date pill applies it. */
export interface Period {
  validAt: string | null;
  asOf: string | null;
}

/** An instant of RFC 3339 in UTC (`Z`), as the API takes it (006 « Lectures datées »). */
const INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/;

function instant(value: LocationQuery[string] | undefined): string | null {
  const text = Array.isArray(value) ? value[0] : value;
  return typeof text === "string" && INSTANT.test(text) && !Number.isNaN(Date.parse(text))
    ? text
    : null;
}

/** The dated context of a query; a malformed date counts as absent. */
export function timeOf(query: LocationQuery): Time {
  const validAt = instant(query.valid_at);
  const asOf = instant(query.as_of);
  return { validAt, asOf, present: validAt === null && asOf === null };
}

let leavingForPresent = false;

/**
 * The guard of time: a navigation that names neither date keeps those of the page it
 * leaves, unless it is the explicit return to the present.
 */
export function carryTime(
  to: RouteLocationNormalized,
  from: RouteLocationNormalized,
): RouteLocationRaw | true {
  if (leavingForPresent) {
    leavingForPresent = false;
    return true;
  }
  const before = timeOf(from.query);
  if (before.present || "valid_at" in to.query || "as_of" in to.query) return true;
  const query = { ...to.query };
  if (before.validAt) query.valid_at = before.validAt;
  if (before.asOf) query.as_of = before.asOf;
  return { path: to.path, query, hash: to.hash, replace: true };
}

/** Installs the guard, and pushes the context to the SDK after each navigation. */
export function installTime(router: Router): void {
  router.beforeEach(carryTime);
  router.afterEach((to) => {
    const { validAt, asOf } = timeOf(to.query);
    setContext({ time: { validAt, asOf } });
  });
}

/**
 * Moves the current page to `period` (`null` clears a date); with neither date, the
 * explicit return to the present.
 */
export function goToPeriod(router: Router, { validAt, asOf }: Period): Promise<unknown> {
  const route = router.currentRoute.value;
  const query = { ...route.query };
  delete query.valid_at;
  delete query.as_of;
  if (validAt) query.valid_at = validAt;
  if (asOf) query.as_of = asOf;
  leavingForPresent = !validAt && !asOf;
  return router.push({ path: route.path, query, hash: route.hash }).finally(() => {
    leavingForPresent = false;
  });
}

/** Back to the present: the same page without the dates. */
export function toPresent(router: Router): Promise<unknown> {
  return goToPeriod(router, { validAt: null, asOf: null });
}

/** The dated context of the current route, reactive. */
export function useTime(): ComputedRef<Time> {
  const route = useRoute();
  return computed(() => timeOf(route.query));
}

/**
 * What may be written (REQ-021-5, 007/W9): nothing under `as_of`, the draft included; a
 * `valid_at` off the present leaves the draft only (its changes take effect at that date).
 */
export function writesOf(time: Time): { quickEdit: boolean; draft: boolean } {
  return { quickEdit: time.present, draft: time.asOf === null };
}

/** The refusal of a date by the API, on its field of the period picker. */
export type PeriodRefusal = Partial<Record<keyof Period, string>>;

/** What the check reads: a dated read of the API by URL, its problem thrown. */
export type Probe = (url: string) => Promise<unknown>;

const probe: Probe = (url) => api.request("GET", url);

/**
 * Asks the API whether it reads at `period` before the URL takes it, so that its refusal
 * comes back on the field of the date pill (020, `DatePill` `errors`): the browser's
 * clock refuses an `as_of` it sees in the future, the database's `now()` may still be
 * earlier (`as_of is later than the time of kartenn`). The lightest dated read: one
 * domain. Any answer but a 400 lets the period through (a 403 is a screen's business).
 * At the present, nothing to ask.
 */
export async function checkPeriod(
  period: Period,
  get: Probe = probe,
): Promise<PeriodRefusal | null> {
  if (!period.validAt && !period.asOf) return null;
  const search = new URLSearchParams({ page_size: "1" });
  if (period.validAt) search.set("valid_at", period.validAt);
  if (period.asOf) search.set("as_of", period.asOf);
  try {
    await get(`/v1/domains?${search}`);
    return null;
  } catch (error) {
    if (!(error instanceof ApiProblem) || error.status !== 400) return null;
    const reason = error.detail ?? error.title;
    const field: keyof Period = period.asOf && /as_of/.test(reason) ? "asOf" : "validAt";
    return { [field]: reason };
  }
}
