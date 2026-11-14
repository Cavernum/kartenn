// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What a row of `TreeGrid` reads of its tree (`TreeGridRow`, task 021-22e-b): one object,
// the same for every row and every render, so that a row whose own props did not change
// (its row of the model, its index) renders no more as the tree scrolls; what it reads
// through these functions is tracked by its own render, as it was by the tree's.

import type { ComputedRef, Ref, Slot } from "vue";
import type { CellRef, GridColumn } from "../DataGrid/grid.ts";
import type { useTreeMove } from "../internal/treeMove.ts";
import type { ListOption } from "../Select/select.ts";
import type { TreeNode, TreeNodeState, TreeRow } from "./tree.ts";

/** A row of a node (not a notice, nor the row that reads a page). */
export type NodeRow = Extract<TreeRow, { kind: "node" }>;

/** The error a cell shows (`messageOf`). */
export interface CellMessage {
  error: string;
  code?: string;
}

export interface TreeGridRowContext {
  /** The tree column, then the columns rendered. */
  treeColumn: ComputedRef<GridColumn>;
  shown: ComputedRef<GridColumn[]>;
  priority(column: GridColumn): 1 | 2 | 3 | undefined;
  colIndex(column: GridColumn): number;
  selected: Ref<string | undefined>;
  /** The host made the tree editable. */
  editable(): boolean;
  isExpanded(key: string): boolean;
  isStop(row: string, column: string): boolean;
  isEditing(row: string, column: string): boolean;
  canEdit(node: TreeNode, column: GridColumn): boolean;
  nodeState(node: TreeNode): TreeNodeState | undefined;
  stateOf(node: TreeNode, column: GridColumn): string | undefined;
  /** The words of a cell's state, from the state `stateOf` gave. */
  stateWord(state: string | undefined): string | undefined;
  messageOf(node: TreeNode, column: GridColumn): CellMessage | undefined;
  reasonOf(node: TreeNode, column: GridColumn): string | undefined;
  errorId(row: number, column: string): string;
  errorAnchor(node: TreeNode, column: GridColumn): HTMLElement | null;
  cellOf(node: TreeNode, column: GridColumn): string | null;
  extraOf(node: TreeNode, column: GridColumn): Record<string, unknown> | null | undefined;
  addLabel(node: TreeNode): string | undefined;
  addBlocked(node: TreeNode): { forbidden?: string; reason?: string } | undefined;
  moveOf(node: TreeNode): "pending" | "refused" | "cut" | undefined;
  refusal(node: TreeNode): { reason: string; code?: string } | undefined;
  dropOf(node: TreeNode): "ok" | "refused" | undefined;
  rowWords(node: TreeNode): string;
  moves: Pick<
    ReturnType<typeof useTreeMove>,
    "movable" | "onDragStart" | "onDragOver" | "onDrop" | "onDragEnd"
  >;
  /** The editor of the cell being edited: its list, if any, and its text. */
  editorOptions: Ref<readonly ListOption[] | undefined>;
  draft: Ref<string>;
  setDraft(value: string): void;
  choose(value: string, move: number, now: boolean): void;
  editorLabel(node: TreeNode, column: GridColumn): string;
  onRowClick(row: TreeRow, event: MouseEvent): void;
  onCellEnter(node: TreeNode, column: GridColumn, event: PointerEvent): void;
  onCellLeave(row: number, column: string, event: PointerEvent): void;
  onCellOver(event: PointerEvent): void;
  add(key: string): void;
  /** The cell's error: hovered no more, dismissed, back to its cell, focus gone. */
  errorLeave(): void;
  errorDismiss(): void;
  errorBack(cell: CellRef): void;
  errorBlur(): void;
  /** The host's slot `extension`, read when a cell renders it. */
  extension(): Slot | undefined;
  /** The widgets of a row rendered alone out of the tab order, as the tree does its own. */
  tame(row: HTMLElement | null | undefined): void;
}
