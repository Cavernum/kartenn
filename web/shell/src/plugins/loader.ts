import { SDK_VERSION } from "@kartenn/sdk";
import { abandonPlugin, addCatalog, expectPlugin, satisfies } from "@kartenn/sdk/host";
import { announce, settle, type Reason, type UiPlugin } from "./registry.ts";

// Loading of the plugins by the shell (REQ-007-9, 10; design « Chargement des plugins par
// le shell »): each plugin of the capabilities in parallel, without holding the first
// screen; one that fails is set aside with its reason, never the shell.

/** How long a plugin has to call `definePlugin` once its modules are fetched. */
export const DEFINE_TIMEOUT_MS = 5_000;

/** A catalog of a plugin: nested keys. */
export type Messages = { [key: string]: string | Messages };

/** Where catalogs of plugins go: the shell's vue-i18n, under the plugin's name. */
export type MergeCatalog = (locale: string, namespace: string, messages: Messages) => void;

class SetAside extends Error {
  constructor(readonly reason: Reason) {
    super(reason);
  }
}

/** Resolves on `load`, rejects on `error`: a `<link>` or `<script>` inserted in `head`. */
function inserted(element: HTMLLinkElement | HTMLScriptElement): Promise<void> {
  return new Promise((resolve, reject) => {
    element.addEventListener("load", () => resolve(), { once: true });
    element.addEventListener("error", () => reject(new SetAside("load-failed")), { once: true });
    document.head.append(element);
  });
}

async function catalog(plugin: UiPlugin, path: string): Promise<Messages> {
  const response = await fetch(new URL(path, location.origin + plugin.base), {
    integrity: plugin.integrity[path] ?? "",
    credentials: "same-origin",
  }).catch(() => null);
  if (!response?.ok) throw new SetAside("load-failed");
  return (await response.json().catch(() => {
    throw new SetAside("load-failed");
  })) as Messages;
}

async function load(plugin: UiPlugin, locales: readonly string[], merge: MergeCatalog) {
  // 1. The range of the SDK.
  if (!satisfies(SDK_VERSION, plugin.sdk)) throw new SetAside("incompatible");

  // 2. Its catalogs in the languages of the shell, under its name.
  const wanted = Object.entries(plugin.catalogs).filter(([locale]) => locales.includes(locale));
  for (const [locale, messages] of await Promise.all(
    wanted.map(async ([locale, path]) => [locale, await catalog(plugin, path)] as const),
  )) {
    merge(locale, plugin.name, messages);
    addCatalog(locale, plugin.name, messages);
  }

  // 3. Every module checked by its digest before an `import` resolves it.
  const modules = Object.entries(plugin.integrity).filter(([path]) => path.endsWith(".js"));
  await Promise.all(
    modules.map(([path, integrity]) => {
      const link = document.createElement("link");
      link.rel = "modulepreload";
      link.href = plugin.base + path;
      link.integrity = integrity;
      return inserted(link);
    }),
  );

  // 4. The entry, then `definePlugin` within the delay; an exception thrown from its
  // files while loading sets it aside (5).
  const entry = plugin.entry.slice(plugin.base.length);
  const defined = expectPlugin({
    name: plugin.name,
    base: plugin.base,
    integrity: plugin.integrity,
    extensions: plugin.extensions,
  });
  let onError: (event: ErrorEvent) => void = () => {};
  const thrown = new Promise<never>((_, reject) => {
    onError = (event) => {
      if (event.filename?.startsWith(location.origin + plugin.base)) {
        reject(new SetAside("exception"));
      }
    };
    window.addEventListener("error", onError);
  });
  // Raced below; never left to reject unobserved.
  thrown.catch(() => {});
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    const script = document.createElement("script");
    script.type = "module";
    script.src = plugin.entry;
    script.integrity = plugin.integrity[entry] ?? "";
    await Promise.race([inserted(script), thrown]);
    const late = new Promise<never>((_, reject) => {
      timer = setTimeout(() => reject(new SetAside("timeout")), DEFINE_TIMEOUT_MS);
    });
    await Promise.race([defined, thrown, late]);
  } finally {
    window.removeEventListener("error", onError);
    clearTimeout(timer);
  }
}

/** Loads each plugin of the list, in parallel; settles each in the registry. */
export async function loadPlugins(
  list: readonly UiPlugin[],
  locales: readonly string[],
  merge: MergeCatalog,
): Promise<void> {
  announce(list);
  await Promise.all(
    list.map(async (plugin) => {
      try {
        await load(plugin, locales, merge);
        settle(plugin.name, { state: "loaded" });
      } catch (error) {
        abandonPlugin(plugin.name);
        const reason = error instanceof SetAside ? error.reason : "exception";
        console.warn(`kartenn: plugin ${plugin.name} ${plugin.version} set aside (${reason})`);
        settle(plugin.name, { state: "rejected", reason });
      }
    }),
  );
}

/** The plugins of the capabilities of the caller; none when signed out or on failure. */
export async function readPlugins(fetcher: typeof fetch = fetch): Promise<UiPlugin[]> {
  const response = await fetcher("/v1/capabilities").catch(() => null);
  if (!response?.ok) return [];
  const capabilities = (await response.json().catch(() => ({}))) as { ui_plugins?: UiPlugin[] };
  return capabilities.ui_plugins ?? [];
}
