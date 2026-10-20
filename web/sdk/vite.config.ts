// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

// A library with two entry points: the public API and the hooks of the shell.
export default defineConfig({
  build: {
    target: "es2022",
    lib: {
      entry: {
        index: fileURLToPath(new URL("src/index.ts", import.meta.url)),
        host: fileURLToPath(new URL("src/host.ts", import.meta.url)),
        testing: fileURLToPath(new URL("src/testing/index.ts", import.meta.url)),
      },
      formats: ["es"],
    },
  },
  test: {
    name: "sdk",
    environment: "node",
  },
});
