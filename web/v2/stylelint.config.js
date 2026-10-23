// Style lint of the new shell, its library and its plugins (spec 020, design « Architecture
// CSS »; REQ-020-12, 13, 14): every value comes from a token `--k-…`, defined in the token
// files of @kartenn/ui only; every rule sits in a known layer.

/** Components whose sheets may cast the shadow of a floating layer (REQ-020-9). */
export const FLOATING = [
  "Menu",
  "Popover",
  "Tooltip",
  "Select",
  "Combobox",
  "FilterPicker",
  "PeriodPicker",
  "Dialog",
  "AlertDialog",
  "Drawer",
  "Toast",
  "CommandPalette",
];

/** Properties whose value must be a token (design « Architecture CSS »). */
const STRICT = [
  "/color$/",
  "fill",
  "stroke",
  "background",
  "font",
  "font-family",
  "font-size",
  "font-weight",
  "line-height",
  "letter-spacing",
  "border-radius",
  "/^border-.*-radius$/",
  "/^border(-(block|inline)(-(start|end))?)?-width$/",
  "outline-width",
  "outline-offset",
  "box-shadow",
  "/^transition(-(duration|delay|timing-function))?$/",
  "/^animation(-(duration|delay|timing-function))?$/",
  "z-index",
  "/gap$/",
  "/^margin/",
  "/^padding/",
  "/^inset/",
  "/^(min-|max-)?(width|height|inline-size|block-size)$/",
  "/^(min-|max-)?(inline|block)-size$/",
  "flex-basis",
];

export default {
  plugins: [
    "stylelint-declaration-strict-value",
    "stylelint-use-logical",
    "./tools/stylelint/plugins.js",
  ],
  ignoreFiles: ["**/dist/**", "**/node_modules/**", "**/test-results/**"],
  rules: {
    "kartenn/known-layer": true,
    "kartenn/token-reference": true,
    "kartenn/raw-length": true,
    "kartenn/floating-shadow": [true, { floating: FLOATING }],
    "scale-unlimited/declaration-strict-value": [
      STRICT,
      {
        // Functions (calc(), min()…) are checked by kartenn/raw-length and the colour rules.
        expandShorthand: true,
        recurseLonghand: true,
        ignoreValues: {
          "": [
            "0",
            "inherit",
            "initial",
            "unset",
            "revert",
            "revert-layer",
            "currentcolor",
            "currentColor",
            "transparent",
            "none",
            "auto",
            "/^\\d+(\\.\\d+)?%$/",
            // Border widths of 1px: kartenn/raw-length refuses it elsewhere.
            "1px",
            "fit-content",
            "min-content",
            "max-content",
          ],
        },
        message: 'raw value "${value}" for "${property}": use a token var(--k-…) (REQ-020-13)',
      },
    ],
    "color-no-hex": true,
    "color-named": "never",
    "function-disallowed-list": [
      "rgb",
      "rgba",
      "hsl",
      "hsla",
      "hwb",
      "lab",
      "lch",
      "oklab",
      "oklch",
      "color",
      "color-mix",
    ],
    "declaration-no-important": true,
    "selector-max-id": 0,
    "selector-max-compound-selectors": 3,
    "csstools/use-logical": "always",
  },
  overrides: [
    {
      files: ["**/*.vue"],
      customSyntax: "postcss-html",
    },
    {
      // The token files: the only place of raw values (REQ-020-4).
      files: ["ui/src/tokens/**/*.css"],
      rules: {
        "kartenn/token-reference": null,
        "kartenn/raw-length": null,
        "scale-unlimited/declaration-strict-value": null,
        "color-no-hex": null,
        "color-named": null,
        "function-disallowed-list": null,
      },
    },
  ],
};
