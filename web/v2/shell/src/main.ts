import { createApp } from "vue";
import { createRouter, createWebHistory } from "vue-router";
import "@kartenn/ui/style.css";
import App from "./App.vue";
import { shellRoutes } from "./router.ts";

// The new shell (spec 021): for now its frame and, outside production, the catalogue of
// the design system (spec 020, REQ-020-24).
const router = createRouter({ history: createWebHistory(), routes: await shellRoutes() });
createApp(App).use(router).mount("#app");
