import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [vue()],
  build: {
    target: "es2022",
  },
  test: {
    name: "shell",
    environment: "node",
  },
});
