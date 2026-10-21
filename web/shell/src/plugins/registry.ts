import type { Extension, ExtensionPoint } from "@kartenn/sdk";
import { shallowRef, type ShallowRef } from "vue";
import type { Rights } from "../rights/permissions.ts";

/** A plugin as `ui_plugins` of `GET /v1/capabilities` publishes it (REQ-007-7). */
export interface UiPlugin {
  name: string;
  version: string;
  sdk: string;
  base: string;
  entry: string;
  integrity: Record<string, string>;
  catalogs: Record<string, string>;
  extensions: Extension[];
}

/** Why the shell set a plugin aside (design « Chargement des plugins par le shell »). */
export type Reason = "incompatible" | "load-failed" | "exception" | "timeout";

export type Status =
  { state: "loading" } | { state: "loaded" } | { state: "rejected"; reason: Reason };

export interface Loaded {
  plugin: UiPlugin;
  status: Status;
}

/** An extension of a loaded plugin, with the plugin that brings it. */
export interface Placed extends Extension {
  plugin: string;
}

const plugins: ShallowRef<readonly Loaded[]> = shallowRef([]);

/** The plugins of the capabilities and where their loading stands, reactive. */
export function usePlugins(): Readonly<ShallowRef<readonly Loaded[]>> {
  return plugins;
}

/** Replaces the list (a new reading of the capabilities). */
export function announce(list: readonly UiPlugin[]): void {
  plugins.value = list.map((plugin) => ({ plugin, status: { state: "loading" } }));
}

/** Records the outcome of a plugin. */
export function settle(name: string, status: Status): void {
  plugins.value = plugins.value.map((entry) =>
    entry.plugin.name === name ? { ...entry, status } : entry,
  );
}

/**
 * The extensions of `point` that may show (REQ-007-13): from a loaded plugin, for the type
 * of the object if one is given, and whose permission, if any, is held. Holding is a hint
 * only: the server stays the judge of what the element then asks.
 */
export function extensionsOf(
  loaded: readonly Loaded[],
  point: ExtensionPoint,
  rights: Rights,
  objectType?: string,
): Placed[] {
  return loaded.flatMap(({ plugin, status }) =>
    status.state !== "loaded"
      ? []
      : plugin.extensions
          .filter((e) => e.point === point)
          .filter((e) => objectType === undefined || (e.object_types ?? []).includes(objectType))
          .filter((e) => !e.permission || rights.has(e.permission))
          .map((e) => ({ ...e, plugin: plugin.name })),
  );
}

/** The key of a label of a plugin in the shell's catalogs: under its name (W11). */
export function pluginKey(extension: Placed, key: string | undefined): string {
  return `${extension.plugin}.${key ?? extension.id}`;
}
