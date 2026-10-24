import { createHash } from "node:crypto";
import { mkdtempSync, readdirSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "vite";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

// The builds of the shell (spec 020, tasks 6 and 8; REQ-020-10, 24): the production build
// leaves the catalogue out; both preload the sans font, served from the origin.

const root = fileURLToPath(new URL("..", import.meta.url));
const out = mkdtempSync(join(tmpdir(), "kartenn-shell-build-"));

type Manifest = Record<string, { file: string; src?: string }>;

async function built(mode: string) {
  const outDir = join(out, mode);
  await build({
    root,
    mode,
    configFile: join(root, "vite.config.ts"),
    logLevel: "silent",
    build: { outDir, emptyOutDir: true },
  });
  const manifest = JSON.parse(
    readFileSync(join(outDir, ".vite/manifest.json"), "utf8"),
  ) as Manifest;
  const assets = readdirSync(join(outDir, "assets"));
  const js = assets
    .filter((file) => file.endsWith(".js"))
    .map((file) => readFileSync(join(outDir, "assets", file), "utf8"))
    .join("\n");
  const html = readFileSync(join(outDir, "index.html"), "utf8");
  return { outDir, manifest, assets, js, html };
}

let production: Awaited<ReturnType<typeof built>>;
let catalogue: Awaited<ReturnType<typeof built>>;

beforeAll(async () => {
  production = await built("production");
  catalogue = await built("catalogue");
}, 120_000);
afterAll(() => rmSync(out, { recursive: true, force: true }));

describe("builds of the shell", () => {
  it("production has no module of the catalogue", () => {
    expect(Object.keys(production.manifest).filter((key) => key.includes("_ui/"))).toEqual([]);
    expect(production.js).not.toContain("Catalogue du système de design");
  });

  it("the catalogue build has it, in separate chunks", () => {
    expect(Object.keys(catalogue.manifest).some((key) => key.includes("_ui/"))).toBe(true);
    expect(catalogue.js).toContain("Catalogue du système de design");
  });

  it("serves the fonts as fingerprinted files, the sans one preloaded", () => {
    for (const { assets, html } of [production, catalogue]) {
      const fonts = assets.filter((file) => file.endsWith(".woff2")).sort();
      expect(fonts).toHaveLength(4);
      const sans = fonts.find((file) => file.startsWith("geist-latin-wght-normal-"));
      expect(sans).toBeDefined();
      expect(html).toContain(
        `<link rel="preload" href="/assets/${sans}" as="font" type="font/woff2" crossorigin`,
      );
      expect(html).not.toMatch(/style=|<style/);
    }
  });

  it("shares vue, @kartenn/sdk and @kartenn/ui through an import map with digests", () => {
    const sri = (file: string) =>
      `sha384-${createHash("sha384").update(readFileSync(file)).digest("base64")}`;
    for (const { outDir, html } of [production, catalogue]) {
      const text = /<script type="importmap">([\s\S]*?)<\/script>/.exec(html)?.[1];
      expect(text, "import map").toBeDefined();
      expect(html.indexOf("importmap")).toBeLessThan(html.indexOf('type="module"'));
      const map = JSON.parse(text!) as {
        imports: Record<string, string>;
        integrity: Record<string, string>;
      };
      expect(Object.keys(map.imports).sort()).toEqual(["@kartenn/sdk", "@kartenn/ui", "vue"]);
      const modules = readdirSync(join(outDir, "assets")).filter((file) => file.endsWith(".js"));
      expect(Object.keys(map.integrity).sort()).toEqual(
        modules.map((file) => `/assets/${file}`).sort(),
      );
      for (const [url, digest] of Object.entries(map.integrity))
        expect(digest, url).toBe(sri(join(outDir, url)));
      for (const url of Object.values(map.imports)) expect(map.integrity[url]).toBeDefined();

      const sheet =
        /<link rel="stylesheet" crossorigin href="(\/assets\/ui-[^"]+\.css)" integrity="([^"]+)"/.exec(
          html,
        );
      expect(sheet, "sheet of @kartenn/ui linked with its digest").not.toBeNull();
      expect(sheet![2]).toBe(sri(join(outDir, sheet![1]!)));
    }
  });

  it("serves the runtime build of vue: nothing evaluated under the CSP", () => {
    for (const { js } of [production, catalogue]) {
      expect(js).not.toMatch(/new Function\(|\beval\(/);
      expect(js).toContain("vue.runtime");
    }
  });
});
