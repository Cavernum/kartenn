import { describe, expect, it } from "vitest";
import { documentFiles, initialFiles, SHELL_BUDGET, shellMeasure } from "../scripts/shell-size.ts";
import { report, type Manifest } from "../scripts/size.ts";

// The weight of the initial shell (spec 021, REQ-021-30; scripts/shell-size.ts): what the
// document loads — its entry with its static imports and sheets, and what `index.html`
// names — each file once; a screen read by its route, the fonts and the entries of the
// import map only a plugin loads left out.

const manifest: Manifest = {
  "index.html": {
    file: "assets/index.js",
    src: "index.html",
    isEntry: true,
    imports: ["_vue.runtime.js", "_ui.js", "src/shared/ui.ts"],
    css: ["assets/index.css"],
  },
  "src/shared/ui.ts": { file: "assets/ui-entry.js", isEntry: true, imports: ["_ui.js"] },
  "../sdk/src/index.ts": { file: "assets/sdk.js", isEntry: true, imports: ["_vue.runtime.js"] },
  "_ui.js": { file: "assets/ui.js", imports: ["_vue.runtime.js"] },
  "_vue.runtime.js": { file: "assets/vue.runtime.js" },
  "src/views/ListView.vue": { file: "assets/ListView.js", imports: ["_ui.js"] },
};
const html = [
  '<script type="importmap">{"imports":{"vue":"/assets/vue.js"}}</script>',
  '<link rel="stylesheet" href="/boot.css" />',
  '<script src="/boot.js"></script>',
  '<script type="module" crossorigin src="/assets/index.js"></script>',
  '<link rel="modulepreload" crossorigin href="/assets/ui.js">',
  '<link rel="stylesheet" crossorigin href="/assets/ui.css" integrity="sha384-x">',
  '<link rel="preload" href="/assets/geist.woff2" as="font" type="font/woff2" crossorigin="">',
].join("\n");

describe("weight of the initial shell", () => {
  it("reads the scripts, module preloads and sheets of the document, never a font", () => {
    expect(documentFiles(html)).toEqual([
      "boot.css",
      "boot.js",
      "assets/index.js",
      "assets/ui.js",
      "assets/ui.css",
    ]);
  });

  it("counts what the document loads once each, never a screen nor an entry left to plugins", () => {
    expect(initialFiles(manifest, html).sort()).toEqual([
      "assets/index.css",
      "assets/index.js",
      "assets/ui-entry.js",
      "assets/ui.css",
      "assets/ui.js",
      "assets/vue.runtime.js",
      "boot.css",
      "boot.js",
    ]);
  });

  it("fails above 300 Ko", () => {
    const at = (bytes: number) => shellMeasure(manifest, html, () => bytes / 8);
    expect(report([at(SHELL_BUDGET)]).ok).toBe(true);
    const over = report([at(SHELL_BUDGET + 8)]);
    expect(over.ok).toBe(false);
    expect(over.lines[0]).toMatch(/^FAIL initial shell, gzip -9: /);
  });
});
