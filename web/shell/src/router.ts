import { createRouter, type Router, type RouterHistory } from "vue-router";

/** The routes of the client; history injected so that tests run without a browser. */
export function createShellRouter(history: RouterHistory): Router {
  return createRouter({
    history,
    routes: [{ path: "/", name: "home", component: () => import("./views/HomeView.vue") }],
  });
}
