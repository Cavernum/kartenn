// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What a row of `DataGrid` reads of its grid (`DataGridRow`, task 021-22o-b): one object, the
// same for every row and every render, so that a row whose props stay (its row of the
// model, its key, its index) renders no more when the grid renders: the roving focus that
// moves, a press on a link, an editor that opens elsewhere, another row's state. What
// a row reads of the grid's shared state (the focus, the editor, the open error, the
// selection) it reads through its own key (`stopIn`, `editingIn`…): the rows whose answer
// changes render again, never the others.

import type { ComponentPublicInstance, ComputedRef, Ref } from "vue";
import type { IconName } from "../../icons/index.ts";
import type { ListOption } from "../Select/select.ts";
import type { GridColumn, RowState } from "./grid.ts";

/** The error a cell shows: its editor's (the host's check), else the host's. */
export interface CellMessage {
  error: string;
  code?: string | undefined;
}

/** Where the grid's shared state stands in a row: the column of each, or none. */
export interface RowAt {
  /** The cell of the roving stop. */
  stop: string | null;
  /** The cell whose editor is open. */
  editing: string | null;
  /** The cell whose error's bubble is open (hovered or focused, not dismissed). */
  error: string | null;
}

/** What a cell of a row shows, each state of the host asked once (`cellsOf`). */
export interface CellView {
  column: GridColumn;
  /** Its place among the columns rendered (the first carries the row's mark). */
  c: number;
  editing: boolean;
  /** `pending`, `draft`, `review` or `error` (an error's message in `message`). */
  state: string | undefined;
  /** The state in words, for assistive technologies. */
  word: string | undefined;
  message: CellMessage | undefined;
  /** An editable grid, and no editor here. */
  readonly: boolean;
  describedBy: string | undefined;
  /** The id of its error's message, when it has one. */
  errorId: string | undefined;
  errorOpen: boolean;
  tabindex: 0 | -1 | undefined;
  /** The properties of the element of an extension column (`elementProps`). */
  extra: Record<string, unknown> | null | undefined;
}

export interface DataGridRowContext<R> {
  /** The columns rendered, in order. */
  shown: ComputedRef<GridColumn[]>;
  /** Several rows selectable: a column of checkboxes first. */
  checkboxes: ComputedRef<boolean>;
  selectable(): boolean | "single";
  stack(): boolean;
  rowOffset(): number;
  priority(column: GridColumn): 1 | 2 | 3 | undefined;
  colIndex(column: GridColumn): number;
  widthOf(column: GridColumn): string;
  widthStyle(column: GridColumn): Record<string, string> | undefined;
  /** The column of the roving stop, of the editor, of the open error, in the row `key`. */
  stopIn(key: string): string | null;
  editingIn(key: string): string | null;
  errorIn(key: string): string | null;
  isSelected(key: string): boolean;
  /** The row just joined a draft: its background fades once. */
  isFresh(key: string): boolean;
  rowState(row: R): RowState | undefined;
  /** The icon of a row with a state, and its hidden word (in the language of now). */
  markOf(state: RowState): { icon: IconName; word: string };
  /** The cells of a row, each state of the host asked once. */
  cellsOf(row: R, index: number, at: RowAt, state: RowState | undefined): CellView[];
  /** The host gave the slot `name` (`editor`, `extension`). */
  has(name: "editor" | "extension"): boolean;
  /** The editor of the cell being edited: its list, if any, and its text. */
  editorOptions: Ref<readonly ListOption[] | undefined>;
  draft: Ref<string>;
  setDraft(value: string): void;
  choose(value: string, move: number, now: boolean): void;
  editorLabel(row: R, column: GridColumn): string;
  /** The row's element, for the virtualiser to measure (`virtualRows.ts`). */
  measure(el: Element | ComponentPublicInstance | null): void;
  onAnimationend(key: string, event: AnimationEvent): void;
  onCellEnter(row: R, column: GridColumn, event: PointerEvent): void;
  onCellLeave(index: number, c: number, event: PointerEvent): void;
  /** Whether the click that reaches a checkbox holds Shift (heard before the checkbox). */
  noteShift(event: MouseEvent): void;
  onBox(key: string): void;
  /** The cell's error: hovered no more, dismissed, back to its cell, focus gone. */
  errorLeave(): void;
  errorDismiss(): void;
  errorBack(index: number, c: number): void;
  errorBlur(): void;
}
