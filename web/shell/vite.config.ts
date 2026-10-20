import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [vue()],
  define: {
    // vue-i18n (W11): composition API only; messages compiled to an AST at run time (JIT,
    // no eval nor new Function, CSP without 'unsafe-eval'), so the message compiler stays
    // in the build for the catalogs of plugins (design « Risques », vue-i18n and CSP).
    __VUE_I18N_FULL_INSTALL__: true,
    __VUE_I18N_LEGACY_API__: false,
    __INTLIFY_PROD_DEVTOOLS__: false,
    __INTLIFY_JIT_COMPILATION__: true,
    __INTLIFY_DROP_MESSAGE_COMPILER__: false,
  },
  build: {
    target: "es2022",
    // Served by kartenn-server from `[web] root` (design « Arborescence de web/ »).
    outDir: "../dist/shell",
    emptyOutDir: true,
  },
  test: {
    name: "shell",
    environment: "node",
  },
});
