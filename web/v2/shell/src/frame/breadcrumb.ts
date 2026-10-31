import type { Crumb } from "@kartenn/ui";
import type { RouteLocationNormalizedLoaded } from "vue-router";

/**
 * The breadcrumb of a route (REQ-021-4): that of its deepest record that declares one
 * (`meta.breadcrumb`, prefetch/intent.ts); every crumb but the last a link, the last never.
 */
export function crumbsOf(
  route: Pick<RouteLocationNormalizedLoaded, "matched" | "params">,
  t: (key: string) => string,
): Crumb[] {
  const record = [...route.matched].reverse().find((r) => r.meta.breadcrumb);
  const crumbs = record?.meta.breadcrumb?.(route.params, t) ?? [];
  return crumbs.map((crumb, i) =>
    i < crumbs.length - 1 ? crumb : { label: crumb.label, ...(crumb.mono ? { mono: true } : {}) },
  );
}
