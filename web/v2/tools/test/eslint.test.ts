import { fileURLToPath } from "node:url";
import { ESLint } from "eslint";
import { describe, expect, it } from "vitest";

// The code lint of the new shell (spec 020, task 3; REQ-020-13, 14, 33): forbidden cases
// fail, their corrected form passes.

const root = fileURLToPath(new URL("../..", import.meta.url));
const eslint = new ESLint({ cwd: root });

async function rules(code: string, file: string): Promise<string[]> {
  const [result] = await eslint.lintText(code, { filePath: `${root}${file}` });
  return result!.messages.map((m) => m.ruleId ?? m.message).sort();
}

const sfc = (template: string, script = "") =>
  `<script setup lang="ts">\n${script}</script>\n\n<template>\n  ${template}\n</template>\n`;

describe("eslint", () => {
  it("refuses raw interactive elements in screens and plugins", async () => {
    for (const element of ["button", "input", "select", "textarea", "table"]) {
      const template = element === "input" ? "<input />" : `<${element}></${element}>`;
      expect(await rules(sfc(template), "shell/src/views/PrefixList.vue"), element).toEqual([
        "vue/no-restricted-html-elements",
      ]);
    }
    expect(await rules(sfc("<button></button>"), "plugins/racks/src/RackList.vue")).toEqual([
      "vue/no-restricted-html-elements",
    ]);
  });

  it("lets the library and the catalogue use raw elements", async () => {
    expect(
      await rules(sfc("<button></button>"), "ui/src/components/IconButton/IconButton.vue"),
    ).toEqual([]);
    expect(await rules(sfc("<table></table>"), "shell/src/_ui/TokensPage.vue")).toEqual([]);
  });

  it("refuses inline styles as strings, static styles and v-html", async () => {
    const file = "ui/src/components/CodeBadge/CodeBadge.vue";
    expect(await rules(sfc('<div style="color: red" />'), file)).toEqual([
      "vue/no-static-inline-styles",
    ]);
    expect(await rules(sfc(`<div :style="'color: ' + c" />`, "const c = 'red';\n"), file)).toEqual([
      "vue/no-restricted-syntax",
    ]);
    expect(
      await rules(sfc('<div :style="`inline-size: ${w}px`" />', "const w = 1;\n"), file),
    ).toEqual(["vue/no-restricted-syntax"]);
    expect(await rules(sfc('<div v-html="h" />', "const h = '';\n"), file)).toEqual([
      "vue/no-v-html",
    ]);
    // The accepted form: an object bound to a token (CSSOM).
    expect(
      await rules(sfc(`<div :style="{ '--k-x-offset': y }" />`, "const y = '1px';\n"), file),
    ).toEqual([]);
  });

  it("refuses cssText, injected <style> and the style attribute in scripts", async () => {
    const file = "ui/src/x.ts";
    expect(await rules("document.body.style.cssText = '';\n", file)).toEqual([
      "no-restricted-properties",
    ]);
    expect(await rules("document.createElement('style');\n", file)).toEqual([
      "no-restricted-syntax",
    ]);
    expect(await rules("document.body.setAttribute('style', '');\n", file)).toEqual([
      "no-restricted-syntax",
    ]);
    expect(await rules("document.body.style.setProperty('--k-x', '1px');\n", file)).toEqual([]);
  });

  it("refuses the Reka UI parts that inject a <style>, and Reka UI in screens", async () => {
    const forbidden =
      "import { ComboboxViewport, SplitterGroup } from 'reka-ui';\nexport { ComboboxViewport, SplitterGroup };\n";
    expect(await rules(forbidden, "ui/src/x.ts")).toEqual([
      "no-restricted-imports",
      "no-restricted-imports",
    ]);
    const allowed = "import { ComboboxRoot } from 'reka-ui';\nexport { ComboboxRoot };\n";
    expect(await rules(allowed, "ui/src/x.ts")).toEqual([]);
    expect(await rules(allowed, "shell/src/views/x.ts")).toEqual(["no-restricted-imports"]);
  });
});
