import { fileURLToPath } from "node:url";
import stylelint from "stylelint";
import { describe, expect, it } from "vitest";
import config from "../../stylelint.config.js";

// The style lint (spec 020, task 3; REQ-020-12, 13): forbidden cases fail, their corrected
// form passes.

const root = fileURLToPath(new URL("../..", import.meta.url));

async function rules(code: string, file = "ui/src/components/Button/Button.css") {
  const { results } = await stylelint.lint({
    code,
    codeFilename: `${root}${file}`,
    config,
    configBasedir: root,
  });
  return results[0]!.warnings.map((w) => (process.env.DEBUG_LINT ? w.text : w.rule)).sort();
}

const layer = (body: string) => `@layer components {\n  .k-button {\n    ${body}\n  }\n}\n`;

describe("stylelint", () => {
  it("accepts a sheet made of tokens", async () => {
    const sheet = layer(
      [
        "padding: var(--k-space-1) var(--k-space-2);",
        "margin-inline: calc(var(--k-space-2) * -1);",
        "border: 1px solid var(--k-border-control);",
        "border-radius: var(--k-radius-md);",
        "color: var(--k-text);",
        "background-color: transparent;",
        "inline-size: 100%;",
        "max-inline-size: calc(100% - var(--k-space-4));",
        "font: inherit;",
        "font-variant-numeric: tabular-nums;",
        "transition: background-color var(--k-duration-short) var(--k-ease-standard);",
        "box-shadow: var(--k-shadow-inset-accent);",
        "--k-button-bg: var(--k-surface);",
      ].join("\n    "),
    );
    expect(await rules(sheet)).toEqual([]);
  });

  it.each([
    ["color: #fff;", ["color-no-hex", "scale-unlimited/declaration-strict-value"]],
    ["color: red;", ["color-named", "scale-unlimited/declaration-strict-value"]],
    ["background-color: rgb(0 0 0);", ["function-disallowed-list"]],
    ["color: color-mix(in oklab, var(--k-text) 50%, var(--k-bg));", ["function-disallowed-list"]],
    ["padding: 4px;", ["kartenn/raw-length", "scale-unlimited/declaration-strict-value"]],
    ["max-inline-size: calc(100% - 8px);", ["kartenn/raw-length"]],
    ["z-index: 10;", ["scale-unlimited/declaration-strict-value"]],
    ["font-family: Arial;", ["scale-unlimited/declaration-strict-value"]],
    ["line-height: 1.5;", ["scale-unlimited/declaration-strict-value"]],
    [
      "transition: opacity 120ms;",
      ["kartenn/raw-length", "scale-unlimited/declaration-strict-value"],
    ],
    ["color: var(--private);", ["kartenn/token-reference"]],
    ["--k-button-gap: 10px;", ["kartenn/token-reference"]],
    ["color: var(--k-text) !important;", ["declaration-no-important"]],
    ["margin-left: var(--k-space-1);", ["csstools/use-logical"]],
    ["box-shadow: var(--k-shadow-overlay);", ["kartenn/floating-shadow"]],
  ])("refuses « %s »", async (declaration, expected) => {
    expect(await rules(layer(declaration))).toEqual(expected);
  });

  it("refuses an id, a deep selector, a rule outside a layer, an unknown layer", async () => {
    expect(
      await rules(
        [
          "@layer components { #main { color: var(--k-text); } }",
          "@layer components { .a .b .c .d { color: var(--k-text); } }",
          ".k-loose { color: var(--k-text); }",
          "@layer theme { .k-x { color: var(--k-text); } }",
        ].join("\n"),
        // Outside @kartenn/ui: kartenn/component-prefix aside.
        "shell/src/views/View.css",
      ),
    ).toEqual([
      "kartenn/known-layer",
      "kartenn/known-layer",
      "selector-max-compound-selectors",
      "selector-max-id",
    ]);
  });

  it("lets a keyframe turn, nowhere else", async () => {
    const spin =
      "@layer components {\n  @keyframes k-button-spin {\n    to {\n      transform: rotate(1turn);\n    }\n  }\n}\n";
    expect(await rules(spin)).toEqual([]);
    expect(await rules(layer("transform: rotate(1turn);"))).toEqual(["kartenn/raw-length"]);
  });

  it("lets the floating components cast their shadow", async () => {
    const sheet = layer("box-shadow: var(--k-shadow-overlay);").replace(".k-button", ".k-popover");
    expect(await rules(sheet, "ui/src/components/Popover/Popover.css")).toEqual([]);
  });

  // kartenn/component-prefix (spec 029, task 2): the one sheet of @kartenn/ui matches only
  // what each component renders.
  it("keeps the rules of a component to the classes it owns", async () => {
    const sheet = (selector: string) =>
      `@layer components {\n  ${selector} {\n    color: var(--k-text);\n  }\n}\n`;
    const grid = "ui/src/components/DataGrid/DataGrid.vue";
    const sfc = (selector: string) =>
      `<template><div /></template>\n<style>\n${sheet(selector)}</style>\n`;
    for (const ok of [
      ".k-data-grid",
      ".k-data-grid__cell:hover",
      ".k-data-grid--dense",
      ".k-data-grid-frame > .k-column-chooser",
      ".k-cell-error[hidden]",
      ".k-data-grid :focus-visible",
      ".k-data-grid, .k-data-grid-frame",
    ])
      expect(await rules(sfc(ok), grid), ok).toEqual([]);
    for (const ko of [
      ".k-button",
      "button",
      ":root",
      ".k-tree-grid",
      ".k-data-gridx",
      ".k-data-grid, .k-menu",
    ])
      expect(await rules(sfc(ko), grid), ko).toEqual(["kartenn/component-prefix"]);
    // The longest prefix owns a class: k-tree-grid… is TreeGrid's, not Tree's.
    expect(await rules(sheet(".k-tree-grid"), "ui/src/components/Tree/Tree.css")).toEqual([
      "kartenn/component-prefix",
    ]);
    expect(await rules(sheet(".k-tree-frame"), "ui/src/components/Tree/Tree.css")).toEqual([]);
    // Internal parts and demos are units of their own; the shell is not concerned.
    expect(
      await rules(sfc(".k-column-chooser"), "ui/src/components/internal/ColumnChooser.vue"),
    ).toEqual([]);
    expect(
      await rules(sfc(".k-data-grid"), "ui/src/components/internal/ColumnChooser.vue"),
    ).toEqual(["kartenn/component-prefix"]);
    expect(await rules(sfc(".k-demo-box"), "ui/src/demo/DemoBox.vue")).toEqual([]);
    expect(await rules(sfc(".k-demo-frame"), "ui/src/demo/DemoBox.vue")).toEqual([
      "kartenn/component-prefix",
    ]);
    expect(await rules(sheet(".k-anything"), "shell/src/frame/ShellFrame.css")).toEqual([]);
    // The exceptions (prefixes.js), which the impact graph turns into everything.
    expect(await rules(sheet(".k-list"), "ui/src/components/Select/options.css")).toEqual([]);
  });

  it("names the keyframes of a component by one of its prefixes", async () => {
    const frames = (name: string) =>
      `@layer components {\n  @keyframes ${name} {\n    to {\n      opacity: 0;\n    }\n  }\n}\n`;
    expect(await rules(frames("k-row-fresh"), "ui/src/components/DataGrid/DataGrid.css")).toEqual(
      [],
    );
    expect(await rules(frames("k-fade"), "ui/src/components/DataGrid/DataGrid.css")).toEqual([
      "kartenn/component-prefix",
    ]);
  });

  it("lets the token files hold raw values, still in their layer", async () => {
    const tokens =
      "@layer tokens {\n  :root {\n    --k-bg: #f3f5f9;\n    --k-space-1: 4px;\n  }\n}\n";
    expect(await rules(tokens, "ui/src/tokens/tokens.css")).toEqual([]);
    expect(await rules(":root { --k-bg: #f3f5f9; }", "ui/src/tokens/tokens.css")).toEqual([
      "kartenn/known-layer",
    ]);
  });

  it("checks the style blocks of single-file components", async () => {
    const sfc = `<template><div class="k-x" /></template>\n<style>\n${layer("padding: 4px;")}</style>\n`;
    expect(await rules(sfc, "ui/src/components/X/X.vue")).toContain("kartenn/raw-length");
  });
});
