// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { IconName } from "../../icons/index.ts";

/** A node of a tree: its identifier, label, key in fixed pitch, icon and children. */
export interface TreeNode {
  id: string;
  label: string;
  /** The key of the object (a prefix, an address), in fixed pitch. */
  objectKey?: string;
  icon?: IconName;
  /** Children, when known; `hasChildren` alone when they are still to be read. */
  children?: TreeNode[];
  hasChildren?: boolean;
  /** Further cells of the row, by column id (treegrid). */
  cells?: Record<string, string>;
}

/** How the parent of a node is known (REQ-020-31): deduced from the data, or chosen. */
export type ParentMode = "derived" | "declared";

/** The outcome of `canMove` for a target (design « Contrat de déplacement »). */
export type MoveCheck = { ok: true } | { ok: false; reason: string; code?: string };

/**
 * The state of a node as the host renders it: in a draft, its children loading, a target
 * accepted or refused during a drag, a move pending or refused.
 */
export type TreeNodeState =
  | "draft"
  | "loading-children"
  | "drop-ok"
  | { drop: "refused"; reason: string; code?: string }
  | "move-pending"
  | { refused: { reason: string; code?: string } };

/** One rendered row of the flattened tree. */
export interface TreeRow {
  node: TreeNode;
  level: number;
  setSize: number;
  posInSet: number;
  parent: string | null;
}

/**
 * The rows of the tree as shown: depth first, the children of expanded nodes only. Each
 * row keeps the parent of the model (REQ-020-32), what the display guard compares.
 */
export function flatten(nodes: readonly TreeNode[], expanded: ReadonlySet<string>): TreeRow[] {
  const rows: TreeRow[] = [];
  const walk = (list: readonly TreeNode[], level: number, parent: string | null) => {
    list.forEach((node, i) => {
      rows.push({ node, level, setSize: list.length, posInSet: i + 1, parent });
      if (node.children && expanded.has(node.id)) walk(node.children, level + 1, node.id);
    });
  };
  walk(nodes, 1, null);
  return rows;
}

/**
 * The display invariant (REQ-020-32): the visual parent of each row, the nearest row above
 * of level − 1, is the parent of the model. Returns the rows that break it.
 */
export function misplaced(rows: readonly TreeRow[]): string[] {
  const stack: string[] = [];
  const wrong: string[] = [];
  for (const row of rows) {
    stack.length = row.level - 1;
    const visual = row.level === 1 ? null : (stack[row.level - 2] ?? null);
    if (visual !== row.parent) wrong.push(row.node.id);
    stack[row.level - 1] = row.node.id;
  }
  return wrong;
}
