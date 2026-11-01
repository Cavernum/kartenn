// Weight of @kartenn/ui in the production build of the new shell (spec 020, REQ-020-29,
// design « Budgets »): the module the import map names for it, with everything it imports
// statically (its dependencies Reka UI, TanStack Virtual and Lucide come in its chunk),
// then the same with `vue`, each file gzipped at level 9; its sheet, gzipped; the fonts
// `index.html` preloads, as served (woff2). 1 Ko = 1 000 bytes. Prints one line per budget,
// `PASS|FAIL <budget>: <measured> (limit <limit>)`; exits 1 if one is exceeded.
// `node web/v2/shell/scripts/size.ts` (Node strips the types) after
// `pnpm -C web/v2 --filter @kartenn/shell build`.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

/** The budgets, in bytes (REQ-020-29); a budget is relaxed by decision only. */
export const BUDGETS = {
  ui: 90_000,
  uiVue: 120_000,
  sheet: 20_000,
  fonts: 120_000,
} as const;

/** The manifest of a Vite build (`.vite/manifest.json`). */
export type Manifest = Record<
  string,
  { file: string; src?: string; isEntry?: boolean; imports?: string[]; css?: string[] }
>;

/** One budget of weight: what it counts, its total in bytes and its limit. */
export interface Measure {
  name: string;
  files: string[];
  bytes: number;
  limit: number;
}

/** The key of the manifest of the entry whose source ends with `source`. */
function entryOf(manifest: Manifest, source: string): string {
  const key = Object.keys(manifest).find(
    (k) => manifest[k]!.isEntry && (manifest[k]!.src ?? k).endsWith(source),
  );
  if (!key) throw new Error(`no entry ${source} in the manifest`);
  return key;
}

/** The scripts an entry loads (itself and its static imports) and their sheets. */
export function closure(manifest: Manifest, key: string): { scripts: string[]; sheets: string[] } {
  const scripts = new Set<string>();
  const sheets = new Set<string>();
  const visit = (k: string) => {
    const chunk = manifest[k];
    if (!chunk || scripts.has(chunk.file)) return;
    scripts.add(chunk.file);
    for (const css of chunk.css ?? []) sheets.add(css);
    for (const imported of chunk.imports ?? []) visit(imported);
  };
  visit(key);
  return { scripts: [...scripts], sheets: [...sheets] };
}

/** The fonts `index.html` preloads (`<link rel="preload" as="font">`), as paths of the build. */
export function preloadedFonts(html: string): string[] {
  return [...html.matchAll(/<link\b[^>]*>/g)]
    .map(([tag]) => tag)
    .filter((tag) => /\brel="preload"/.test(tag) && /\bas="font"/.test(tag))
    .flatMap((tag) => {
      const href = /\bhref="\/?([^"]+)"/.exec(tag)?.[1];
      return href === undefined ? [] : [href];
    });
}

/**
 * The measures of a build: `size(file, gzip)` gives the bytes of one of its files, gzipped
 * at level 9 or as served.
 */
export function measure(
  manifest: Manifest,
  html: string,
  size: (file: string, gzip: boolean) => number,
): Measure[] {
  const ui = closure(manifest, entryOf(manifest, "src/shared/ui.ts"));
  const vue = closure(manifest, entryOf(manifest, "vue.runtime.esm-bundler.js"));
  const scripts = ui.scripts.filter((file) => !vue.scripts.includes(file));
  const withVue = [...new Set([...ui.scripts, ...vue.scripts])];
  const fonts = preloadedFonts(html);
  const total = (files: string[], gzip: boolean) =>
    files.reduce((sum, file) => sum + size(file, gzip), 0);
  return [
    { name: "@kartenn/ui JS, gzip -9", files: scripts, limit: BUDGETS.ui, gzip: true },
    { name: "@kartenn/ui + vue JS, gzip -9", files: withVue, limit: BUDGETS.uiVue, gzip: true },
    { name: "@kartenn/ui sheet, gzip -9", files: ui.sheets, limit: BUDGETS.sheet, gzip: true },
    { name: "preloaded fonts", files: fonts, limit: BUDGETS.fonts, gzip: false },
  ].map(({ gzip, ...rest }) => ({ ...rest, bytes: total(rest.files, gzip) }));
}

/** The lines of the report, and whether every budget holds. */
export function report(measures: Measure[]): { lines: string[]; ok: boolean } {
  const lines = measures.map(({ name, bytes, limit }) => {
    const verdict = bytes <= limit ? "PASS" : "FAIL";
    return `${verdict} ${name}: ${(bytes / 1000).toFixed(1)} Ko (limit ${limit / 1000} Ko)`;
  });
  return { lines, ok: measures.every(({ bytes, limit }) => bytes <= limit) };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const dist = fileURLToPath(new URL("../../dist/shell/", import.meta.url));
  const manifest = JSON.parse(readFileSync(join(dist, ".vite/manifest.json"), "utf8")) as Manifest;
  const html = readFileSync(join(dist, "index.html"), "utf8");
  const size = (file: string, gzip: boolean) => {
    const body = readFileSync(join(dist, file));
    return gzip ? gzipSync(body, { level: 9 }).length : body.length;
  };
  const measures = measure(manifest, html, size);
  for (const file of new Set(measures.flatMap(({ files }) => files)))
    console.log(`  ${file}: ${size(file, !file.endsWith(".woff2"))}`);
  const { lines, ok } = report(measures);
  for (const line of lines) console.log(line);
  if (!ok) process.exit(1);
}
