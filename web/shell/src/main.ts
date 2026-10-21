import { createApp } from "vue";
import { createWebHistory } from "vue-router";
import App from "./App.vue";
import { createShellI18n, LOCALES } from "./i18n/index.ts";
import { loadPlugins, readPlugins } from "./plugins/loader.ts";
import { loadSession } from "./rights/index.ts";
import { createShellRouter } from "./router.ts";
import { applyTheme, storedTheme } from "./theme.ts";
import "./styles/tokens.css";
import "./styles/base.css";

applyTheme(storedTheme());
const i18n = createShellI18n();
createApp(App).use(i18n).use(createShellRouter(createWebHistory())).mount("#app");
void loadSession();
// The first screen does not wait for the plugins (REQ-007-9).
void readPlugins().then((plugins) =>
  loadPlugins(plugins, LOCALES, (locale, name, messages) =>
    i18n.global.mergeLocaleMessage(locale, { [name]: messages }),
  ),
);
