// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The model of the trees of kartenn (`TreeGrid`, `Tree`; design « Inventaire », line 21,
// « Contrat de déplacement »; REQ-020-19, 31, 32): the nodes the host gives, flattened into
// the rows shown (the children of open nodes, a row that reads the next page where pages
// remain), the display guard, and the keys of the tree pattern over the flattened index.

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
  /**
   * Children beyond those given remain to be read (a paged list): their count is unknown
   * (`aria-setsize="-1"`) and a row after them reads the next page (`load`).
   */
  more?: boolean;
  /** Further cells of the row, by column id (treegrid). */
  cells?: Record<string, string>;
  /** What the node is, beside its label (a description of the row, not its name). */
  detail?: string;
}

/** How the parent of a node is known (REQ-020-31): deduced from the data, or chosen. */
export type ParentMode = "derived" | "declared";

/** The outcome of `canMove` for a target (design « Contrat de déplacement »). */
export type MoveCheck = { ok: true } | { ok: false; reason: string; code?: string };

/** A move asked by the user: an intent only, the host moves (or not) in its data. */
export interface MoveIntent {
  node: TreeNode;
  /** The new parent; `null` for the first level. */
  target: TreeNode | null;
}

/** Where a move stands, as the host says it: none, pending, refused (with its reason). */
export type MoveState = "idle" | "pending" | { refused: MoveCheck };

/** The state of a node as the host renders it: in a draft, its children loading. */
export type TreeNodeState = "draft" | "loading-children";

/** One rendered row of the flattened tree: a node, or the row that reads a next page. */
export type TreeRow =
  | {
      kind: "node";
      key: string;
      node: TreeNode;
      level: number;
      /** The number of siblings, `-1` while pages remain (`aria-setsize`). */
      setSize: number;
      posInSet: number;
      parent: string | null;
    }
  | { kind: "more"; key: string; level: number; parent: string | null };

/** The key of the row that reads the next page of the children of `parent`. */
export const moreKey = (parent: string | null): string => `more:${parent ?? ""}`;

/** Whether a node has children, read or still to read. */
export const hasChildren = (node: TreeNode): boolean =>
  Boolean(node.children?.length || node.hasChildren || node.more);

/**
 * The rows of the tree as shown: depth first, the children of expanded nodes only, a
 * `more` row after a list that has pages left (`more`: the first level's). Each row keeps
 * the parent of the model (REQ-020-32), what the display guard compares.
 */
export function flatten(
  nodes: readonly TreeNode[],
  expanded: ReadonlySet<string>,
  more = false,
): TreeRow[] {
  const rows: TreeRow[] = [];
  const walk = (list: readonly TreeNode[], level: number, parent: string | null, rest: boolean) => {
    const setSize = rest ? -1 : list.length;
    list.forEach((node, i) => {
      rows.push({ kind: "node", key: node.id, node, level, setSize, posInSet: i + 1, parent });
      if (!expanded.has(node.id)) return;
      // Open with its children still to read: the row that reads their first page.
      if (node.children) walk(node.children, level + 1, node.id, Boolean(node.more));
      else if (hasChildren(node))
        rows.push({ kind: "more", key: moreKey(node.id), level: level + 1, parent: node.id });
    });
    if (rest) rows.push({ kind: "more", key: moreKey(parent), level, parent });
  };
  walk(nodes, 1, null, more);
  return rows;
}

/**
 * The display invariant (REQ-020-32): the visual parent of each row, the nearest row above
 * of level − 1, is the parent of the model. Returns the keys of the rows that break it.
 */
export function misplaced(rows: readonly { key: string; level: number; parent: string | null }[]) {
  const stack: string[] = [];
  const wrong: string[] = [];
  for (const row of rows) {
    stack.length = row.level - 1;
    const visual = row.level === 1 ? null : (stack[row.level - 2] ?? null);
    if (visual !== row.parent) wrong.push(row.key);
    stack[row.level - 1] = row.key;
  }
  return wrong;
}

/**
 * The rows of a component, the guard checked outside production (REQ-020-32): a row under
 * a parent not its own throws.
 */
export function guarded(component: string, rows: TreeRow[]): TreeRow[] {
  if (import.meta.env.MODE !== "production") {
    const wrong = misplaced(rows);
    if (wrong.length) throw new Error(`${component}: rows under a parent not theirs: ${wrong}`);
  }
  return rows;
}

/** Each node of a tree by its id, with the id of its parent. */
export function indexNodes(
  nodes: readonly TreeNode[],
): Map<string, { node: TreeNode; parent: string | null }> {
  const index = new Map<string, { node: TreeNode; parent: string | null }>();
  const walk = (list: readonly TreeNode[], parent: string | null) => {
    for (const node of list) {
      index.set(node.id, { node, parent });
      if (node.children) walk(node.children, node.id);
    }
  };
  walk(nodes, null);
  return index;
}

/**
 * Where the focus goes when its row leaves the rows shown (its node moved under a closed
 * parent, removed, its parent closed by the host): the node's parent of now if shown,
 * else the nearest row before it that is still shown, else the first row (WCAG 2.4.3).
 */
export function landingRow(
  key: string,
  before: readonly TreeRow[],
  now: readonly TreeRow[],
  parentOf: (key: string) => string | null | undefined,
): string | undefined {
  const shown = new Set(now.map((row) => row.key));
  const parent = parentOf(key);
  if (parent && shown.has(parent)) return parent;
  const at = before.findIndex((row) => row.key === key);
  for (let i = at - 1; i >= 0; i--) if (shown.has(before[i]!.key)) return before[i]!.key;
  return now[0]?.key;
}

/** What a key of the tree pattern does: the row to focus, a node to open or to close. */
export interface TreeStep {
  to?: number;
  open?: string;
  close?: string;
}

/**
 * The step of `key` from the row `at` (APG Tree View, the tree column of a treegrid): Up
 * and Down by one row, Home and End to the ends, the open key (Right, Left in RTL) opens a
 * closed node then goes to its first child, the close key closes an open node then goes to
 * its parent. Over the flattened index, never over the rendered rows (a virtualised list
 * loses no press). Null: not a key of the tree, or nothing to do.
 */
export function treeStep(
  rows: readonly TreeRow[],
  at: number,
  key: string,
  expanded: ReadonlySet<string>,
  rtl = false,
): TreeStep | null {
  const row = rows[at];
  if (!row) return null;
  const open = rtl ? "ArrowLeft" : "ArrowRight";
  const close = rtl ? "ArrowRight" : "ArrowLeft";
  switch (key) {
    case "ArrowDown":
      return { to: Math.min(rows.length - 1, at + 1) };
    case "ArrowUp":
      return { to: Math.max(0, at - 1) };
    case "Home":
      return { to: 0 };
    case "End":
      return { to: rows.length - 1 };
  }
  if (key === open) {
    if (row.kind !== "node" || !hasChildren(row.node)) return null;
    if (!expanded.has(row.key)) return { open: row.key };
    return rows[at + 1]?.parent === row.key ? { to: at + 1 } : null;
  }
  if (key === close) {
    if (row.kind === "node" && hasChildren(row.node) && expanded.has(row.key))
      return { close: row.key };
    if (row.parent === null) return null;
    const parent = rows.findIndex((r) => r.key === row.parent);
    return parent >= 0 ? { to: parent } : null;
  }
  return null;
}

/** The siblings of row `at` that `*` opens (APG Tree View): those with children, closed. */
export function closedSiblings(
  rows: readonly TreeRow[],
  at: number,
  expanded: ReadonlySet<string>,
): string[] {
  const row = rows[at];
  if (!row) return [];
  return rows.flatMap((r) =>
    r.kind === "node" &&
    r.parent === row.parent &&
    r.level === row.level &&
    hasChildren(r.node) &&
    !expanded.has(r.key)
      ? [r.key]
      : [],
  );
}

/** Folds case and accents, so that « reseau » finds « Réseau ». */
const fold = (text: string) => text.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();

/**
 * A key that types into the search of a tree: one printable character (Shift allowed,
 * for capitals), none with Control, Alt or Meta, not Space (it selects). Shift, Control,
 * an arrow or a function key alone never enter the search.
 */
export const typesAhead = (
  event: Pick<KeyboardEvent, "key" | "ctrlKey" | "altKey" | "metaKey">,
): boolean =>
  [...event.key].length === 1 &&
  event.key !== " " &&
  !event.ctrlKey &&
  !event.altKey &&
  !event.metaKey;

/**
 * The row the search `text` goes to from row `at` (APG Tree View, type-ahead): the next
 * node whose label starts with it, from the row after (one character) or from the row
 * itself (a longer text still matching it), around the end; -1 when none.
 */
export function searchRow(rows: readonly TreeRow[], at: number, text: string): number {
  const wanted = fold(text);
  if (!wanted || rows.length === 0) return -1;
  const from = [...text].length > 1 ? at : at + 1;
  for (let i = 0; i < rows.length; i++) {
    const index = (((from + i) % rows.length) + rows.length) % rows.length;
    const row = rows[index]!;
    if (row.kind === "node" && fold(row.node.label).startsWith(wanted)) return index;
  }
  return -1;
}
