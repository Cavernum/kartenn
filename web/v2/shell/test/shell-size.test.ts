import { describe, expect, it } from "vitest";
import { initialFiles, SHELL_BUDGET, shellMeasure } from "../scripts/shell-size.ts";
import { report, type Manifest } from "../scripts/size.ts";

// The weight of the initial shell (spec 021, REQ-021-30; scripts/shell-size.ts): the
// document's entry and the shared modules, with their static imports and sheets, each file
// once; a screen read by its route left out.

const manifest: Manifest = {
  "index.html": {
    file: "assets/index.js",
    src: "index.html",
    isEntry: true,
    imports: ["_vue.runtime.js", "_ui.js"],
    css: ["assets/index.css"],
  },
  "src/shared/ui.ts": { file: "assets/ui-entry.js", isEntry: true, imports: ["_ui.js"] },
  "../sdk/src/index.ts": { file: "assets/sdk.js", isEntry: true, imports: ["_vue.runtime.js"] },
  "_ui.js": { file: "assets/ui.js", imports: ["_vue.runtime.js"], css: ["assets/ui.css"] },
  "_vue.runtime.js": { file: "assets/vue.runtime.js" },
  "src/views/ListView.vue": { file: "assets/ListView.js", imports: ["_ui.js"] },
};

describe("weight of the initial shell", () => {
  it("counts every entry and its static imports once, never a screen's chunk", () => {
    expect(initialFiles(manifest).sort()).toEqual([
      "assets/index.css",
      "assets/index.js",
      "assets/sdk.js",
      "assets/ui-entry.js",
      "assets/ui.css",
      "assets/ui.js",
      "assets/vue.runtime.js",
    ]);
  });

  it("fails above 300 Ko", () => {
    const at = (bytes: number) => shellMeasure(manifest, () => bytes / 7);
    expect(report([at(SHELL_BUDGET)]).ok).toBe(true);
    const over = report([at(SHELL_BUDGET + 7)]);
    expect(over.ok).toBe(false);
    expect(over.lines[0]).toMatch(/^FAIL initial shell with the shared modules, gzip -9: /);
  });
});
