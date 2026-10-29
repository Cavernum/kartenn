import { defineConfig } from "vitest/config";

// One Vitest run for the whole workspace: each package is a project with its own config.
export default defineConfig({
  test: {
    projects: ["tools", "sdk", "ui", "shell", "plugins/*"],
  },
});
