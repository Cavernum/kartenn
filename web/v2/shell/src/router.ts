import type { RouteRecordRaw } from "vue-router";

/**
 * The routes of the new shell. The catalogue /_ui (REQ-020-24) is a separate chunk, loaded
 * in every mode but production: `import.meta.env.MODE` is a constant of the build, so the
 * production build drops the branch and never emits the chunk.
 */
export async function shellRoutes(): Promise<RouteRecordRaw[]> {
  const routes: RouteRecordRaw[] = [
    { path: "/", name: "home", component: () => import("./HomePage.vue") },
  ];
  if (import.meta.env.MODE !== "production") {
    routes.push(...(await import("./_ui/routes.ts")).catalogueRoutes);
  }
  return routes;
}
