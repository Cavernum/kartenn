// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The weight of a first-party plugin (spec 021, REQ-021-30, Q-021-11; design « Budgets »):
// its `index.js` as its own build makes it, in memory (nothing written to `dist`), gzipped
// at level 9; the shared modules of the import map left out — they are imported, never
// bundled. 1 Ko = 1 000 bytes. Read by the package test of each plugin
// (`plugins/<name>/test/package.test.ts`), hence by `k-web2-check` (Vitest), and run as a
// script by `bench-web` over every plugin of web/v2/plugins:
// `node web/v2/tools/plugin-size.ts`, one line `PASS|FAIL plugin <name> …` each, exit 1 if
// one weighs more than its budget or bundles a shared module.

import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { build, type Rollup } from "vite";

/** The budget of a plugin, in bytes, blocking (Q-021-11). */
export const PLUGIN_BUDGET = 50_000;

/**
 * The modules of the import map (ADR 0038, `SHARED` of web/v2/shell/vite.config.ts). The
 * sheet of @kartenn/ui is not one: the document links it, a plugin never imports it.
 */
export const SHARED_MODULES: readonly string[] = ["@kartenn/sdk", "@kartenn/ui", "vue"];

/** The sources of a shared module, found in a plugin's file only by mistake. */
const SHARED_SOURCES =
  /\/(sdk|ui)\/src\/|\/node_modules\/(\.pnpm\/[^/]+\/node_modules\/)?(vue|@vue|@tanstack|reka-ui)\//;

export interface PluginWeight {
  /** Bytes of `index.js`, gzipped at level 9. */
  bytes: number;
  /** What `index.js` imports (static and dynamic). */
  imports: string[];
  /** Modules of the shared packages bundled into it (none expected). */
  bundled: string[];
  /**
   * The sheets the build emits: none for a first-party plugin, which has no sheet of its
   * own (A23: the document's @kartenn/ui, layout by `Stack` and `Cluster`).
   */
  sheets: string[];
}

/** Builds the plugin at `root` with its own `vite.config.ts`, in memory, and weighs it. */
export async function pluginWeight(root: string): Promise<PluginWeight> {
  const result = await build({
    root,
    configFile: join(root, "vite.config.ts"),
    logLevel: "silent",
    build: { write: false },
  });
  const outputs = (Array.isArray(result) ? result : [result]) as Rollup.RollupOutput[];
  const files = outputs.flatMap((output) => output.output);
  const entry = files.find(
    (file): file is Rollup.OutputChunk => file.type === "chunk" && file.isEntry,
  );
  if (!entry) throw new Error(`${root}: no entry in the build`);
  return {
    bytes: gzipSync(entry.code, { level: 9 }).length,
    imports: [...entry.imports, ...entry.dynamicImports],
    bundled: entry.moduleIds.filter((id) => SHARED_SOURCES.test(id)),
    sheets: files.map((file) => file.fileName).filter((name) => name.endsWith(".css")),
  };
}

/** The line of the bench for one plugin, and whether it holds. */
export function pluginLine(name: string, weight: PluginWeight): { line: string; ok: boolean } {
  const ok = weight.bytes <= PLUGIN_BUDGET && weight.bundled.length === 0;
  const bundled = weight.bundled.length > 0 ? `, ${weight.bundled.length} shared bundled` : "";
  const measured = `${(weight.bytes / 1000).toFixed(1)} Ko${bundled}`;
  return {
    line: `${ok ? "PASS" : "FAIL"} plugin ${name}, gzip -9: ${measured} (limit ${PLUGIN_BUDGET / 1000} Ko)`,
    ok,
  };
}

// Run as a script (Node 24: `import.meta.main`), never a silent exit 0 when imported.
if (import.meta.main) {
  const plugins = fileURLToPath(new URL("../plugins/", import.meta.url));
  let ok = true;
  for (const name of readdirSync(plugins).sort()) {
    const root = join(plugins, name);
    if (!existsSync(join(root, "vite.config.ts"))) continue;
    const verdict = pluginLine(name, await pluginWeight(root));
    console.log(verdict.line);
    ok &&= verdict.ok;
  }
  if (!ok) process.exit(1);
}
