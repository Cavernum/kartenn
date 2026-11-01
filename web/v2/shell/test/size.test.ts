import { describe, expect, it } from "vitest";
import { BUDGETS, measure, report, type Manifest } from "../scripts/size.ts";

// The weight of @kartenn/ui (spec 020, REQ-020-29; scripts/size.ts), on a manifest shaped as
// the production build gives it: the library's entry and chunk, the runtime of vue in its
// own chunk, the sheet, the preloaded font.

const manifest: Manifest = {
  "index.html": { file: "assets/index.js", src: "index.html", isEntry: true, imports: ["_ui.js"] },
  "src/shared/ui.ts": {
    file: "assets/ui-entry.js",
    src: "src/shared/ui.ts",
    isEntry: true,
    imports: ["_ui.js"],
  },
  "_ui.js": { file: "assets/ui.js", imports: ["_vue.runtime.js"], css: ["assets/ui.css"] },
  "_vue.runtime.js": { file: "assets/vue.runtime.js" },
  "../node_modules/vue/dist/vue.runtime.esm-bundler.js": {
    file: "assets/vue.js",
    src: "../node_modules/vue/dist/vue.runtime.esm-bundler.js",
    isEntry: true,
    imports: ["_vue.runtime.js"],
  },
};
const html =
  '<link rel="modulepreload" href="/assets/ui.js">' +
  '<link rel="preload" href="/assets/geist.woff2" as="font" type="font/woff2" crossorigin="">';

describe("weight of @kartenn/ui", () => {
  it("counts the library and what it imports, then with vue; its sheet; the preloaded fonts", () => {
    const sizes: Record<string, number> = {
      "assets/ui-entry.js": 1_000,
      "assets/ui.js": 60_000,
      "assets/vue.runtime.js": 40_000,
      "assets/vue.js": 1_000,
      "assets/ui.css": 15_000,
      "assets/geist.woff2": 30_000,
    };
    const measures = measure(manifest, html, (file) => sizes[file]!);
    expect(measures.map(({ name, bytes }) => [name, bytes])).toEqual([
      ["@kartenn/ui JS, gzip -9", 61_000],
      ["@kartenn/ui + vue JS, gzip -9", 102_000],
      ["@kartenn/ui sheet, gzip -9", 15_000],
      ["preloaded fonts", 30_000],
    ]);
    expect(report(measures)).toEqual({
      lines: [
        "PASS @kartenn/ui JS, gzip -9: 61.0 Ko (limit 90 Ko)",
        "PASS @kartenn/ui + vue JS, gzip -9: 102.0 Ko (limit 120 Ko)",
        "PASS @kartenn/ui sheet, gzip -9: 15.0 Ko (limit 20 Ko)",
        "PASS preloaded fonts: 30.0 Ko (limit 120 Ko)",
      ],
      ok: true,
    });
  });

  it("fails on a budget exceeded by one byte", () => {
    const sizes = (file: string) => (file === "assets/ui.css" ? BUDGETS.sheet + 1 : 1);
    expect(report(measure(manifest, html, sizes)).ok).toBe(false);
  });

  it("says which budget fails", () => {
    const sizes = (file: string) => (file === "assets/ui.css" ? BUDGETS.sheet + 100 : 1);
    const { lines, ok } = report(measure(manifest, html, sizes));
    expect(ok).toBe(false);
    expect(lines.filter((line) => line.startsWith("FAIL"))).toEqual([
      "FAIL @kartenn/ui sheet, gzip -9: 20.1 Ko (limit 20 Ko)",
    ]);
  });
});
