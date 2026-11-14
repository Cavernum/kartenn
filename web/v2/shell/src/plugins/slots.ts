import type { ExtensionPoint, ObjectRef } from "@kartenn/sdk";
import type { ExtensionSlotProps, GridColumn, MenuItem, TabItem } from "@kartenn/ui";
import { computed, toValue, type ComputedRef, type MaybeRefOrGetter } from "vue";
import { useSession } from "../rights/index.ts";
import {
  extensionsOf,
  labelOf,
  usePlugins,
  widgetsOf,
  type Placed,
  type Widget,
} from "./registry.ts";

// The places of the plugins in the screens of the new shell (REQ-021-13; design 007
// « Hôtes d'extension », 021 « Écrans »): what a screen asks for the type of the object it
// shows — the tabs and panels of a record, the columns of a list, the actions of the menu
// of an object (the type `changeset` included), the tiles of the home page — read from the
// loaded plugins and the rights, reactive. The screen places them with `ExtensionElement`
// (an element in its place, an error confined to it) and `PluginAction` (an action in its
// dialog). A type of a module (`vlan`, `machines.machine`) is a type like another.

/** The id of a place of an extension, apart from the screen's own (`x.<plugin>.<id>`). */
export function placeId(extension: Pick<Placed, "plugin" | "id">): string {
  return `x.${extension.plugin}.${extension.id}`;
}

/** The extensions of `point` for the type `type` (none without a type), reactive. */
export function useExtensions(
  point: Exclude<ExtensionPoint, "view" | "widget">,
  type: MaybeRefOrGetter<string | null | undefined>,
): ComputedRef<Placed[]> {
  const plugins = usePlugins();
  const session = useSession();
  return computed(() => {
    const of = toValue(type);
    return of ? extensionsOf(plugins.value, point, session.value.rights, of) : [];
  });
}

/** The tabs the plugins add to the record of `type`, after the screen's (`Tabs`). */
export function tabItems(extensions: readonly Placed[], t: (key: string) => string): TabItem[] {
  return extensions.map((e) => ({ value: placeId(e), label: labelOf(t, e) }));
}

/**
 * The columns the plugins add to the lists of `type` (`DataGrid`, `element`) and to the tree
 * of the topography (`TreeGrid`, the prefixes): each cell an element of the plugin, its
 * `row` the representation of the row; `elementProps` of the grid gives it its `object`
 * (`stableObjectRefs`). The screen fills the slot `extension` of the grid with an
 * `ExtensionElement` (`cell`, `cellExtension`): an error of the element stays in its cell,
 * said with its code.
 */
export function gridColumns(
  extensions: readonly Placed[],
  t: (key: string) => string,
): GridColumn[] {
  return extensions.map((e) => ({
    id: placeId(e),
    label: labelOf(t, e),
    element: e.element,
    priority: 3,
  }));
}

/**
 * What `ExtensionElement` (`cell`) takes in a cell of a column of `gridColumns`, from what
 * the slot `extension` of the grid gives: the tag, the plugin, the label of the column, the
 * row and the `object` that `elementProps` gave.
 */
export function cellExtension(extensions: readonly Placed[], given: ExtensionSlotProps) {
  return {
    tag: given.column.element ?? "",
    plugin: placed(extensions, given.column.id)?.plugin ?? "",
    label: given.column.label,
    row: given.row as Record<string, unknown> | undefined,
    object: given.properties.object as ObjectRef | undefined,
  };
}

/** The entries the plugins add to the menu of actions of an object (`Menu`), after its own. */
export function actionItems(extensions: readonly Placed[], t: (key: string) => string): MenuItem[] {
  return extensions.length === 0
    ? []
    : [
        { kind: "separator" },
        ...extensions.map((e) => ({
          id: placeId(e),
          label: labelOf(t, e),
          icon: "plug" as const,
        })),
      ];
}

/** The extension of a place id among `extensions`, if it is one of theirs. */
export function placed(extensions: readonly Placed[], id: string): Placed | undefined {
  return extensions.find((e) => placeId(e) === id);
}

/** The tiles of the plugins on the home page, after the core's (REQ-021-9, 13). */
export function useWidgets(): ComputedRef<Widget[]> {
  const plugins = usePlugins();
  const session = useSession();
  return computed(() => widgetsOf(plugins.value, session.value.rights));
}

/** The `object` an element of a plugin receives for an object of the API. */
export function objectRef(type: string, id: string, href: string): ObjectRef {
  return { type, id, href };
}

/**
 * `objectRef`, the same one for a row from one render to the next: the element of a cell
 * keeps its `object` while the focus moves or the grid renders again, and is created again
 * only when its row changes (`ExtensionElement` creates it again for another row anyway).
 * Held by the row itself (a weak map): a row the screen's data no longer holds (another
 * domain, a filter, a page dropped by the cache) takes its reference with it. Asked only by
 * the cells of an extension column: nothing per row without one.
 */
export function stableObjectRefs(): (
  row: object,
  type: string,
  id: string,
  href: string,
) => ObjectRef {
  const known = new WeakMap<object, ObjectRef>();
  return (row, type, id, href) => {
    let ref = known.get(row);
    if (!ref || ref.type !== type || ref.id !== id || ref.href !== href)
      known.set(row, (ref = objectRef(type, id, href)));
    return ref;
  };
}
