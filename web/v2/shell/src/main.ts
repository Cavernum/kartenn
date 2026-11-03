import { onUnauthorized, reserveKeys, setFetch } from "@kartenn/sdk/host";
import { createApp, watch } from "vue";
import { createRouter, createWebHistory } from "vue-router";
import "./shared/ui.ts";
import App from "./App.vue";
import { installData } from "./data/query.ts";
import { installSession } from "./frame/session.ts";
import { useSettings } from "./frame/settings.ts";
import { createShellI18n, LOCALES, mergePluginCatalog } from "./i18n/index.ts";
import { beginPlugins, configurePlugins } from "./plugins/loader.ts";
import { installPrefetch } from "./prefetch/intent.ts";
import { installViewMark } from "./prefetch/viewMark.ts";
import { useSession } from "./rights/index.ts";
import { shellRoutes } from "./router.ts";
import { expired } from "./session/index.ts";
import { activityFetch, installGestures, markGesture } from "./session/expiry.ts";
import { SHELL_KEYS } from "./shortcuts/shell.ts";
import { installTime } from "./time/index.ts";

// The new shell (spec 021): its frame and screens and, outside production, the catalogue of
// the design system (spec 020, REQ-020-24). Guards, in this order: the session (the first
// page of the frame waits for `GET /v1/me`), the time (the dates of the page left carried
// into the target), then the prefetch (`beforeResolve`: the reads of the target, 100 ms at
// most, REQ-021-22). The settings of the user apply before the first render, and the
// skeleton of index.html stays until the first page is ready (REQ-021-1).
async function start(): Promise<void> {
  // The shell's keys, refused to the plugins' shortcuts before any plugin registers one.
  reserveKeys(SHELL_KEYS);
  // Each answer of the API sees the session again (its end said before it comes); a 401
  // while signed in offers to log in again (« Session expirée »), never on the login page.
  // Only the reads the user asks for (a gesture just before) are activity, not those of the
  // background (an event of the stream, a revalidation, the plugins when idle).
  setFetch(activityFetch());
  installGestures();
  onUnauthorized(() => {
    if (useSession().value.me) expired.value = true;
  });
  useSettings();
  const i18n = createShellI18n();
  const router = createRouter({ history: createWebHistory(), routes: await shellRoutes() });
  installSession(router);
  installTime(router);
  installPrefetch(router);
  installViewMark(router);
  // A navigation is a gesture: the reads of its screen are the user's.
  router.beforeEach(() => markGesture());
  const app = createApp(App).use(i18n).use(router);
  installData(app);
  await router.isReady();
  app.mount("#app");
  // After the first screen of a session, in an idle moment: the plugins (task 16), never
  // before the keys of the shell are reserved (above).
  configurePlugins(LOCALES, (locale, name, messages) =>
    mergePluginCatalog(i18n, locale, name, messages),
  );
  watch(
    () => useSession().value.me,
    (me) => me && beginPlugins(),
    { immediate: true },
  );
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
