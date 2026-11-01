import type { TreeNode } from "@kartenn/ui";

// The nodes of the tree of the topography from its levels read (task 24, REQ-021-34): the
// children of a prefix are those its own list gives, read again after a change level by
// level. Until every level is read again, one list may still show a node another already
// shows at its new place — even a node under its own former child, a cycle: each node is
// placed once, the first list that shows it, depth first, and waits there for the others
// to be read again, rather than show twice or loop.

/** A level read: its items, whether pages remain; `undefined` when not read yet. */
export interface Level<I> {
  items: readonly I[];
  more: boolean;
}

/** The nodes under `parent`, each placed once. */
export function nodesOf<I extends { id: string }>(
  levelOf: (parent: string | null) => Level<I> | undefined,
  nodeOf: (item: I) => TreeNode,
  /** Whether an item holds children (a prefix). */
  holds: (item: I) => boolean,
  parent: string | null = null,
  placed = new Set<string>(),
): TreeNode[] {
  const items = (levelOf(parent)?.items ?? []).filter((item) => !placed.has(item.id));
  for (const item of items) placed.add(item.id);
  return items.map((item) => {
    const node = nodeOf(item);
    if (!holds(item)) return node;
    const level = levelOf(item.id);
    if (level) {
      node.children = nodesOf(levelOf, nodeOf, holds, item.id, placed);
      if (level.more) node.more = true;
    } else node.hasChildren = true;
    return node;
  });
}
