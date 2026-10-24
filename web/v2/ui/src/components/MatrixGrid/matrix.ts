// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** An entry of an axis of a matrix: a row or a column (design « Inventaire », line 34). */
export interface MatrixAxis {
  id: string;
  label: string;
  /** A key in fixed pitch (a prefix, a code) rather than a label. */
  mono?: boolean;
}

/** The state of a cell, decided by the host: in a draft, in review. */
export type MatrixCellState = "draft" | "review";

/**
 * A cell of the matrix: the ids of its row and column; `""` is the header of the axis (the
 * row of column headers, the column of row headers).
 */
export interface MatrixCell {
  row: string;
  column: string;
}
