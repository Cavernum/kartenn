import { createApp } from "vue";
import { createWebHistory } from "vue-router";
import App from "./App.vue";
import { installDraft } from "./draft/index.ts";
import { createShellI18n, LOCALES } from "./i18n/index.ts";
import { startPlugins } from "./plugins/loader.ts";
import { createShellRouter } from "./router.ts";
import { installSession } from "./session/index.ts";
import { applyTheme, storedTheme } from "./theme.ts";
import "./styles/tokens.css";
import "./styles/base.css";

applyTheme(storedTheme());
const i18n = createShellI18n();
const router = createShellRouter(createWebHistory());
// Before the first navigation: it waits for `/v1/me` (REQ-007-28).
installSession(router);
// The draft follows the identity of the session (REQ-007-27).
installDraft();
createApp(App).use(i18n).use(router).mount("#app");
// The first screen does not wait for the plugins (REQ-007-9).
void startPlugins(LOCALES, (locale, name, messages) =>
  i18n.global.mergeLocaleMessage(locale, { [name]: messages }),
);
