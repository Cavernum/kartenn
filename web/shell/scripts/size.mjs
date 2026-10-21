// Size of the initial shell (spec 007, REQ-007-32, design « Budgets »): the entry point
// of the build, its static imports, their CSS and the SDK, read from the manifest of
// Vite, each gzipped at level 9. Prints the total; exits 1 above 300 000 bytes.
// `node web/shell/scripts/size.mjs` after `pnpm -C web build`.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

export const BUDGET = 300_000;
const DIST = fileURLToPath(new URL("../../dist/shell/", import.meta.url));

/** The files the first screen loads: the entry, its static imports and their CSS. */
export function initialFiles(manifest) {
  const files = new Set();
  const visit = (key) => {
    const chunk = manifest[key];
    if (!chunk || files.has(chunk.file)) return;
    files.add(chunk.file);
    for (const css of chunk.css ?? []) files.add(css);
    for (const imported of chunk.imports ?? []) visit(imported);
  };
  for (const [key, chunk] of Object.entries(manifest)) if (chunk.isEntry) visit(key);
  return [...files];
}

const gzipped = (path) => gzipSync(readFileSync(path), { level: 9 }).length;

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const manifest = JSON.parse(readFileSync(join(DIST, ".vite/manifest.json"), "utf8"));
  const shell = initialFiles(manifest).map((file) => [file, gzipped(join(DIST, file))]);
  const sdk = readdirSync(join(DIST, "sdk"))
    .filter((file) => file.endsWith(".js"))
    .map((file) => [`sdk/${file}`, gzipped(join(DIST, "sdk", file))]);
  const total = [...shell, ...sdk].reduce((sum, [, size]) => sum + size, 0);
  for (const [file, size] of [...shell, ...sdk]) console.log(`  ${file}: ${size}`);
  const verdict = total <= BUDGET ? "PASS" : "FAIL";
  console.log(`${verdict} initial shell, gzip -9: ${total} bytes (limit ${BUDGET})`);
  if (total > BUDGET) process.exit(1);
}
