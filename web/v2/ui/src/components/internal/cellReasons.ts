// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The reasons of the cells of a grid (`DataGrid`, `TreeGrid`; tasks 021-12e, 12m, 12f):
// why a cell is in its state (`stateReason`: a write held in review), why a cell of an
// editable grid offers no editor (`readonlyReason`), and, for `DataGrid`, why its row is
// in its state (`rowReason`). Each distinct sentence of the rows in the document is said
// once, hidden, the description of its cells (`aria-describedby`), never in their name;
// and in one tooltip for the grid, decorative (the description says it): under the
// pointer after the delay, at the keyboard focus at once, on a tap or a click until a
// press elsewhere; hoverable, never over an editor nor beside an error's bubble (WCAG
// 1.4.13, REQ-021-37). The sentences sit outside the rows: nothing here moves a row's
// element (`virtualRows.ts`, guarded rows).

import { computed, onBeforeUnmount, ref, watch, type Ref } from "vue";
import type { CellRef, GridColumn } from "../DataGrid/grid.ts";
import { TOOLTIP_DELAY, useTooltip } from "../Tooltip/tooltip.ts";

/** The sentences of a cell, each once, in order: the same text twice is one sentence. */
export const notesOfTexts = (texts: readonly (string | undefined)[]): string[] => [
  ...new Set(texts.filter((text): text is string => !!text)),
];

/** What a grid gives the reasons of its cells. */
export interface CellReasonsHost<R> {
  /** Some reason may be said (the host gave a reason function). */
  enabled(): boolean;
  /** The sentences the cell says of itself besides its value, each once. */
  notesOf(row: R, column: GridColumn): string[];
  /** The rows in the document, and the columns rendered. */
  rendered(): Iterable<R>;
  columns(): readonly GridColumn[];
  rowOf(key: string): R | undefined;
  columnOf(id: string): GridColumn | undefined;
  /** No tooltip on the cell: its editor open (it would cover it), or its error's bubble. */
  quiet(row: R, column: GridColumn): boolean;
  /** The cell of the focus in the grid (`row\ncolumn`, `useCellErrors`). */
  focused: Ref<string | null>;
  editing: Ref<CellRef | undefined>;
  grid: Ref<HTMLElement | null>;
  /** The prefix of the ids of the hidden sentences. */
  idPrefix: string;
  /** Where a click does not pin the tooltip: an editor, a widget of the cell. */
  widgets: string;
}

export function useCellReasons<R>(host: CellReasonsHost<R>) {
  const reasons = computed<string[]>(() => {
    if (!host.enabled()) return [];
    const found = new Set<string>();
    const columns = host.columns();
    for (const row of host.rendered()) {
      for (const column of columns) for (const text of host.notesOf(row, column)) found.add(text);
    }
    // The sentences no row in the document says any more leave the table of ids: it holds
    // those of the rows rendered, never every sentence the grid ever said (task 021-22o-b).
    for (const text of numbers.keys()) if (!found.has(text)) numbers.delete(text);
    return [...found];
  });
  /**
   * The id of the hidden sentence `text`: the same while a row in the document says it,
   * whatever the rows rendered around it (task 021-22e-b: a row that stays in view keeps
   * its description as the list scrolls, and so does not render again). A sentence gone
   * and said again takes a new number, never one another sentence had.
   */
  const numbers = new Map<string, number>();
  let next = 0;
  function reasonId(text: string): string {
    let n = numbers.get(text);
    if (n === undefined) {
      n = next++;
      numbers.set(text, n);
    }
    return `${host.idPrefix}-reason-${n}`;
  }
  /**
   * The ids of the sentences of a cell, its description; nothing without one. A cell is
   * rendered with its row, whose sentences `reasons` holds: never an id without its text.
   */
  function reasonOf(row: R, column: GridColumn): string | undefined {
    const ids = host.enabled() ? host.notesOf(row, column).map(reasonId) : [];
    return ids.length ? ids.join(" ") : undefined;
  }

  const tip = useTooltip();
  const text = ref("");
  /** The tooltip moving to the cell the pointer entered, its delay running (`enter`). */
  let switching: ReturnType<typeof setTimeout> | undefined;
  /**
   * Escape closes it, wherever the focus is, and does nothing else (WCAG 1.4.13): neither
   * the grid nor a layer around it hears the key (heard first, window, capture), even
   * under the cell's focus: no `passEscape`, the grid's Escape being its own (an error's
   * bubble, an editor).
   */
  const closeTip = (event: KeyboardEvent) => {
    if (event.key !== "Escape" || !tip.open.value) return;
    event.stopPropagation();
    event.preventDefault();
    clearTimeout(switching);
    tip.hide();
  };
  watch(tip.open, (now) =>
    now
      ? window.addEventListener("keydown", closeTip, { capture: true })
      : window.removeEventListener("keydown", closeTip, { capture: true }),
  );
  onBeforeUnmount(() => {
    window.removeEventListener("keydown", closeTip, { capture: true });
    clearTimeout(switching);
  });

  /** What the tooltip of a cell says: its sentences, unless quiet. */
  const tipOf = (row: R, column: GridColumn): string =>
    host.quiet(row, column) ? "" : host.notesOf(row, column).join(" ");
  function cellTip(el: HTMLElement | null | undefined): string {
    const row = el?.dataset.row === undefined ? undefined : host.rowOf(el.dataset.row);
    const column = el?.dataset.column === undefined ? undefined : host.columnOf(el.dataset.column);
    return row !== undefined && column ? tipOf(row, column) : "";
  }

  /** The pointer entered a cell: its reason after the delay (REQ-021-37: hover and focus). */
  function enter(row: R, column: GridColumn, event: PointerEvent): void {
    if (event.pointerType === "touch" || !tipOf(row, column)) return;
    const el = event.currentTarget as HTMLElement;
    const show = (delayed: boolean) => {
      const now = tipOf(row, column);
      if (!now) return;
      text.value = now;
      tip.show(el, delayed);
    };
    // Open on another cell, it stays there and moves after the delay, the pointer still
    // here: on its way to the bubble, the pointer crosses the cells under it (1.4.13,
    // hoverable).
    clearTimeout(switching);
    if (tip.open.value && tip.anchor.value !== el) {
      tip.hold();
      switching = setTimeout(() => {
        if (el.matches(":hover")) show(false);
      }, TOOLTIP_DELAY);
    } else show(true);
  }
  /**
   * The pointer over a widget of the cell (the « + » of a node): that widget has its own
   * tooltip, and the cell's leaves, never two at once (WCAG 1.4.13).
   */
  function over(event: PointerEvent): void {
    if (event.pointerType === "touch") return;
    if (!(event.target as HTMLElement | null)?.closest(host.widgets)) return;
    clearTimeout(switching);
    tip.hide();
  }
  /** The pointer left a cell. A finger leaves its cell as it lifts, before the pinning click. */
  function leave(event: PointerEvent): void {
    clearTimeout(switching);
    if (event.pointerType !== "touch") tip.hide(true);
  }
  /** A tap (or a click) on a cell says why at once, until a press elsewhere. */
  function pin(target: HTMLElement): void {
    const el = target.closest<HTMLElement>("[data-column]");
    if (!el || el.dataset.row === undefined) return;
    if (target.closest(host.widgets)) return;
    const now = cellTip(el);
    if (!now) return;
    text.value = now;
    tip.pin(el);
  }

  // What the open tooltip says follows its cell: a reason that changes (a write held, then
  // applied) or goes (an error instead) changes or closes it, whatever opened it.
  const anchorTip = computed(() => cellTip(tip.anchor.value));
  watch(anchorTip, (now) => {
    if (!tip.open.value) return;
    if (now) text.value = now;
    else tip.hide();
  });
  // The keyboard focus on a cell shows its reason at once (`:focus-visible`: a click
  // focuses without it), as its reason comes (a write held after its commit); the focus
  // gone from the cell of the tooltip, it closes, unless the pointer keeps it.
  const focusTip = computed(() => {
    const [r, c] = host.focused.value?.split("\n") ?? [];
    const row = r === undefined ? undefined : host.rowOf(r);
    const column = c === undefined ? undefined : host.columnOf(c);
    return row !== undefined && column ? tipOf(row, column) : "";
  });
  watch(
    [host.focused, focusTip],
    ([key, now]) => {
      const el = document.activeElement as HTMLElement | null;
      const cell =
        key && el?.matches("[data-column]:focus-visible") && host.grid.value?.contains(el)
          ? el
          : null;
      if (cell && now) {
        clearTimeout(switching);
        text.value = now;
        tip.show(cell);
        return;
      }
      const at = tip.anchor.value;
      if (!at || !tip.open.value) return;
      const bubble = document.getElementById(tip.id);
      const hovered = at.matches(":hover") || !!bubble?.matches(":hover");
      // Its reason gone under the focus (an error came instead), or the focus gone elsewhere.
      if (at === cell || (at !== el && !hovered)) tip.hide();
    },
    { flush: "post" },
  );
  // Its editor opening (Enter, F2, a double click), the cell's tooltip leaves: the pointer
  // over it would keep it above the editor.
  watch(host.editing, (cell) => {
    if (!cell) return;
    clearTimeout(switching);
    tip.hide();
  });

  return { reasons, reasonId, reasonOf, tip, text, enter, over, leave, pin };
}
