import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import vue from "@vitejs/plugin-vue";
import type { Plugin, ResolvedConfig } from "vite";
import { defineConfig } from "vitest/config";

/**
 * The modules the shell shares with its plugins (ADR 0038; spec 021, REQ-021-26), each an
 * entry of the build with its exports kept whole: `vue` resolves to its *runtime* build
 * (no template compiler, nothing evaluated under the CSP), `@kartenn/ui` comes with its
 * sheet.
 */
const SHARED = [
  { specifier: "vue", entry: "vue", source: "vue" },
  { specifier: "@kartenn/sdk", entry: "sdk", source: "@kartenn/sdk" },
  { specifier: "@kartenn/ui", entry: "ui", source: "src/shared/ui.ts" },
];
/** The sheet of @kartenn/ui, as a module of the build. */
const UI_SHEET = /\/ui\/src\/style\.css(\?|$)/;
const IMPORT_MAP = /<script type="importmap">([\s\S]*?)<\/script>/;
const PLACEHOLDER = "<!--kartenn-import-map-->";

const sri = (body: Buffer | string) =>
  `sha384-${createHash("sha384").update(body).digest("base64")}`;

/** The policy kartenn-server sends (crates/kartenn-rest/src/web.rs, `csp`). */
function policy(importMap: string | undefined): string {
  const hash =
    importMap === undefined
      ? ""
      : ` 'sha256-${createHash("sha256").update(importMap).digest("base64")}'`;
  return (
    `default-src 'none'; script-src 'self'${hash}; style-src 'self'; img-src 'self' data:; ` +
    "font-src 'self'; connect-src 'self'; manifest-src 'self'; base-uri 'none'; " +
    "frame-src 'none'; frame-ancestors 'none'"
  );
}

/**
 * The import map of the build (design « API des plugins »): `@kartenn/sdk`, `@kartenn/ui`
 * and `vue` by their fingerprinted files, the digest of every module of the build under
 * `integrity`, and the sheet of `@kartenn/ui` linked with its digest. Written once the
 * files are on disk, so the digests are those of what is served. The development server
 * has none: no plugin loads there yet (task 16).
 */
function sharedModules(): Plugin {
  let config: ResolvedConfig;
  const entries = new Map<string, string>();
  let sheet: string | undefined;
  const modules: string[] = [];
  return {
    name: "kartenn-shared-modules",
    apply: (_, env) => env.command === "build" || env.isPreview === true,
    config: () => ({
      build: {
        rollupOptions: {
          input: {
            index: "index.html",
            ...Object.fromEntries(SHARED.map(({ entry, source }) => [entry, source])),
          },
          preserveEntrySignatures: "strict",
        },
      },
    }),
    configResolved(resolved) {
      config = resolved;
    },
    transformIndexHtml: {
      order: "pre",
      handler: () => [{ tag: "script", children: PLACEHOLDER, injectTo: "head-prepend" }],
    },
    generateBundle(_, bundle) {
      entries.clear();
      modules.length = 0;
      sheet = undefined;
      for (const chunk of Object.values(bundle)) {
        if (chunk.type !== "chunk") continue;
        modules.push(chunk.fileName);
        const shared = SHARED.find(({ entry }) => chunk.isEntry && chunk.name === entry);
        if (shared) entries.set(shared.specifier, chunk.fileName);
        // The sheet of @kartenn/ui: that of the chunk holding its style.css.
        if (chunk.moduleIds.some((id) => UI_SHEET.test(id))) {
          const css = [...(chunk.viteMetadata?.importedCss ?? [])];
          if (css.length !== 1) throw new Error(`@kartenn/ui: sheets ${css.join(", ")}`);
          sheet = css[0];
        }
      }
    },
    // Once the files are on disk (and only if the build succeeded).
    writeBundle: {
      sequential: true,
      order: "post",
      handler() {
        const out = resolve(config.root, config.build.outDir);
        const url = (file: string) => `${config.base}${file}`;
        const imports: Record<string, string> = {};
        for (const { specifier } of SHARED) {
          const file = entries.get(specifier);
          if (!file) throw new Error(`${specifier}: no entry in the build`);
          imports[specifier] = url(file);
        }
        if (!sheet) throw new Error("@kartenn/ui: no sheet in the build");
        const integrity = Object.fromEntries(
          modules.sort().map((file) => [url(file), sri(readFileSync(join(out, file)))]),
        );
        const page = join(out, "index.html");
        let html = readFileSync(page, "utf8");
        const tag = `<script>${PLACEHOLDER}</script>`;
        if (!html.includes(tag)) throw new Error("index.html: no place for the import map");
        html = html.replace(
          tag,
          `<script type="importmap">${JSON.stringify({ imports, integrity })}</script>`,
        );
        const link = `href="${url(sheet)}"`;
        if (!html.includes(link)) throw new Error(`index.html: ${sheet} is not linked`);
        html = html.replace(link, `${link} integrity="${sri(readFileSync(join(out, sheet)))}"`);
        writeFileSync(page, html);
      },
    },
    // The preview sends the policy of kartenn-server, with the hash of this import map.
    configurePreviewServer(server) {
      const out = resolve(server.config.root, server.config.build.outDir);
      server.middlewares.use((_, response, next) => {
        let map: string | undefined;
        try {
          map = IMPORT_MAP.exec(readFileSync(join(out, "index.html"), "utf8"))?.[1];
        } catch {
          map = undefined;
        }
        response.setHeader("Content-Security-Policy", policy(map));
        next();
      });
    },
  };
}

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
  plugins: [vue(), preloadFont(), sharedModules()],
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
    // The captures pass a free port (`--port`, e2e/playwright.config.ts); 4173 by hand.
    port: Number(process.env.KARTENN_PREVIEW_PORT) || 4173,
    strictPort: true,
    // The policy kartenn-server sends, so that the journeys over the catalogue see the
    // violations the shell would (REQ-020-14): set by `sharedModules`, with the hash of
    // the import map.
  },
  test: {
    name: "shell",
    environment: "node",
  },
}));
