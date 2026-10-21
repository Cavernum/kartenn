import { setContext } from "@kartenn/sdk/host";
import { computed, type ComputedRef } from "vue";
import {
  useRoute,
  useRouter,
  type LocationQuery,
  type RouteLocationNormalized,
  type RouteLocationRaw,
  type Router,
} from "vue-router";

// The dated context (REQ-007-22 to 24, W9): `valid_at` and `as_of` live in the query of
// the URL, so that a link shows what its author saw; a guard carries them along every
// internal navigation, until an explicit return to the present.

export interface Time {
  validAt: string | null;
  asOf: string | null;
  present: boolean;
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

/** `?valid_at=…&as_of=…` for a read of the API in this context, or nothing. */
export function datedSearch(time: Time): string {
  const search = new URLSearchParams();
  if (time.validAt) search.set("valid_at", time.validAt);
  if (time.asOf) search.set("as_of", time.asOf);
  const text = search.toString();
  return text ? `?${text}` : "";
}

let leavingForPresent = false;

/**
 * The guard (design « Contexte temporel »): a navigation that names neither date keeps
 * those of the page it leaves, unless it is the explicit return to the present.
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

/** Back to the present: the same page without the dates. */
export function toPresent(router: Router): Promise<unknown> {
  const route = router.currentRoute.value;
  const query = { ...route.query };
  delete query.valid_at;
  delete query.as_of;
  leavingForPresent = true;
  return router.push({ path: route.path, query, hash: route.hash }).finally(() => {
    leavingForPresent = false;
  });
}

/** The dated context of the current route, reactive. */
export function useTime(): ComputedRef<Time> {
  const route = useRoute();
  return computed(() => timeOf(route.query));
}

/** Moves the current page to other dates (`null` clears one). */
export function useSetTime(): (time: { validAt: string | null; asOf: string | null }) => void {
  const router = useRouter();
  return ({ validAt, asOf }) => {
    const route = router.currentRoute.value;
    const query = { ...route.query };
    delete query.valid_at;
    delete query.as_of;
    if (validAt) query.valid_at = validAt;
    if (asOf) query.as_of = asOf;
    if (!validAt && !asOf) leavingForPresent = true;
    void router.push({ path: route.path, query, hash: route.hash });
  };
}

/**
 * What may be written (REQ-007-23): nothing under `as_of`, the draft included; a
 * `valid_at` off the present leaves the draft only (its changes take effect at that date).
 */
export function writesOf(time: Time): { quickEdit: boolean; draft: boolean } {
  return { quickEdit: time.present, draft: time.asOf === null };
}

export function useWrites(): ComputedRef<{ quickEdit: boolean; draft: boolean }> {
  const time = useTime();
  return computed(() => writesOf(time.value));
}
