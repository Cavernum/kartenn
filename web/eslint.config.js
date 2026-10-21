import pluginVue from "eslint-plugin-vue";
import { defineConfigWithVueTs, vueTsConfigs } from "@vue/eslint-config-typescript";
import prettier from "eslint-config-prettier";

export default defineConfigWithVueTs(
  {
    // e2e/.state: generated material of the journeys, built plugins included.
    ignores: ["**/dist/**", "**/test-results/**", "**/playwright-report/**", "e2e/.state/**"],
  },
  pluginVue.configs["flat/recommended"],
  vueTsConfigs.recommended,
  {
    rules: {
      // CSP without 'unsafe-inline' for styles: no static `style` attribute (design « Thème »).
      "vue/no-static-inline-styles": "error",
    },
  },
  prettier,
);
