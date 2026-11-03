// Weight of the initial shell (spec 021, REQ-021-30, design « Budgets »), run by
// `bench-web`: the entry of `index.html` and its static imports, with the shared modules
// the import map names (ADR 0038: the SDK, @kartenn/ui, vue) and what they import
// statically, their sheets included; each file once, gzipped at level 9. The plugins and
// the chunks of the screens, read when a route opens, are left out. 1 Ko = 1 000 bytes.
// Prints `PASS|FAIL initial shell …`; exits 1 above the budget.
// `node web/v2/shell/scripts/shell-size.ts [dist]` after
// `pnpm -C web/v2 --filter @kartenn/shell build`; `dist` defaults to web/v2/dist/shell.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { closure, report, type Manifest, type Measure } from "./size.ts";

/** The budget of the initial shell, in bytes (REQ-021-30), framework and library included. */
export const SHELL_BUDGET = 300_000;

/**
 * The files of the initial shell: the closure of every entry of the build (the document's
 * and those of the shared modules), scripts and sheets.
 */
export function initialFiles(manifest: Manifest): string[] {
  const files = new Set<string>();
  for (const [key, chunk] of Object.entries(manifest)) {
    if (!chunk.isEntry) continue;
    const { scripts, sheets } = closure(manifest, key);
    for (const file of [...scripts, ...sheets]) files.add(file);
  }
  return [...files];
}

/** The measure of the initial shell; `size(file)` gives the gzipped bytes of one file. */
export function shellMeasure(manifest: Manifest, size: (file: string) => number): Measure {
  const files = initialFiles(manifest);
  return {
    name: "initial shell with the shared modules, gzip -9",
    files,
    bytes: files.reduce((sum, file) => sum + size(file), 0),
    limit: SHELL_BUDGET,
  };
}

// Run as a script (Node 24: `import.meta.main`), never a silent exit 0 when imported.
if (import.meta.main) {
  const dist = process.argv[2] ?? fileURLToPath(new URL("../../dist/shell/", import.meta.url));
  const manifest = JSON.parse(readFileSync(join(dist, ".vite/manifest.json"), "utf8")) as Manifest;
  const size = (file: string) => gzipSync(readFileSync(join(dist, file)), { level: 9 }).length;
  const shell = shellMeasure(manifest, size);
  for (const file of shell.files) console.log(`  ${file}: ${size(file)}`);
  const { lines, ok } = report([shell]);
  for (const line of lines) console.log(line);
  if (!ok) process.exit(1);
}
