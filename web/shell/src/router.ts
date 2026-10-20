import { createRouter, type Router, type RouterHistory } from "vue-router";

/** The routes of the client; history injected so that tests run without a browser. */
export function createShellRouter(history: RouterHistory): Router {
  return createRouter({
    history,
    routes: [
      { path: "/", name: "home", component: () => import("./views/HomeView.vue") },
      { path: "/admin", name: "admin", component: () => import("./views/AdminView.vue") },
      { path: "/about", name: "about", component: () => import("./views/AboutView.vue") },
      {
        path: "/:missing(.*)*",
        name: "not-found",
        component: () => import("./views/NotFoundView.vue"),
      },
    ],
  });
}
