import { createApp } from "vue";
import { createWebHistory } from "vue-router";
import App from "./App.vue";
import { createShellRouter } from "./router.ts";

createApp(App).use(createShellRouter(createWebHistory())).mount("#app");
