import { createApp } from "vue";
import { createWebHistory } from "vue-router";
import App from "./App.vue";
import { createShellI18n } from "./i18n/index.ts";
import { loadSession } from "./rights/index.ts";
import { createShellRouter } from "./router.ts";
import { applyTheme, storedTheme } from "./theme.ts";
import "./styles/tokens.css";
import "./styles/base.css";

applyTheme(storedTheme());
createApp(App).use(createShellI18n()).use(createShellRouter(createWebHistory())).mount("#app");
void loadSession();
