import { createRouter, type Router, type RouteRecordRaw, type RouterHistory } from "vue-router";
import { COLLECTION_PATTERN } from "./data/collections.ts";
import { installTime } from "./time/index.ts";

/** The screens of the administration and of the imports (REQ-007-19). */
const ADMIN_ROUTES: RouteRecordRaw[] = [
  { path: "/admin/roles", component: () => import("./admin/RolesView.vue") },
  { path: "/admin/bindings", component: () => import("./admin/BindingsView.vue") },
  { path: "/admin/issuers", component: () => import("./admin/IssuersView.vue") },
  { path: "/admin/service-accounts", component: () => import("./admin/ServiceAccountsView.vue") },
  {
    path: "/admin/service-accounts/:id",
    component: () => import("./admin/ServiceAccountView.vue"),
  },
  { path: "/admin/identities", component: () => import("./admin/IdentitiesView.vue") },
  { path: "/admin/identities/:id", component: () => import("./admin/IdentityView.vue") },
  { path: "/admin/audit", component: () => import("./admin/AuditView.vue") },
  { path: "/imports", component: () => import("./admin/ImportsView.vue") },
  { path: "/imports/:id", component: () => import("./admin/ImportView.vue") },
];

/** The routes of the client; history injected so that tests run without a browser. */
export function createShellRouter(history: RouterHistory): Router {
  const router = createRouter({
    history,
    routes: [
      { path: "/", name: "home", component: () => import("./views/HomeView.vue") },
      { path: "/admin", name: "admin", component: () => import("./views/AdminView.vue") },
      ...ADMIN_ROUTES,
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
        path: `/:collection(${COLLECTION_PATTERN})/new`,
        name: "new",
        component: () => import("./views/NewView.vue"),
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
