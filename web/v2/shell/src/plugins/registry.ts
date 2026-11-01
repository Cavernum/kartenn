import type { components, ExtensionPoint } from "@kartenn/sdk";
import { shallowRef, type ShallowRef } from "vue";
import type { Rights } from "../rights/index.ts";

// The plugins of the new shell (spec 021, REQ-021-13; 007 « Chargement des plugins par le
// shell »): the list of `ui_plugins` and where the loading of each stands, read by the
// frame (entries of the spaces, `g` letters, routes `/x/…`), the hosts of the extension
// points and the page « À propos ». A plugin set aside keeps its reason, said with a code.

/** A plugin as `ui_plugins` of `GET /v1/capabilities` publishes it (REQ-007-7, 021-26). */
export type UiPlugin = components["schemas"]["UiPlugin"];
/** One of its extensions, as published. */
export type UiExtension = components["schemas"]["UiExtension"];

/** Why the shell set a plugin aside (007 design, « Chargement des plugins par le shell »). */
export type Reason = "incompatible" | "load-failed" | "exception" | "timeout";

export type Status =
  | { state: "loading" }
  | { state: "loaded" }
  | { state: "rejected"; reason: Reason; detail?: string };

export interface Loaded {
  plugin: UiPlugin;
  status: Status;
}

/** An extension of a plugin, with the plugin that brings it. */
export interface Placed extends UiExtension {
  plugin: string;
}

/**
 * The code an error of a plugin is shown with (an encart, « À propos »): `plugin.<reason>`
 * for a plugin set aside, `plugin.element` for an element that failed in its place. Codes
 * of the shell, never of the API: no problem of HTTP stands behind them.
 */
export function reasonCode(reason: Reason | "element"): string {
  return `plugin.${reason}`;
}

const plugins: ShallowRef<readonly Loaded[]> = shallowRef([]);
const failed: ShallowRef<boolean> = shallowRef(false);
const read: ShallowRef<boolean> = shallowRef(false);
/** The `g` letters refused to each plugin, by name: the letter and who holds it. */
const refused: ShallowRef<ReadonlyMap<string, readonly RefusedLetter[]>> = shallowRef(new Map());

export interface RefusedLetter {
  /** The id of the view that asked for it. */
  view: string;
  letter: string;
}

/** The plugins of the capabilities and where their loading stands, reactive. */
export function usePlugins(): Readonly<ShallowRef<readonly Loaded[]>> {
  return plugins;
}

/** Whether the last reading of the capabilities failed: the plugins are then unknown. */
export function usePluginsUnavailable(): Readonly<ShallowRef<boolean>> {
  return failed;
}

/** The letters refused to the plugins, reactive (« À propos » says them). */
export function useRefusedLetters(): Readonly<
  ShallowRef<ReadonlyMap<string, readonly RefusedLetter[]>>
> {
  return refused;
}

const server: ShallowRef<string | null> = shallowRef(null);

/** The version of the core of the server, as the capabilities said it (« À propos »). */
export function useServerVersion(): Readonly<ShallowRef<string | null>> {
  return server;
}

/** Records the version of the core of the server. */
export function serverVersion(version: string | null): void {
  server.value = version;
}

/** Records whether the capabilities could be read. */
export function unavailable(value: boolean): void {
  failed.value = value;
  if (value) read.value = true;
}

/**
 * Whether the capabilities were read (or found unreadable): before, a view of a plugin
 * opened by its URL is still loading, not missing.
 */
export function useCapabilitiesRead(): Readonly<ShallowRef<boolean>> {
  return read;
}

/** Replaces the list (a new reading of the capabilities). */
export function announce(list: readonly UiPlugin[]): void {
  plugins.value = list.map((plugin) => ({ plugin, status: { state: "loading" } }));
  read.value = true;
}

/** Records the outcome of a plugin. */
export function settle(name: string, status: Status): void {
  plugins.value = plugins.value.map((entry) =>
    entry.plugin.name === name ? { ...entry, status } : entry,
  );
}

/** Records the letters refused to the plugins, replacing the last record. */
export function refuseLetters(byPlugin: ReadonlyMap<string, readonly RefusedLetter[]>): void {
  refused.value = byPlugin;
}

/** Forgets every plugin (tests). */
export function resetPlugins(): void {
  plugins.value = [];
  failed.value = false;
  read.value = false;
  refused.value = new Map();
}

/** Whether `rights` may be shown `extension` (its permission, if any: a hint only). */
export function allowed(extension: Pick<UiExtension, "permission">, rights: Rights): boolean {
  return !extension.permission || rights.has(extension.permission);
}

/**
 * The extensions of `point` that may show (REQ-007-13, REQ-021-13): from a loaded plugin,
 * for the type of the object if one is given (`changeset`, `vlan`, `machines.machine`…),
 * and whose permission, if any, is held. The server stays the judge of what the element
 * then asks.
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
          .filter((e) => allowed(e, rights))
          .map((e) => ({ ...e, plugin: plugin.name })),
  );
}

/** A tile of the home page: the widget of a plugin, and where its plugin stands. */
export interface Widget {
  extension: Placed;
  status: Status;
}

/**
 * The tiles of the home page (REQ-021-13, 021/X38): one per plugin at most (the first it
 * declares; the server refuses a second), shown by its permission, in the order of the
 * plugins; a plugin still loading has its tile already (a skeleton), one set aside its
 * error, so that the home page does not jump when it settles.
 */
export function widgetsOf(loaded: readonly Loaded[], rights: Rights): Widget[] {
  return loaded.flatMap(({ plugin, status }) => {
    const extension = plugin.extensions.find((e) => e.point === "widget");
    if (!extension || !allowed(extension, rights)) return [];
    return [{ extension: { ...extension, plugin: plugin.name }, status }];
  });
}

/** The namespace of a plugin's catalogs in the shell's: apart from the shell's own keys. */
export const CATALOG_ROOT = "plugin";

/** The key of a label of a plugin in the shell's catalogs (W11): under its name. */
export function pluginKey(plugin: string, key: string): string {
  return `${CATALOG_ROOT}.${plugin}.${key}`;
}

/** The key of the label of an extension: its `label`, else its `nav`, else its id. */
export function extensionLabel(extension: Placed): string {
  return pluginKey(extension.plugin, extension.label ?? extension.nav ?? extension.id);
}

/**
 * The label of an extension in the language of `t`; before its catalogs are merged (a
 * plugin loading, or set aside), its plugin and its id, never a bare key of the shell.
 */
export function labelOf(t: (key: string) => string, extension: Placed): string {
  const key = extensionLabel(extension);
  const text = t(key);
  return text === key ? `${extension.plugin} · ${extension.id}` : text;
}

/** The path of a view of a plugin (`/x/<plugin>/<id>`). */
export function viewPath(plugin: string, id: string): string {
  return `/x/${encodeURIComponent(plugin)}/${encodeURIComponent(id)}`;
}
