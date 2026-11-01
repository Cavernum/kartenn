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

/**
 * Inserts a `modulepreload` with its digest for each module of the plugin not yet inserted;
 * resolves once all are loaded; once all have settled, rejects with the first that failed,
 * so that nothing of the plugin is still fetched when it is set aside.
 */
export async function preloadModules(plugin: PluginModules): Promise<void> {
  const modules = Object.entries(plugin.integrity).filter(([path]) => path.endsWith(".js"));
  const settled = await Promise.allSettled(
    modules.map(([path, integrity]) => preload(plugin.base + path, integrity)),
  );
  const failed = settled.find((result) => result.status === "rejected");
  if (failed) throw failed.reason;
}
