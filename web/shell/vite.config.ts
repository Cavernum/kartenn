import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [vue()],
  build: {
    target: "es2022",
    // Served by kartenn-server from `[web] root` (design « Arborescence de web/ »).
    outDir: "../dist/shell",
    emptyOutDir: true,
  },
  test: {
    name: "shell",
    environment: "node",
  },
});
