// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The contract of in-place editing (spec 020, design « Contrat d'édition »; REQ-020-19),
// shared by the grids (`DataGrid`; `TreeGrid`, task 26): the host says which cells are
// editable and checks the text; Enter or F2 opens the editor, Escape cancels, Enter or Tab
// commits; the focus leaving for elsewhere in the page commits a changed text, leaving the
// window keeps the editor open; an unchanged text never commits. A text refused by the
// check keeps the editor open, its error said with the way out (Escape). After a commit,
// the host renders the cell's state; an error it gives then is said once.

import { nextTick, ref, watch, type Ref } from "vue";
import type { CellRef, CellState, GridCommit } from "../DataGrid/grid.ts";

/** What the grid gives the editor of its cells. */
export interface CellEditorHost {
  editing: Ref<CellRef | undefined>;
  /** The text the cell holds now (what the editor opens with). */
  textOf(cell: CellRef): string;
  /** Whether the grid drives the editing (the host gave `editable`). */
  driven(): boolean;
  /** The local check of the host: the error of the text, or nothing. */
  check(cell: CellRef, value: string): string | undefined;
  cellElement(cell: CellRef): HTMLElement | null;
  /** After an editor closes by the keyboard: the focus to the cell `by` columns along. */
  moveFocus(cell: CellRef, by: number): void;
  commit(commit: GridCommit): void;
  /** The alert of the grid: a refusal, said with the way out. */
  refuse(cell: CellRef, error: string): void;
}

/** The controls an editor may hold, the first one taking the focus. */
export const EDITOR_CONTROLS = "input, select, textarea, button, [tabindex]";

export function useCellEditor(host: CellEditorHost) {
  /** The text of the open editor, and the one it opened with. */
  const draft = ref("");
  const original = ref("");
  /** The error of the text, from the host's check. */
  const error = ref<string | undefined>(undefined);

  /** The focus into the editor of `cell` once it is rendered, its text chosen. */
  async function focusEditor(cell: CellRef): Promise<void> {
    await nextTick();
    const el = host
      .cellElement(cell)
      ?.querySelector<HTMLElement>(`.k-data-grid__editor :is(${EDITOR_CONTROLS})`);
    if (!el || el === document.activeElement) return;
    el.focus();
    if (el instanceof HTMLInputElement) el.select();
  }

  watch(
    host.editing,
    (cell, before) => {
      if (cell && (cell.row !== before?.row || cell.column !== before?.column)) {
        draft.value = original.value = host.textOf(cell);
        error.value = undefined;
        // The host opened the editor of the cell that has the focus: the focus goes into
        // it (the cell leaves the tab order while it edits; WCAG 2.4.3).
        const el = host.cellElement(cell);
        if (el && document.activeElement === el) void focusEditor(cell);
      }
      if (!cell) error.value = undefined;
    },
    { immediate: true },
  );
  /** A new text: the refusal of the former one no longer holds. */
  watch(draft, () => (error.value = undefined));

  async function start(cell: CellRef): Promise<void> {
    draft.value = original.value = host.textOf(cell);
    error.value = undefined;
    host.editing.value = cell;
    await focusEditor(cell);
  }

  /**
   * Leaves the editor with its text: refused by the check, it stays open with its error,
   * said; unchanged, it closes without a commit; else committed. `move`: the column the
   * focus goes on to (Tab: 1, Shift+Tab: -1, Enter: 0), or none (the focus left).
   */
  function finish(move: number | null): boolean {
    const cell = host.editing.value;
    if (!cell) return false;
    if (draft.value !== original.value) {
      const refused = host.check(cell, draft.value);
      if (refused) {
        error.value = refused;
        host.refuse(cell, refused);
        return false;
      }
      host.commit({ row: cell.row, column: cell.column, value: draft.value });
    }
    host.editing.value = undefined;
    if (move !== null) host.moveFocus(cell, move);
    return true;
  }

  function cancel(): void {
    const cell = host.editing.value;
    host.editing.value = undefined;
    if (cell) host.moveFocus(cell, 0);
  }

  /** Escape, Enter and Tab of the editor (the others are its own). */
  function onKeydown(event: KeyboardEvent): void {
    if (event.defaultPrevented || !host.driven()) return;
    if (event.key === "Escape") {
      event.preventDefault();
      cancel();
    } else if (event.key === "Enter" || event.key === "Tab") {
      event.preventDefault();
      finish(event.key === "Enter" ? 0 : event.shiftKey ? -1 : 1);
    }
  }

  /**
   * The focus leaves the editor: for elsewhere in the page, a changed text commits; for
   * nowhere (another window, the browser's own controls), the editor stays open.
   */
  function onFocusout(event: FocusEvent): void {
    const cell = host.editing.value;
    const to = event.relatedTarget as Node | null;
    if (!host.driven() || !cell || !to) return;
    if (!(event.target as HTMLElement).closest(".k-data-grid__editor")) return;
    if (host.cellElement(cell)?.contains(to)) return;
    finish(null);
  }

  return { draft, error, start, finish, cancel, onKeydown, onFocusout, focusEditor };
}

/** The most commits followed at once; the oldest are dropped beyond. */
const AWAITED = 64;

/**
 * The commits a grid follows until the host settles them: an error it gives the cell then
 * is said once (`refused`); the cell in the draft, in review, or showing the committed text
 * without a state (applied), and the commit is done. Never more than `AWAITED`.
 */
export function useCommitWatch(
  stateOf: (cell: CellRef) => CellState | undefined,
  textOf: (cell: CellRef) => string | undefined,
  refused: (cell: CellRef, error: { error: string; code?: string }) => void,
) {
  const awaiting = ref<GridCommit[]>([]);
  watch(
    () => awaiting.value.map((commit) => [stateOf(commit), textOf(commit)] as const),
    (now) => {
      const still: GridCommit[] = [];
      now.forEach(([state, text], i) => {
        const commit = awaiting.value[i]!;
        if (typeof state === "object") refused(commit, state);
        else if (state === "pending" || (state === undefined && text !== commit.value))
          still.push(commit);
      });
      if (still.length !== awaiting.value.length) awaiting.value = still;
    },
  );
  function follow(commit: GridCommit): void {
    awaiting.value = [
      ...awaiting.value.filter((c) => c.row !== commit.row || c.column !== commit.column),
      commit,
    ].slice(-AWAITED);
  }
  return { awaiting, follow };
}
