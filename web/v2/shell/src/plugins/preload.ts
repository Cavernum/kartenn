import type { components } from "@kartenn/sdk";

// The modules of a plugin, preloaded with their digests (007 « Chargement des plugins par le
// shell »; 021 design « Préchargement »): the mechanism of the 007 loader, called earlier —
// on the intent of a link to the plugin, before its view is opened. The loader of the new
// shell (task 16) waits on the same promises, so that a module is never inserted twice.

/** What the preload needs of a plugin of `ui_plugins`. */
export type PluginModules = Pick<components["schemas"]["UiPlugin"], "base" | "integrity">;

/** Every `modulepreload` inserted, by URL; a failed one is forgotten, to be tried again. */
const inserted = new Map<string, Promise<void>>();

function preload(href: string, integrity: string): Promise<void> {
  const known = inserted.get(href);
  if (known) return known;
  const link = document.createElement("link");
  link.rel = "modulepreload";
  link.href = href;
  link.integrity = integrity;
  const loaded = new Promise<void>((resolve, reject) => {
    link.addEventListener("load", () => resolve(), { once: true });
    link.addEventListener(
      "error",
      () => {
        inserted.delete(href);
        link.remove();
        reject(new Error(`kartenn: module ${href} not preloaded`));
      },
      { once: true },
    );
  });
  inserted.set(href, loaded);
  document.head.append(link);
  return loaded;
}

/** The modules of a plugin: its `.js` files, by URL, with their digests. */
const modulesOf = (plugin: PluginModules) =>
  Object.entries(plugin.integrity)
    .filter(([path]) => path.endsWith(".js"))
    .map(([path, integrity]) => [plugin.base + path, integrity] as const);

/**
 * Inserts a `modulepreload` with its digest for each module of the plugin not yet inserted;
 * resolves once all are loaded, rejects if one fails.
 */
export function preloadModules(plugin: PluginModules): Promise<void> {
  return Promise.all(modulesOf(plugin).map(([href, integrity]) => preload(href, integrity))).then(
    () => undefined,
  );
}

/**
 * Resolves once every module of the plugin inserted so far has loaded or failed (a failed one
 * is already forgotten): what `PLUGINS_FETCHED` waits for, never the setting aside.
 */
export function modulesSettled(plugin: PluginModules): Promise<void> {
  return Promise.allSettled(
    modulesOf(plugin).map(([href]) => inserted.get(href) ?? Promise.resolve()),
  ).then(() => undefined);
}
