// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Registration of the extensions (REQ-007-13, W5): the shell announces each plugin it is
// about to load (`expectPlugin`), then the plugin's module calls `definePlugin`, which
// defines only the elements its manifest declares (design « Chargement des plugins »).

import { format, translate } from "./i18n.ts";

/** The extension points (W5). */
export type ExtensionPoint = "view" | "tab" | "panel" | "column" | "action";

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
  extensions: Extension[];
}

/** What a plugin gets back: its translations, formats and style sheets. */
export interface PluginHandle {
  /** The key under the namespace of the plugin (`<name>.<key>`). */
  t(key: string, params?: Record<string, unknown>): string;
  format: { dateTime(iso: string): string; number(n: number): string; timeZone: string };
  /** Adds a style sheet of the plugin to a shadow root, with its digest. */
  styles(root: ShadowRoot, path: string): Promise<void>;
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
    t: (key, params) => translate(`${manifest.name}.${key}`, params),
    format,
    styles(root, path) {
      const integrity = manifest.integrity[path];
      if (!integrity) return Promise.reject(new Error(`${path}: no digest in the manifest`));
      const link = root.ownerDocument.createElement("link");
      link.rel = "stylesheet";
      link.href = manifest.base + path;
      link.integrity = integrity;
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
