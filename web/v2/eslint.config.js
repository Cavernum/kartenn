import pluginVue from "eslint-plugin-vue";
import { defineConfigWithVueTs, vueTsConfigs } from "@vue/eslint-config-typescript";
import prettier from "eslint-config-prettier";

// Lint of the new shell (spec 020, design « Architecture CSS »; REQ-020-13, 14, 33).

/** Reka UI parts that inject a `<style>` element, refused by the CSP (REQ-020-33). */
export const FORBIDDEN_REKA = [
  "ComboboxViewport",
  "SplitterGroup",
  "SplitterPanel",
  "SplitterResizeHandle",
];

/** Raw interactive elements: screens and plugins use the components of @kartenn/ui. */
export const RAW_ELEMENTS = ["button", "input", "select", "textarea", "table"];

// CSP without 'unsafe-inline' for styles (ADR 0025, REQ-020-14): dynamic values go through
// `style.setProperty('--k-…', v)` or an object binding (CSSOM), never through a string.
const STRING_STYLE = (kind) =>
  `VAttribute[directive=true][key.name.name='bind'][key.argument.name='style'] > VExpressionContainer > ${kind}`;
const cspTemplate = ["Literal", "TemplateLiteral", "BinaryExpression[operator='+']"].map(
  (kind) => ({
    selector: STRING_STYLE(kind),
    message: "string style binding: bind an object (CSSOM) or use a class (REQ-020-14)",
  }),
);
const cspScript = [
  {
    selector: "CallExpression[callee.property.name='createElement'][arguments.0.value='style']",
    message: "no injected <style> element: the CSP refuses it (REQ-020-14)",
  },
  {
    selector: "CallExpression[callee.property.name='setAttribute'][arguments.0.value='style']",
    message: "no style attribute: use style.setProperty('--k-…', value) (REQ-020-14)",
  },
];

export default defineConfigWithVueTs(
  {
    ignores: ["**/dist/**", "**/test-results/**", "**/playwright-report/**"],
  },
  pluginVue.configs["flat/recommended"],
  vueTsConfigs.recommended,
  {
    rules: {
      "vue/no-static-inline-styles": ["error", { allowBinding: false }],
      "vue/no-v-html": "error",
      "vue/no-restricted-syntax": ["error", ...cspTemplate],
      "no-restricted-syntax": ["error", ...cspScript],
      "no-restricted-properties": [
        "error",
        { property: "cssText", message: "no cssText: use style.setProperty (REQ-020-14)" },
      ],
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "reka-ui",
              importNames: FORBIDDEN_REKA,
              message: "injects a <style> element, refused by the CSP (REQ-020-33)",
            },
          ],
        },
      ],
    },
  },
  {
    // The components of the library bear the names of the inventory (design « Inventaire
    // des composants »): `Icon`, `Button`, `Tabs`…
    files: ["ui/src/components/**/*.vue"],
    rules: { "vue/multi-word-component-names": "off" },
  },
  {
    // Screens and plugins: @kartenn/ui only, neither raw controls nor Reka UI directly. The
    // catalogue (_ui) shows the library and may use raw elements to frame it.
    files: ["shell/src/**/*.{vue,ts}", "plugins/*/src/**/*.{vue,ts}"],
    ignores: ["shell/src/_ui/**"],
    rules: {
      "vue/no-restricted-html-elements": [
        "error",
        ...RAW_ELEMENTS.map((element) => ({
          element,
          message: `raw <${element}>: use the component of @kartenn/ui (REQ-020-13)`,
        })),
      ],
      "no-restricted-imports": [
        "error",
        {
          paths: [
            {
              name: "reka-ui",
              message: "screens and plugins use @kartenn/ui, which wraps Reka UI (REQ-020-33)",
            },
          ],
        },
      ],
    },
  },
  prettier,
);
