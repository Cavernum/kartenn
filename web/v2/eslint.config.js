import pluginVue from "eslint-plugin-vue";
import { defineConfigWithVueTs, vueTsConfigs } from "@vue/eslint-config-typescript";
import prettier from "eslint-config-prettier";

export default defineConfigWithVueTs(
  {
    ignores: ["**/dist/**", "**/test-results/**", "**/playwright-report/**"],
  },
  pluginVue.configs["flat/recommended"],
  vueTsConfigs.recommended,
  {
    rules: {
      // CSP without 'unsafe-inline' for styles: no static `style` attribute (ADR 0025).
      "vue/no-static-inline-styles": "error",
    },
  },
  prettier,
);
