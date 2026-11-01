// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Registration of the extensions (REQ-007-13, W5): the shell announces each plugin it is
// about to load (`expectPlugin`), then the plugin's module calls `definePlugin`, which
// defines only the elements its manifest declares (design « Chargement des plugins »).

import { format, translate } from "./i18n.ts";

/**
 * The extension points (W5), and the tile of the home page (`widget`, spec 021,
 * REQ-021-13: at most one per plugin, read-only, on no object).
 */
export type ExtensionPoint = "view" | "tab" | "panel" | "column" | "action" | "widget";

/** An extension, as `ui_plugins` of the capabilities publishes it. */
export interface Extension {
  point: ExtensionPoint;
  id: string;
  element: string;
  object_types?: string[];
  label?: string;
  nav?: string;
  permission?: string;
  /** The letter of navigation after `g` a view proposes (spec 021). */
  go?: string;
}

/** What the shell knows of a plugin before loading it. */
export interface PluginManifest {
  name: string;
  /** URL prefix of its files, `/plugins/<name>/<version>/`. */
  base: string;
  /** SRI digest by file relative to `base`. */
  integrity: Record<string, string>;
  /** Its extensions, as `ui_plugins` publishes them: the elements it may define. */
  extensions: readonly Pick<Extension, "element">[];
}

/** What a plugin gets back: its translations, formats and style sheets. */
export interface PluginHandle {
  /** The key under the namespace of the plugin (`<name>.<key>`). */
  t(key: string, params?: Record<string, unknown>): string;
  format: { dateTime(iso: string): string; number(n: number): string; timeZone: string };
  /**
   * Adds a style sheet to a shadow root, with its digest: a file of the plugin (`path`
   * relative to its directory), or a sheet the shell shares, named by its specifier
   * (`@kartenn/ui/style.css`, REQ-021-27). Sheets apply in the order of the calls.
   */
  styles(root: ShadowRoot, path: string): Promise<void>;
}

/**
 * The sheets the shell shares with the shadow roots of the plugins: the `<link>` of its
 * page that carries the specifier in `data-kartenn-sheet` (written by its build, with the
 * digest of the file).
 */
export const SHARED_SHEETS: readonly string[] = ["@kartenn/ui/style.css"];

/** The `href`, `integrity` and `crossorigin` of a sheet: the plugin's, or the shell's. */
function sheetOf(
  manifest: PluginManifest,
  document: Document,
  path: string,
): Pick<HTMLLinkElement, "href" | "integrity" | "crossOrigin"> {
  if (SHARED_SHEETS.includes(path)) {
    const shared = [...document.querySelectorAll<HTMLLinkElement>("link[data-kartenn-sheet]")].find(
      (link) => link.dataset["kartennSheet"] === path,
    );
    if (!shared?.integrity) throw new Error(`${path}: not linked by the shell`);
    return { href: shared.href, integrity: shared.integrity, crossOrigin: shared.crossOrigin };
  }
  const integrity = manifest.integrity[path];
  if (!integrity) throw new Error(`${path}: no digest in the manifest`);
  return { href: manifest.base + path, integrity, crossOrigin: null };
}

export interface PluginDefinition {
  name: string;
  elements: Record<string, CustomElementConstructor>;
}

interface Expected {
  manifest: PluginManifest;
  defined: (handle: PluginHandle) => void;
  state: "waiting" | "defined" | "abandoned";
}

const expected = new Map<string, Expected>();

function handleOf(manifest: PluginManifest): PluginHandle {
  return {
    t: (key, params) => translate(`${manifest.name}.${key}`, params, manifest.name),
    format,
    styles(root, path) {
      let sheet: ReturnType<typeof sheetOf>;
      try {
        sheet = sheetOf(manifest, root.ownerDocument, path);
      } catch (error) {
        return Promise.reject(error as Error);
      }
      const link = root.ownerDocument.createElement("link");
      link.rel = "stylesheet";
      // The same request mode as the shell's link: the sheet comes from the cache.
      if (sheet.crossOrigin !== null) link.crossOrigin = sheet.crossOrigin;
      link.href = sheet.href;
      link.integrity = sheet.integrity;
      return new Promise((resolve, reject) => {
        link.addEventListener("load", () => resolve(), { once: true });
        link.addEventListener("error", () => reject(new Error(`${path}: not loaded`)), {
          once: true,
        });
        root.append(link);
      });
    },
  };
}

/**
 * Called by the module of a plugin: defines its elements, all or none. Refused for an
 * unannounced plugin, an element its manifest does not declare, or an element already
 * defined. A call after the shell gave up on the plugin (timeout) is ignored.
 */
export function definePlugin(definition: PluginDefinition): PluginHandle {
  const entry = expected.get(definition.name);
  if (!entry) throw new Error(`plugin ${definition.name} was not announced by the shell`);
  const handle = handleOf(entry.manifest);
  if (entry.state === "abandoned") return handle;
  if (entry.state === "defined") throw new Error(`plugin ${definition.name} is already defined`);
  const declared = new Set(entry.manifest.extensions.map((e) => e.element));
  for (const tag of Object.keys(definition.elements)) {
    if (!declared.has(tag)) {
      throw new Error(`${tag} is not declared by the manifest of ${definition.name}`);
    }
    if (customElements.get(tag)) throw new Error(`${tag} is already defined`);
  }
  for (const [tag, constructor] of Object.entries(definition.elements)) {
    customElements.define(tag, constructor);
  }
  entry.state = "defined";
  entry.defined(handle);
  return handle;
}

/** The shell's side: the plugin is about to load; resolves when it is defined. */
export function expectPlugin(manifest: PluginManifest): Promise<PluginHandle> {
  return new Promise((resolve) => {
    expected.set(manifest.name, { manifest, defined: resolve, state: "waiting" });
  });
}

/** The shell gave up on a plugin (failure, timeout): a later `definePlugin` is ignored. */
export function abandonPlugin(name: string): void {
  const entry = expected.get(name);
  if (entry && entry.state === "waiting") entry.state = "abandoned";
}
