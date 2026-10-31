import { createApp } from "vue";
import { createRouter, createWebHistory } from "vue-router";
import "./shared/ui.ts";
import App from "./App.vue";
import { installData } from "./data/query.ts";
import { installSession } from "./frame/session.ts";
import { useSettings } from "./frame/settings.ts";
import { createShellI18n } from "./i18n/index.ts";
import { installPrefetch } from "./prefetch/intent.ts";
import { shellRoutes } from "./router.ts";
import { installTime } from "./time/index.ts";

// The new shell (spec 021): its frame and screens and, outside production, the catalogue of
// the design system (spec 020, REQ-020-24). Guards, in this order: the session (the first
// page of the frame waits for `GET /v1/me`), the time (the dates of the page left carried
// into the target), then the prefetch (`beforeResolve`: the reads of the target, 100 ms at
// most, REQ-021-22). The settings of the user apply before the first render, and the
// skeleton of index.html stays until the first page is ready (REQ-021-1).
async function start(): Promise<void> {
  useSettings();
  const i18n = createShellI18n();
  const router = createRouter({ history: createWebHistory(), routes: await shellRoutes() });
  installSession(router);
  installTime(router);
  installPrefetch(router);
  const app = createApp(App).use(i18n).use(router);
  installData(app);
  await router.isReady();
  app.mount("#app");
}

// A start that fails says so over the skeleton, never a page that loads for ever.
start().catch((error: unknown) => {
  const status = document.getElementById("k-boot-status");
  if (status) {
    status.className = "k-boot__failed";
    status.textContent = `kartenn n'a pas pu démarrer (${String(error)}). Rechargez la page.`;
  }
  throw error;
});
