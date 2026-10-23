import vue from "@vitejs/plugin-vue";
import type { Plugin } from "vite";
import { defineConfig } from "vitest/config";

/** The one font the first screen preloads (design « Typographie »): Geist, Latin. */
const PRELOADED_FONT = "geist-latin-wght-normal";

/**
 * Preload of the sans font in the built `index.html`: a `<link rel=preload>` element (no
 * inline style, CSP), naming the fingerprinted file the sheet of @kartenn/ui refers to.
 */
function preloadFont(): Plugin {
  return {
    name: "kartenn-preload-font",
    apply: "build",
    transformIndexHtml: {
      order: "post",
      handler(_, ctx) {
        const font = Object.values(ctx.bundle ?? {}).find(
          (asset) => asset.type === "asset" && asset.fileName.includes(`${PRELOADED_FONT}-`),
        );
        if (!font) throw new Error(`${PRELOADED_FONT}: font missing from the build`);
        return [
          {
            tag: "link",
            attrs: {
              rel: "preload",
              href: `/${font.fileName}`,
              as: "font",
              type: "font/woff2",
              crossorigin: "",
            },
            injectTo: "head",
          },
        ];
      },
    },
  };
}

// The new shell (spec 021), its catalogue /_ui (spec 020) in every mode but production:
// `vite build` leaves it out, `vite build --mode catalogue` keeps it (test build, CI
// artefact).
export default defineConfig(({ mode }) => ({
  plugins: [vue(), preloadFont()],
  build: {
    target: "es2022",
    outDir: mode === "catalogue" ? "../dist/catalogue" : "../dist/shell",
    emptyOutDir: true,
    // Fonts stay files with a fingerprint: preloaded and cached, never data: URLs.
    assetsInlineLimit: 0,
    // Read by the test of the production build (no module of the catalogue).
    manifest: true,
  },
  preview: {
    host: "127.0.0.1",
    port: 4173,
    strictPort: true,
  },
  test: {
    name: "shell",
    environment: "node",
  },
}));
