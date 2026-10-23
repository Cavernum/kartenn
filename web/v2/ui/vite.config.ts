// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { fileURLToPath } from "node:url";
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

// A library (design « Arborescence »): the module and its sheet `style.css`. Vue and Reka
// UI stay external: the shell serves one instance of each through its import map
// (ADR 0038).
export default defineConfig({
  plugins: [vue()],
  build: {
    target: "es2022",
    cssCodeSplit: false,
    lib: {
      entry: { index: fileURLToPath(new URL("src/index.ts", import.meta.url)) },
      formats: ["es"],
      cssFileName: "style",
    },
    rollupOptions: {
      external: ["vue", "reka-ui", "@tanstack/vue-virtual"],
    },
  },
  test: {
    name: "ui",
    environment: "node",
  },
});
