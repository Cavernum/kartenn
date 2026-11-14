import { api, ApiProblem } from "@kartenn/sdk";
import { setContext } from "@kartenn/sdk/host";
import type { WidthClass } from "@kartenn/ui";
import { computed, type ComputedRef } from "vue";
import {
  useRoute,
  type LocationQuery,
  type RouteLocationNormalized,
  type RouteLocationRaw,
  type Router,
} from "vue-router";
import { frameWidth } from "../frame/width.ts";

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

/**
 * Installs the guard, and pushes the context to the SDK after each navigation. A move in
 * the history (Back, Forward) goes to the URL as it was, dates or none: never carried.
 * Its listener goes before the router's (installed before the first navigation), so the
 * guard knows the navigation it starts is a traversal.
 */
export function installTime(router: Router): void {
  let traversal = false;
  router.options.history.listen(() => {
    traversal = true;
  });
  router.beforeEach((to, from) => {
    if (!traversal) return carryTime(to, from);
    traversal = false;
    return true;
  });
  router.afterEach((to) => {
    traversal = false;
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
 * Why nothing at all may be written: the view read as known at a past time (`as_of`), or a
 * frame narrower than 1024 px (`narrow`, REQ-021-29, Q-021-12). Its sentence is
 * `edit.why.<refusal>` (`refusalWords`).
 */
export type WriteRefusal = "past" | "narrow";

/**
 * What may be written: at once (in place, applied), to the draft; and when neither, why
 * (`null` when the draft is open: under `valid_at` alone, it is the only way).
 */
export interface Writes {
  quickEdit: boolean;
  draft: boolean;
  refused: WriteRefusal | null;
}

/** The width classes under 1024 px, which read but do not write (Q-021-12). */
const READING_ONLY: ReadonlySet<WidthClass> = new Set(["compact", "narrow"]);

/**
 * The gate of writes (REQ-021-5, 007/W9; REQ-021-29, Q-021-12): nothing under `as_of`, the
 * draft included; nothing under 1024 px either (`width`, the class of the frame), said
 * after the past (widening the window would not open a date gone); a `valid_at` off the
 * present leaves the draft only (its changes take effect at that date). Without `width`,
 * the period alone: what a read that a write follows asks (the simulation of a closure); a
 * surface that writes passes it, by `useWrites`.
 */
export function writesOf(time: Time, width?: WidthClass): Writes {
  if (time.asOf !== null) return { quickEdit: false, draft: false, refused: "past" };
  if (width !== undefined && READING_ONLY.has(width)) {
    return { quickEdit: false, draft: false, refused: "narrow" };
  }
  return { quickEdit: time.present, draft: true, refused: null };
}

/** The gate of writes at the period of the route and the width of the frame, reactive. */
export function useWrites(): ComputedRef<Writes> {
  const time = useTime();
  return computed(() => writesOf(time.value, frameWidth.value));
}

/**
 * The key of each refusal of the gate, whole: the frame says it before any screen, so its
 * words lie in the catalog of the initial shell (`views/i18n/shared/`), which
 * `test/build.test.ts` checks of every key the first load names (task 22n).
 */
const REFUSAL_KEYS: Record<WriteRefusal, string> = {
  past: "edit.why.past",
  narrow: "edit.why.narrow",
};

/**
 * The sentence of a refusal of the gate (`edit.why.past`, `edit.why.narrow`), the one a
 * cell, a key, a command or a panel says; `null` when the draft is open.
 */
export function refusalWords(writes: Writes, t: (key: string) => string): string | null {
  return writes.refused === null ? null : t(REFUSAL_KEYS[writes.refused]);
}

/**
 * The reasons a write is refused for, by rank (task 021-21h, REQ-021-37): `lasting`, what
 * neither a date nor a width changes — a permission lacking, an object delivered, a scope
 * wider, nothing for it to act in —; `state`, what the object or the draft holds now —
 * already in the draft, its author only, held by a policy, a draft empty or in error —;
 * `gate`, the gate of writes (`writesOf`: the past before the width).
 */
export interface RankedReasons<T> {
  lasting?: T | null | undefined;
  state?: T | null | undefined;
  gate?: T | null | undefined;
}

/**
 * The one rank of every refusal of a write, wherever it is said — a button, a cell, a key,
 * a command, a dialog: the most lasting reason first, so that the user reads what widening
 * the window or coming back to the present would not change. Only the sentence depends on
 * it: the action is refused whenever any of the reasons is given, whatever their order.
 */
export function rankedRefusal<T>(reasons: RankedReasons<T>): T | undefined {
  return reasons.lasting ?? reasons.state ?? reasons.gate ?? undefined;
}

/** The refusal of a date by the API (a 400): the field it is about, and its problem. */
export interface PeriodRefusal {
  field: keyof Period;
  problem: ApiProblem;
}

/**
 * What the period picker shows of a refusal, by `problems/` (REQ-021-20): on its field, an
 * `as_of` past the time of the base in its own words, else the sentence of the problem's
 * type; its codes under the fields (`DatePill` `problem`). A new object each time.
 */
export function refusalOnField(
  refusal: PeriodRefusal,
  t: (key: string, params?: Record<string, unknown>) => string,
  sentence: (error: unknown) => string,
): Partial<Record<keyof Period, string>> {
  const detail = refusal.problem.detail ?? refusal.problem.title;
  const words =
    refusal.field === "asOf" && /later than the time/.test(detail)
      ? t("time.asOfLater")
      : t("time.refused", { why: sentence(refusal.problem) });
  return { [refusal.field]: words };
}

/** What the check reads: a dated read of the API by URL, its problem thrown. */
export type Probe = (url: string) => Promise<unknown>;

const probe: Probe = (url) => api.request("GET", url);

/** The dated lists a read right opens, the lightest first: where a period is asked. */
const PROBES: readonly (readonly [permission: string, path: string])[] = [
  ["core.domain.read", "/v1/domains"],
  ["core.entity.read", "/v1/entities"],
  ["core.site.read", "/v1/sites"],
  ["core.vlan_group.read", "/v1/vlan-groups"],
  ["core.vlan.read", "/v1/vlans"],
  ["core.prefix.read", "/v1/prefixes"],
  ["core.range.read", "/v1/ranges"],
  ["core.address.read", "/v1/addresses"],
];

/** The dated list to ask a period of, among those `has` opens; `null` if none. */
export function probePath(has: (permission: string) => boolean): string | null {
  return PROBES.find(([permission]) => has(permission))?.[1] ?? null;
}

/**
 * Asks the API whether it reads at `period` before the URL takes it, so that its refusal
 * comes back on the field of the date pill (020, `DatePill` `errors`): the browser's
 * clock refuses an `as_of` it sees in the future, the database's `now()` may still be
 * earlier (`as_of is later than the time of kartenn`). One row of `path`, a dated list
 * the user may read (`probePath`); without one, nothing is asked and the period goes
 * through (a screen shows the refusal of its own reads). Any answer but a 400 lets it
 * through. At the present, nothing to ask.
 */
export async function checkPeriod(
  period: Period,
  path: string | null,
  get: Probe = probe,
): Promise<PeriodRefusal | null> {
  if ((!period.validAt && !period.asOf) || path === null) return null;
  const search = new URLSearchParams({ page_size: "1" });
  if (period.validAt) search.set("valid_at", period.validAt);
  if (period.asOf) search.set("as_of", period.asOf);
  try {
    await get(`${path}?${search}`);
    return null;
  } catch (error) {
    if (!(error instanceof ApiProblem) || error.status !== 400) return null;
    const reason = error.detail ?? error.title;
    const field: keyof Period = period.asOf && /as_of/.test(reason) ? "asOf" : "validAt";
    return { field, problem: error };
  }
}
