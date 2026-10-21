import { createRouter, type Router, type RouterHistory } from "vue-router";

/** The routes of the client; history injected so that tests run without a browser. */
export function createShellRouter(history: RouterHistory): Router {
  const router = createRouter({
    history,
    routes: [
      { path: "/", name: "home", component: () => import("./views/HomeView.vue") },
      { path: "/admin", name: "admin", component: () => import("./views/AdminView.vue") },
      { path: "/about", name: "about", component: () => import("./views/AboutView.vue") },
      {
        path: "/:collection(entities|sites|domains|prefixes|ranges|addresses)/:id",
        name: "object",
        component: () => import("./views/ObjectView.vue"),
      },
      {
        path: "/x/:plugin/:id/:rest(.*)*",
        name: "plugin-view",
        component: () => import("./plugins/PluginView.vue"),
      },
      {
        path: "/:missing(.*)*",
        name: "not-found",
        component: () => import("./views/NotFoundView.vue"),
      },
    ],
  });
  // A navigation that fails, a lazy view whose loading the browser dropped because the
  // page is leaving included: reported, without the uncaught error of vue-router.
  router.onError((error, to) => {
    console.warn(`kartenn: navigation to ${to.fullPath} failed`, error);
  });
  return router;
}
