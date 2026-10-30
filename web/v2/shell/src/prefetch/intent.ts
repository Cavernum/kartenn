import { context, prefetch, type Prefetchable, type TimeArg } from "@kartenn/sdk";
import type { LocationQuery, RouteLocationNormalized, RouteParams, Router } from "vue-router";
import { preloadModules, type PluginModules } from "../plugins/preload.ts";

// Prefetch on intent, the shell's part (REQ-021-22; design « Préchargement », « Routes »):
// each route declares the reads of its screen; a link reads them on intent (`KLink`), the
// guard of the router waits for them 100 ms at most, then the screen shows the cache or its
// skeleton. The SDK holds the delays (`prefetch.intent`, `prefetch.guard`).

declare module "vue-router" {
  interface RouteMeta {
    /**
     * The reads of the screen, for the dated context of the target (its `valid_at` and
     * `as_of`, else the current one) and its query (the filters of a list).
     */
    queries?: (params: RouteParams, time: TimeArg, query: LocationQuery) => readonly Prefetchable[];
    /** The modules of the plugin whose view the route opens, preloaded on intent. */
    modules?: (params: RouteParams) => PluginModules | null;
  }
}

/** What the prefetch reads of a target: a resolved location, or the route of a guard. */
export type Target = Pick<RouteLocationNormalized, "matched" | "params" | "query">;

const instant = (value: LocationQuery[string] | undefined) =>
  typeof value === "string" && value !== "" ? value : null;

/**
 * The dated context of a target: its own when its query names a date, else the current
 * one, which the navigation carries (W9, 007 `carryTime`).
 */
export function timeOf(query: LocationQuery): TimeArg {
  if ("valid_at" in query || "as_of" in query) {
    return { validAt: instant(query.valid_at), asOf: instant(query.as_of) };
  }
  const { validAt, asOf } = context.get().time;
  return { validAt, asOf };
}

/** The reads that the routes of a target declare; a route that throws declares none. */
export function queriesOf(target: Target): Prefetchable[] {
  const time = timeOf(target.query);
  return target.matched.flatMap((record) => {
    try {
      return [...(record.meta.queries?.(target.params, time, target.query) ?? [])];
    } catch (error) {
      console.warn(`kartenn: the reads of ${record.path} could not be declared`, error);
      return [];
    }
  });
}

/** The intent of a link: the modules of a plugin inserted, the reads of the target returned. */
export function intentOf(target: Target): Prefetchable[] {
  for (const record of target.matched) {
    const modules = record.meta.modules?.(target.params);
    // A module refused or missing shows when the view opens, set aside by the loader.
    if (modules) preloadModules(modules).catch(() => {});
  }
  return queriesOf(target);
}

/**
 * The guard of navigation: the reads of the target, awaited 100 ms at most; a route without
 * reads goes on at once. A refused read (403) is not an error of the navigation: the
 * screen shows it.
 */
export function installPrefetch(router: Router, guard = prefetch.guard): void {
  router.beforeResolve(async (to) => {
    const queries = queriesOf(to);
    if (queries.length > 0) await guard(queries);
  });
}
