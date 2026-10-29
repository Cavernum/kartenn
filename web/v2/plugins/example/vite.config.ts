// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

/** The modules of the import map of the shell (ADR 0038), left to it. */
const SHARED = ["@kartenn/sdk", "@kartenn/ui", "vue"];

// Build of the plugin as an operator installs it (spec 007, design « Plugins côté
// serveur »): `web/v2/dist/plugins/<name>/<version>/` holds `index.js` and the files of
// `public/` (manifest, catalogues). That directory is the `[web] plugins_dir` of
// web/v2/e2e/kartenn.toml.
const manifest = JSON.parse(
  readFileSync(fileURLToPath(new URL("public/manifest.json", import.meta.url)), "utf8"),
) as { name: string; version: string };

export default defineConfig({
  build: {
    target: "es2022",
    outDir: fileURLToPath(
      new URL(`../../dist/plugins/${manifest.name}/${manifest.version}/`, import.meta.url),
    ),
    emptyOutDir: true,
    lib: {
      entry: fileURLToPath(new URL("src/index.ts", import.meta.url)),
      formats: ["es"],
      fileName: () => "index.js",
    },
    rollupOptions: { external: SHARED },
  },
  test: {
    name: "plugin-example",
    environment: "node",
  },
});
