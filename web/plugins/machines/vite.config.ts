// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// Build of the plugin of the machines as an operator deploys it (design « Plugins côté serveur »):
// `web/dist/plugins/machines/<version>/` holds `index.js`, with `@kartenn/sdk` left to the
// import map of the shell (W7), and the files of `public/` (manifest, catalogs, style
// sheet). Copy that directory under `[web] plugins_dir` to install it.
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
    rollupOptions: { external: ["@kartenn/sdk"] },
  },
  test: {
    name: "machines",
    environment: "happy-dom",
  },
});
