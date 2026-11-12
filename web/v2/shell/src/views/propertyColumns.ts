import { keys, qualify, queryClient, reads, type components, type Entry } from "@kartenn/sdk";
import type { CellRef, CellState, GridColumn, GridCommit, GridWindow, RowState } from "@kartenn/ui";
import { computed, shallowRef, type ComputedRef } from "vue";
import { useQueries } from "../data/query.ts";
import { useTime } from "../time/index.ts";
import { nameOf, type Collection, type Item } from "./collections.ts";
import type { NotEditable } from "./edit/fields.ts";
import { whyWords, type InlineDeps, type InlineEdit } from "./edit/inline.ts";
import {
  propertyText,
  usePropertyEdit,
  type Declared,
  type PropertyRead,
} from "./edit/properties.ts";
import type { DeclarationOfModel } from "./filters.ts";
import { limited } from "./parts/vlan.ts";
import { useViewColumns, type ColumnVisibility } from "./viewSettings.ts";

// The columns of properties of a list (task 12i, Q-021-18): none shown by default; the
// chooser « Colonnes » of the grid offers every property declared for the kind of the list,
// and the choice is kept by view and by user (`viewSettings.ts`). A column chosen reads the
// resolved properties of its rows (`/v1/<kind>/{id}/properties`, the cache the record's tab
// and the VLAN column share) for the rows in view and a screen around them, a few at a time
// (`limited`). Its cells are edited in place by the rules of the tab (`usePropertyEdit`,
// task 12h): `prop.<key>.edit` alone, the draft guarded by an `expect` (Q-021-21), Ctrl+Enter
// with `prop.<key>.approval.self`, a value inherited overridden and said so (Q-021-19).

type Resolved = components["schemas"]["Resolved"];

/** The key of the property a column of a list shows (`prop.<key>`), `null` for another. */
export function propertyOfColumn(column: string): string | null {
  return column.startsWith("prop.") ? column.slice("prop.".length) : null;
}

/** The declarations whose properties an object of the qualified type `type` may carry. */
export function declaredFor<D extends Pick<DeclarationOfModel, "key" | "carriers">>(
  declarations: readonly D[],
  type: string,
): D[] {
  return declarations
    .filter((d) => d.carriers.length === 0 || d.carriers.some((c) => qualify(c) === type))
    .sort((a, b) => a.key.localeCompare(b.key));
}

/**
 * The columns of the properties declared, after those of the list: by label, the key when
 * the property has none; of priority 3, so that the chooser lists them (it shows the
 * columns that may hide).
 */
export function propertyGridColumns(
  declared: readonly Pick<DeclarationOfModel, "key">[],
  label: (key: string) => string | null,
): GridColumn[] {
  const labels = declared.map((d) => label(d.key));
  // Two properties of the same label told apart by their keys (REQ-020-21).
  const shared = (one: string | null) => one !== null && labels.filter((l) => l === one).length > 1;
  return declared.map((d, i) => ({
    id: `prop.${d.key}`,
    label: labels[i] === null ? d.key : shared(labels[i]!) ? `${labels[i]} (${d.key})` : labels[i]!,
    width: "md",
    priority: 3,
  }));
}

/** The choice of the columns, a column of a property not chosen hidden (none by default). */
export function withPropertiesHidden(
  visibility: ColumnVisibility,
  columns: readonly GridColumn[],
): ColumnVisibility {
  const out = { ...visibility };
  for (const column of columns) {
    if (propertyOfColumn(column.id) !== null && out[column.id] === undefined)
      out[column.id] = false;
  }
  return out;
}

/** How many rows are read before the grid says what it shows. */
export const FIRST_READ = 20;

/** The rows whose properties are read: those in view and a screen around, the focus's too. */
export function rowsToRead(
  ids: readonly string[],
  view: GridWindow | null,
  focus?: string,
): string[] {
  const size = view && view.size > 0 ? view.size : FIRST_READ;
  const start = view && view.size > 0 ? Math.max(0, view.start - size) : 0;
  const end = view && view.size > 0 ? view.end + size : FIRST_READ;
  const out = ids.slice(start, end);
  if (focus && !out.includes(focus) && ids.includes(focus)) out.push(focus);
  return out;
}

/** What a cell of a column of a property shows. */
export type PropertyCellState =
  | { kind: "reading" }
  | { kind: "failed"; error: unknown }
  | { kind: "masked" }
  | { kind: "value"; value: unknown };

export interface PropertyColumnsDeps extends Pick<InlineDeps, "t" | "sentence" | "say"> {
  collection: () => Collection;
  /** The rows of the list as read (their versions), and as the fields edited show them. */
  rows: () => readonly Item[];
  inline: InlineEdit;
  declarations: () => readonly DeclarationOfModel[];
  /** The cell of the focus of the grid. */
  active: () => CellRef | undefined;
  /** The label of a property, `null` for its key alone. */
  label: (key: string) => string | null;
  /** The label of a value of the closed list of a property, if any. */
  valueLabel: (key: string, value: string) => string | null;
}

export function usePropertyColumns(deps: PropertyColumnsDeps) {
  const time = useTime();
  const kind = () => deps.collection().kind;
  const kept = useViewColumns(() => deps.collection().id);
  const declared = computed(() =>
    declaredFor(deps.declarations(), deps.collection().type),
  ) as ComputedRef<DeclarationOfModel[]>;
  const columns = computed(() => propertyGridColumns(declared.value, deps.label));
  /** The choice of the columns of the grid (`v-model:columnVisibility`), kept at each change. */
  const visibility = computed<ColumnVisibility>({
    get: () => withPropertiesHidden(kept.value, columns.value),
    set: (next) => (kept.value = next),
  });
  /** The keys of the properties whose column is shown. */
  const chosen = computed(() =>
    columns.value
      .filter((column) => visibility.value[column.id] === true)
      .map((column) => propertyOfColumn(column.id)!),
  );

  // The resolved properties of the rows in view, while a column of a property shows.
  const view = shallowRef<GridWindow | null>(null);
  const ids = computed(() =>
    chosen.value.length === 0
      ? []
      : rowsToRead(
          deps.rows().map((row) => String(row.id)),
          view.value,
          deps.active()?.row || undefined,
        ),
  );
  const optionsOf = (id: string) =>
    reads.sub<Resolved>(kind(), id, "properties", null, {
      validAt: time.value.validAt,
      asOf: time.value.asOf,
    });
  const results = useQueries(() =>
    ids.value.map((id) => {
      const read = optionsOf(id);
      return {
        ...read,
        queryFn: (context: Parameters<typeof read.queryFn>[0]) =>
          limited(context.signal, () => read.queryFn(context)),
      };
    }),
  );
  const byId = computed(() => {
    const out = new Map<string, (typeof results.value)[number]>();
    ids.value.forEach((id, i) => {
      const result = results.value[i];
      if (result) out.set(id, result);
    });
    return out;
  });
  const resolvedOf = (id: string) =>
    (byId.value.get(id)?.data as Entry<Resolved> | undefined)?.data ??
    (queryClient.getQueryData(optionsOf(id).queryKey) as Entry<Resolved> | undefined)?.data;

  /** The property `key` of the row `id` as read, `undefined` while it is not. */
  function readOf(id: string, key: string): PropertyRead | undefined {
    const resolved = resolvedOf(id);
    if (!resolved) return undefined;
    if (resolved.masked.includes(key)) return { value: "", read: null, masked: true };
    const value = (resolved.properties as Record<string, unknown>)[key];
    return value === undefined || value === null
      ? { value: "", read: null, masked: false }
      : { value: propertyText(value), read: value, masked: false };
  }

  const declarationOf = (key: string) =>
    declared.value.find((d) => d.key === key) as Declared | undefined;
  const objects = computed(() =>
    deps.rows().map((row) => ({
      id: String(row.id),
      name: nameOf(row),
      version: typeof row.version === "string" ? row.version : null,
    })),
  );
  const edit = usePropertyEdit({
    collection: deps.collection,
    objects: () => objects.value,
    property: readOf,
    unreadable: (id) => byId.value.get(id)?.isError === true,
    declaration: declarationOf,
    declarationById: (id) =>
      (deps.declarations() as unknown as Declared[]).find((d) => d.id === id),
    label: (key) => deps.label(key) ?? key,
    valueLabel: deps.valueLabel,
    // The row read again: its properties and its version, by the list.
    reread: (id) => {
      void queryClient.invalidateQueries({ queryKey: keys.allOf(kind(), id) });
      void queryClient.invalidateQueries({ queryKey: keys.allOn(deps.collection().path) });
    },
    t: deps.t,
    sentence: deps.sentence,
    say: deps.say,
  });

  /** What the cell of `key` of `row` shows. */
  function cellOf(row: Item, key: string): PropertyCellState {
    const id = String(row.id);
    const shown = edit.shownValue(id, key);
    if (shown) return shown.masked ? { kind: "masked" } : { kind: "value", value: shown.read };
    const result = byId.value.get(id);
    return result?.isError ? { kind: "failed", error: result.error } : { kind: "reading" };
  }

  // The grid of the list: the fields by `useInlineEdit`, the properties by `usePropertyEdit`.
  const { inline } = deps;
  const property = (column: GridColumn | string) =>
    propertyOfColumn(typeof column === "string" ? column : column.id);
  /** The rows as the grid shows them: a column of a property holds the text of its editor. */
  const shownRows = computed<Item[]>(() =>
    chosen.value.length === 0
      ? inline.shownRows.value
      : inline.shownRows.value.map((row) => {
          const out: Item = { ...row };
          for (const key of chosen.value) {
            out[`prop.${key}`] = edit.shownValue(String(row.id), key)?.value ?? "";
          }
          return out;
        }),
  );
  /** Why the cell of `column` of the row `row` offers no editor, `null` when it does. */
  const why = (column: string, row: string): NotEditable | null => {
    const key = property(column);
    return key === null ? inline.why(column) : edit.why(row, key);
  };
  function rowState(row: Item): RowState | undefined {
    const fields = inline.rowState(row);
    const properties = edit.rowState(String(row.id));
    if (fields === "review" || properties === "review") return "review";
    return fields ?? properties;
  }
  function rowReason(row: Item): string[] | undefined {
    const reasons = [...(inline.rowReason(row) ?? []), ...edit.rowReasons(String(row.id))];
    return reasons.length > 0 ? [...new Set(reasons)] : undefined;
  }
  /** The line of the motive open, of a field or of a property: one at a time. */
  const asking = computed(() => inline.asking.value ?? edit.asking.value);

  return {
    columns,
    visibility,
    chosen,
    cellOf,
    declarationOf,
    shownRows,
    /** The window of the grid: the rows whose properties are read. */
    onWindow: (window: GridWindow) => (view.value = window),
    editable: (row: Item, column: GridColumn) => {
      const key = property(column);
      return key === null ? inline.editable(row, column) : edit.why(String(row.id), key) === null;
    },
    readonlyReason: (row: Item, column: GridColumn) =>
      whyWords(why(column.id, String(row.id)), deps.t),
    validate: (row: Item, column: GridColumn, text: string) => {
      const key = property(column);
      return key === null
        ? inline.validate(row, column, text)
        : edit.validate(String(row.id), key, text);
    },
    cellState: (row: Item, column: GridColumn): CellState | undefined => {
      const key = property(column);
      return key === null ? inline.cellState(row, column) : edit.cellState(String(row.id), key);
    },
    stateReason: (row: Item, column: GridColumn) => {
      const key = property(column);
      return key === null ? inline.stateReason(row, column) : edit.heldOf(String(row.id), key);
    },
    rowState,
    rowReason,
    why,
    /** The permission Ctrl+Enter lacks on the cell, `null` when held. */
    applyLacks: (column: string, applyRight: string): string | null => {
      const key = property(column);
      if (key === null) return inline.canApply.value ? null : applyRight;
      return edit.mayApply(key) ? null : edit.applyRight(key);
    },
    /** Why the cell is held in review, said at the foot too. */
    heldOf: (row: Item, column: string) => {
      const key = property(column);
      return key === null
        ? inline.stateReason(row, { id: column, label: "" })
        : edit.heldOf(String(row.id), key);
    },
    /** What an edit of the cell writes, said before it is sent (a value over an inherited one). */
    noteOf: (column: string, row: string) => {
      const key = property(column);
      return key === null ? undefined : edit.noteOf(row, key);
    },
    /** The cell of the focus: the origin of its value and its list are read. */
    focus: (cell: CellRef | undefined) => {
      const key = cell && cell.row !== "" ? property(cell.column) : null;
      edit.focus.value = key === null ? null : { id: cell!.row, key };
    },
    onCommit: (commit: GridCommit) => {
      const key = property(commit.column);
      if (key === null) inline.onCommit(commit);
      else void edit.onCommit(commit, { id: commit.row, key });
    },
    asking,
    confirm: (motive: string, text: string) =>
      inline.asking.value ? inline.confirm(motive, text) : edit.confirm(motive, text),
    cancel: () => (inline.asking.value ? inline.cancel() : edit.cancel()),
    /** The row and the column of the cell of the line of the motive. */
    cellOfAsk: () => {
      const ask = asking.value;
      return ask ? { row: ask.row, column: ask.column } : undefined;
    },
  };
}
