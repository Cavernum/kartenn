// Weight of the initial shell (spec 021, REQ-021-30, design « Budgets »), run by
// `bench-web`: what the document loads before any route opens — the entry of
// `index.html` and its static imports, with their sheets (vue, the SDK and @kartenn/ui
// among them), and every script, module preload and sheet `index.html` names (the boot
// files included); each file once, gzipped at level 9. Left out: the fonts (their own
// budget, `size.ts`), the chunks of the screens, read when a route opens, and the entries
// of the import map no file of the shell imports (`vue`, `@kartenn/sdk`: a plugin loads
// them, with itself). 1 Ko = 1 000 bytes. Prints `PASS|FAIL initial shell …`; exits 1
// above the budget. `node web/v2/shell/scripts/shell-size.ts [dist]` after
// `pnpm -C web/v2 --filter @kartenn/shell build`; `dist` defaults to web/v2/dist/shell.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { closure, report, type Manifest, type Measure } from "./size.ts";

/** The budget of the initial shell, in bytes (REQ-021-30), framework and library included. */
export const SHELL_BUDGET = 300_000;

/** The scripts, module preloads and sheets `html` names, as paths of the build. */
export function documentFiles(html: string): string[] {
  const tags = [...html.matchAll(/<(script|link)\b[^>]*>/g)].map(([tag]) => tag);
  return tags.flatMap((tag) => {
    const loads = tag.startsWith("<script")
      ? /\bsrc="/.test(tag)
      : /\brel="(modulepreload|stylesheet)"/.test(tag);
    const url = /\b(?:src|href)="\/?([^"]+)"/.exec(tag)?.[1];
    return loads && url !== undefined && !/^[a-z]+:/i.test(url) ? [url] : [];
  });
}

/** The files of the initial shell: the closure of the document's entry, and what it names. */
export function initialFiles(manifest: Manifest, html: string): string[] {
  const entry = Object.keys(manifest).find((key) => manifest[key]!.src === "index.html");
  if (!entry) throw new Error("no entry index.html in the manifest");
  const { scripts, sheets } = closure(manifest, entry);
  return [...new Set([...scripts, ...sheets, ...documentFiles(html)])];
}

/** The measure of the initial shell; `size(file)` gives the gzipped bytes of one file. */
export function shellMeasure(
  manifest: Manifest,
  html: string,
  size: (file: string) => number,
): Measure {
  const files = initialFiles(manifest, html);
  return {
    name: "initial shell, gzip -9",
    files,
    bytes: files.reduce((sum, file) => sum + size(file), 0),
    limit: SHELL_BUDGET,
  };
}

// Run as a script (Node 24: `import.meta.main`), never a silent exit 0 when imported.
if (import.meta.main) {
  const dist = process.argv[2] ?? fileURLToPath(new URL("../../dist/shell/", import.meta.url));
  const manifest = JSON.parse(readFileSync(join(dist, ".vite/manifest.json"), "utf8")) as Manifest;
  const html = readFileSync(join(dist, "index.html"), "utf8");
  const size = (file: string) => gzipSync(readFileSync(join(dist, file)), { level: 9 }).length;
  const shell = shellMeasure(manifest, html, size);
  for (const file of shell.files) console.log(`  ${file}: ${size(file)}`);
  const { lines, ok } = report([shell]);
  for (const line of lines) console.log(line);
  if (!ok) process.exit(1);
}
