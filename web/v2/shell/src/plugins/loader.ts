import { UI_VERSION } from "@kartenn/ui";
import {
  abandonPlugin,
  addCatalog,
  expectPlugin,
  SDK_VERSION,
  unmetRange,
  type SharedVersions,
} from "@kartenn/sdk/host";
import { version as VUE_VERSION } from "vue";
import { modulesSettled, preloadModules } from "./preload.ts";
import {
  announce,
  serverVersion,
  settle,
  unavailable,
  type Reason,
  type UiPlugin,
} from "./registry.ts";

// Loading of the plugins by the new shell (REQ-007-9, 10; REQ-021-13, 26; design 007
// « Chargement des plugins par le shell », 021 « Préchargement »): after the first screen,
// in the browser's idle time, each plugin of the capabilities in parallel, never holding a
// screen; one that fails is set aside with its reason, never the shell. The modules go by
// the same `modulepreload` as the prefetch on the intent of a link (`preloadModules`): a
// module is never inserted twice. The keys of the shell are reserved before (`main.ts`).

/** How long a plugin has to call `definePlugin` once its modules are fetched. */
export const DEFINE_TIMEOUT_MS = 5_000;

/** The versions the shell shares through its import map: what the ranges are read against. */
export const SHARED: SharedVersions = { sdk: SDK_VERSION, ui: UI_VERSION, framework: VUE_VERSION };

/** A catalog of a plugin: nested keys. */
export type Messages = { [key: string]: string | Messages };

/** Where catalogs of plugins go: the shell's vue-i18n, under the plugin's name. */
export type MergeCatalog = (locale: string, namespace: string, messages: Messages) => void;

class SetAside extends Error {
  constructor(
    readonly reason: Reason,
    readonly detail?: string,
  ) {
    super(reason);
  }
}

async function catalog(plugin: UiPlugin, path: string): Promise<Messages> {
  const response = await fetch(new URL(path, location.origin + plugin.base), {
    integrity: plugin.integrity[path] ?? "",
    credentials: "same-origin",
  }).catch(() => null);
  if (!response?.ok) throw new SetAside("load-failed", path);
  const body = (await response.json().catch(() => null)) as unknown;
  if (typeof body !== "object" || body === null) throw new SetAside("load-failed", path);
  return body as Messages;
}

/** Resolves on `load`, rejects on `error`: the `<script>` of the entry, inserted in `head`. */
function inserted(script: HTMLScriptElement): Promise<void> {
  return new Promise((resolve, reject) => {
    script.addEventListener("load", () => resolve(), { once: true });
    script.addEventListener("error", () => reject(new SetAside("load-failed", script.src)), {
      once: true,
    });
    document.head.append(script);
  });
}

/**
 * The mark (User Timing) set once every plugin of the capabilities has nothing more to fetch
 * to load: its entry loaded (whether `definePlugin` comes or not), or the plugin set aside,
 * and every catalog and module it asked for settled; or the capabilities could not be
 * read. The journeys wait for it before leaving the first screen (task 029-4): Firefox
 * checks the digest of a module load cut short by leaving the page against the bytes it
 * got, none, and reports an integrity error of the old page.
 */
export const PLUGINS_FETCHED = "kartenn:plugins-fetched";

/**
 * Loads one plugin. `fetches` receives each fetch it starts, which the mark waits for while
 * the plugin is set aside at the first that fails; `fetched` is called once its entry is
 * loaded.
 */
async function load(
  plugin: UiPlugin,
  locales: readonly string[],
  merge: MergeCatalog,
  fetches: Promise<unknown>[],
  fetched: () => void,
) {
  // 1. The ranges of the SDK, of @kartenn/ui and of the framework (REQ-021-26).
  const unmet = unmetRange(plugin, SHARED);
  if (unmet) throw new SetAside("incompatible", unmet);

  // 2. Its catalogs in the languages of the shell, under its name.
  const wanted = Object.entries(plugin.catalogs).filter(([locale]) => locales.includes(locale));
  const reading = wanted.map(
    async ([locale, path]) => [locale, await catalog(plugin, path)] as const,
  );
  fetches.push(...reading);
  for (const [locale, messages] of await Promise.all(reading)) {
    merge(locale, plugin.name, messages);
    addCatalog(locale, plugin.name, messages);
  }

  // 3. Every module checked by its digest before an `import` resolves it: the same
  // promises as the prefetch on intent.
  const preloading = preloadModules(plugin);
  fetches.push(modulesSettled(plugin));
  await preloading.catch((error: unknown) => {
    throw new SetAside("load-failed", error instanceof Error ? error.message : undefined);
  });

  // 4. The entry, then `definePlugin` within the delay; an exception thrown from its
  // files while loading sets it aside.
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
        reject(new SetAside("exception", event.message));
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
    const entryLoaded = inserted(script);
    fetches.push(entryLoaded);
    await Promise.race([entryLoaded, thrown]);
    fetched();
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
  let fetching = list.length;
  if (fetching === 0) performance.mark(PLUGINS_FETCHED);
  await Promise.all(
    list.map(async (plugin) => {
      const fetches: Promise<unknown>[] = [];
      let counted = false;
      // Once its entry is loaded or it is set aside, and what it fetched has settled.
      const fetched = () => {
        if (counted) return;
        counted = true;
        void Promise.allSettled(fetches).then(() => {
          if (--fetching === 0) performance.mark(PLUGINS_FETCHED);
        });
      };
      try {
        await load(plugin, locales, merge, fetches, fetched);
        settle(plugin.name, { state: "loaded" });
      } catch (error) {
        abandonPlugin(plugin.name);
        const reason = error instanceof SetAside ? error.reason : "exception";
        const detail = error instanceof SetAside ? error.detail : undefined;
        console.warn(`kartenn: plugin ${plugin.name} ${plugin.version} set aside (${reason})`);
        settle(plugin.name, { state: "rejected", reason, ...(detail ? { detail } : {}) });
      } finally {
        fetched();
      }
    }),
  );
}

/** Readings of the capabilities before giving up: the first and its retries. */
export const CAPABILITY_ATTEMPTS = 4;

/** Longest wait between two readings, whatever `Retry-After` asks. */
const MAX_RETRY_MS = 5_000;

/** How long to wait before reading again after `response`, at the `attempt`-th failure. */
function backoff(response: Response | null, attempt: number): number {
  const asked = Number(response?.headers.get("retry-after"));
  const wait = Number.isFinite(asked) && asked > 0 ? asked * 1_000 : 250 * 2 ** attempt;
  return Math.min(wait, MAX_RETRY_MS);
}

/**
 * The plugins of the capabilities of the caller, or `null` if they cannot be read. A
 * refusal for load (429, 503) or a network failure is read again, `Retry-After` honoured;
 * a body without a list of plugins is a failure, never an empty list.
 */
export async function readPlugins(
  fetcher: typeof fetch = fetch,
  sleep: (ms: number) => Promise<void> = (ms) => new Promise((r) => setTimeout(r, ms)),
): Promise<UiPlugin[] | null> {
  for (let attempt = 0; attempt < CAPABILITY_ATTEMPTS; attempt++) {
    const response = await fetcher("/v1/capabilities").catch(() => null);
    const transient = !response || response.status === 429 || response.status === 503;
    if (response?.ok) {
      const capabilities = (await response.json().catch(() => null)) as {
        ui_plugins?: unknown;
        modules?: { name?: unknown; version?: unknown }[];
      } | null;
      const core = Array.isArray(capabilities?.modules)
        ? capabilities.modules.find((m) => m.name === "core")?.version
        : undefined;
      serverVersion(typeof core === "string" ? core : null);
      return Array.isArray(capabilities?.ui_plugins)
        ? (capabilities.ui_plugins as UiPlugin[])
        : null;
    }
    if (!transient || attempt === CAPABILITY_ATTEMPTS - 1) return null;
    await sleep(backoff(response, attempt));
  }
  return null;
}

/**
 * Reads the capabilities and loads their plugins; when they cannot be read, the registry
 * says so (`unavailable`) and `startPlugins` may be called again (« Réessayer »).
 */
export async function startPlugins(
  locales: readonly string[],
  merge: MergeCatalog,
  fetcher: typeof fetch = fetch,
): Promise<void> {
  unavailable(false);
  const list = await readPlugins(fetcher);
  if (list === null) {
    console.warn("kartenn: the capabilities could not be read; plugins not loaded");
    unavailable(true);
    performance.mark(PLUGINS_FETCHED);
    return;
  }
  await loadPlugins(list, locales, merge);
}

let configured: { locales: readonly string[]; merge: MergeCatalog } | null = null;
let begun = false;

/** Where the catalogs of the plugins go, and in which languages (`main.ts`). */
export function configurePlugins(locales: readonly string[], merge: MergeCatalog): void {
  configured = { locales, merge };
}

/**
 * Loads the plugins once, in an idle moment: when the first screen of a session is drawn,
 * or after a login in place (the capabilities ask for a session).
 */
export function beginPlugins(idle: (start: () => void) => void = whenIdle): void {
  if (begun || !configured) return;
  begun = true;
  const { locales, merge } = configured;
  idle(() => void startPlugins(locales, merge));
}

/** Reads the capabilities again after they could not be (« Réessayer »). */
export function retryPlugins(): Promise<void> {
  return configured ? startPlugins(configured.locales, configured.merge) : Promise.resolve();
}

/** The longest wait for an idle moment before the plugins load anyway. */
export const IDLE_TIMEOUT_MS = 2_000;

/**
 * Runs `start` once the first screen is drawn, in an idle moment of the browser
 * (`requestIdleCallback`, 2 s at most), else on the next task where it is missing.
 */
export function whenIdle(start: () => void): void {
  if (typeof globalThis.requestIdleCallback === "function") {
    globalThis.requestIdleCallback(() => start(), { timeout: IDLE_TIMEOUT_MS });
  } else {
    setTimeout(start, 0);
  }
}
