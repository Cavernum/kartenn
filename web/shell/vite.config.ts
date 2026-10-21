import { fileURLToPath } from "node:url";
import vue from "@vitejs/plugin-vue";
import { build, type Plugin, type ResolvedConfig } from "vite";
import { defineConfig } from "vitest/config";

const SDK_SOURCES = fileURLToPath(new URL("../sdk/src/", import.meta.url));
const SDK_SPECIFIERS = ["@kartenn/sdk", "@kartenn/sdk/host"];

/** The import map of the shell (W7, REQ-007-12): the single instance of the SDK. */
function importMap(imports: Record<string, string>): string {
  return `<script type="importmap">\n      ${JSON.stringify({ imports })}\n    </script>`;
}

/**
 * The SDK as the shell and its plugins share it (design « SDK », « Chargement des
 * plugins »): in the build, `@kartenn/sdk` and `@kartenn/sdk/host` stay external, the SDK
 * is built once under `sdk/` of the output (one common chunk, so both entry points share
 * their state) and the import map of `index.html` names it. Under the development server,
 * the map names the very modules Vite serves to the shell.
 */
function sharedSdk(): Plugin {
  let config: ResolvedConfig;
  return {
    name: "kartenn-shared-sdk",
    config(_, { command }) {
      if (command === "build") return { build: { rollupOptions: { external: SDK_SPECIFIERS } } };
    },
    configResolved(resolved) {
      config = resolved;
    },
    transformIndexHtml(html) {
      const imports =
        config.command === "build"
          ? { "@kartenn/sdk": "/sdk/index.js", "@kartenn/sdk/host": "/sdk/host.js" }
          : {
              "@kartenn/sdk": `/@fs${SDK_SOURCES}index.ts`,
              "@kartenn/sdk/host": `/@fs${SDK_SOURCES}host.ts`,
            };
      return html.replace(/<script type="importmap">[\s\S]*?<\/script>/, importMap(imports));
    },
    async closeBundle() {
      if (config.command !== "build") return;
      await build({
        configFile: false,
        logLevel: "warn",
        build: {
          target: config.build.target,
          outDir: `${config.build.outDir}/sdk`,
          emptyOutDir: true,
          lib: {
            entry: { index: `${SDK_SOURCES}index.ts`, host: `${SDK_SOURCES}host.ts` },
            formats: ["es"],
          },
          rollupOptions: {
            output: { entryFileNames: "[name].js", chunkFileNames: "[name]-[hash].js" },
          },
        },
      });
    },
  };
}

export default defineConfig({
  plugins: [vue(), sharedSdk()],
  define: {
    // vue-i18n (W11): composition API only; messages compiled to an AST at run time (JIT,
    // no eval nor new Function, CSP without 'unsafe-eval'), so the message compiler stays
    // in the build for the catalogs of plugins (design « Risques », vue-i18n and CSP).
    __VUE_I18N_FULL_INSTALL__: true,
    __VUE_I18N_LEGACY_API__: false,
    __INTLIFY_PROD_DEVTOOLS__: false,
    __INTLIFY_JIT_COMPILATION__: true,
    __INTLIFY_DROP_MESSAGE_COMPILER__: false,
  },
  server: {
    // One origin in development (REQ-007-36): Vite relays the API and the plugins to the
    // `kartenn` process of devenv (dev/kartenn.toml). Its certificate comes from the
    // development authority of kartenn-dev-idp, unknown to Node: not checked, on the
    // loopback only.
    proxy: Object.fromEntries(
      ["/v1", "/plugins"].map((path) => [
        path,
        { target: "https://127.0.0.1:8443", secure: false },
      ]),
    ),
  },
  build: {
    target: "es2022",
    // Served by kartenn-server from `[web] root` (design « Arborescence de web/ »).
    outDir: "../dist/shell",
    emptyOutDir: true,
    // Read by scripts/size.mjs (REQ-007-32); `.vite/` is a hidden directory, never served.
    manifest: true,
  },
  test: {
    name: "shell",
    environment: "node",
  },
});
