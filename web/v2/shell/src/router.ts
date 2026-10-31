import type { RouteRecordRaw } from "vue-router";
import { frameRoutes } from "./frame/routes.ts";
// In the first chunk: the frame replaces the skeleton of index.html without waiting for one
// more file.
import ShellFrame from "./frame/ShellFrame.vue";

/**
 * The routes of the new shell: the frame (spec 021, REQ-021-1) and its screens; outside
 * production, the catalogue /_ui (REQ-020-24), out of the frame, a separate chunk:
 * `import.meta.env.MODE` is a constant of the build, so the production build drops the
 * branch and never emits the chunk.
 */
export async function shellRoutes(): Promise<RouteRecordRaw[]> {
  const routes: RouteRecordRaw[] = [
    {
      path: "/",
      component: ShellFrame,
      meta: { frame: true },
      children: frameRoutes,
    },
  ];
  if (import.meta.env.MODE !== "production") {
    routes.push(...(await import("./_ui/routes.ts")).catalogueRoutes);
  }
  return routes;
}
