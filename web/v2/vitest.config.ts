import { defineConfig } from "vitest/config";

// One Vitest run for the whole workspace. The packages arrive with the tasks of specs 020
// and 021, each as a project with its own config.
export default defineConfig({
  test: {
    passWithNoTests: true,
  },
});
