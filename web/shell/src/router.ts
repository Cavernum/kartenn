import { createRouter, type Router, type RouterHistory } from "vue-router";
import { COLLECTION_PATTERN } from "./data/collections.ts";
import { installTime } from "./time/index.ts";

/** The routes of the client; history injected so that tests run without a browser. */
export function createShellRouter(history: RouterHistory): Router {
  const router = createRouter({
    history,
    routes: [
      { path: "/", name: "home", component: () => import("./views/HomeView.vue") },
      { path: "/admin", name: "admin", component: () => import("./views/AdminView.vue") },
      { path: "/about", name: "about", component: () => import("./views/AboutView.vue") },
      { path: "/login", name: "login", component: () => import("./views/LoginView.vue") },
      { path: "/search", name: "search", component: () => import("./views/SearchView.vue") },
      {
        path: "/anomalies",
        name: "anomalies",
        component: () => import("./views/AnomaliesView.vue"),
      },
      {
        path: "/properties",
        name: "properties",
        component: () => import("./views/PropertiesView.vue"),
      },
      {
        path: "/changesets",
        name: "changesets",
        component: () => import("./changesets/ChangesetsView.vue"),
      },
      {
        path: "/changesets/:id",
        name: "changeset",
        component: () => import("./changesets/ChangesetView.vue"),
      },
      {
        path: "/proposals",
        name: "proposals",
        component: () => import("./changesets/ProposalsView.vue"),
      },
      {
        path: `/:collection(${COLLECTION_PATTERN})`,
        name: "list",
        component: () => import("./views/ListView.vue"),
      },
      {
        path: `/:collection(${COLLECTION_PATTERN})/:id`,
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
  installTime(router);
  return router;
}
