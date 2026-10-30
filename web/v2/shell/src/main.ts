import { createApp } from "vue";
import { createRouter, createWebHistory } from "vue-router";
import "./shared/ui.ts";
import App from "./App.vue";
import { installPrefetch } from "./prefetch/intent.ts";
import { shellRoutes } from "./router.ts";

// The new shell (spec 021): for now its frame and, outside production, the catalogue of
// the design system (spec 020, REQ-020-24). Navigation waits for the reads its routes
// declare, 100 ms at most (REQ-021-22).
const router = createRouter({ history: createWebHistory(), routes: await shellRoutes() });
installPrefetch(router);
createApp(App).use(router).mount("#app");
