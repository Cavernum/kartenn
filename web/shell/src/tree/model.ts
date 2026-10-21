// The tree of the prefixes of a domain (REQ-007-17, 31; design « Thème, composants,
// arbre »): a flat model of the visible rows, the children of a node read on demand, page
// by page; only prefixes have children.

/** A node as the API lists it (`TreeNode` of the contract). */
export type Node =
  | { kind: "prefix"; id: string; href: string; value: string; nature: string }
  | { kind: "range"; id: string; href: string; first: string; last: string; usage: string }
  | { kind: "address"; id: string; href: string; value: string };

/** The children of a node read so far: their identifiers, and the cursor of the rest. */
interface Branch {
  children: string[];
  /** The link of the next page; `null` before the first and once the last is read. */
  next: string | null;
  complete: boolean;
  loading: boolean;
  failed: boolean;
}

/** The pseudo-node whose children are the roots of the domain. */
export const ROOT = "";

/** A row on screen: a node, or the row that reads the next page of its parent. */
export type Row =
  | {
      kind: "node";
      key: string;
      node: Node;
      level: number;
      parent: string;
      /** 1-based rank among the siblings read. */
      posinset: number;
      /** The number of siblings, `-1` while pages remain (`aria-setsize`). */
      setsize: number;
      expanded: boolean | null;
    }
  | { kind: "more"; key: string; parent: string; level: number; loading: boolean; failed: boolean };

/** A page of children: the nodes, and what reads the next page (its link), if any. */
export interface Page {
  items: Node[];
  next: string | null;
}

/** Reads a page of the children of `parent`: the first, or the one `next` names. */
export type Fetcher = (parent: string, next: string | null) => Promise<Page>;

/** The label of a node: its value, or the bounds of a range. */
export function label(node: Node): string {
  return node.kind === "range" ? `${node.first} – ${node.last}` : node.value;
}

/** The route of the record of a node in the client. */
export function recordPath(node: Node): string {
  const collection = node.kind === "prefix" ? "prefixes" : `${node.kind}s`;
  return `/${collection}/${node.id}`;
}

/**
 * The state of a tree: nodes read, branches by parent, open nodes. Mutated in place; the
 * component keeps a version counter to recompute its rows.
 */
export class Tree {
  readonly nodes = new Map<string, Node>();
  readonly branches = new Map<string, Branch>();
  readonly open = new Set<string>();
  private generation = 0;

  constructor(private readonly fetcher: Fetcher) {}

  /** Forgets everything (another domain, other dates); pages in flight are dropped. */
  reset(): void {
    this.nodes.clear();
    this.branches.clear();
    this.open.clear();
    this.generation += 1;
  }

  /** Reads the next page of the children of `parent`, unless read or being read. */
  async loadMore(parent: string): Promise<void> {
    let branch = this.branches.get(parent);
    if (!branch) {
      branch = { children: [], next: null, complete: false, loading: false, failed: false };
      this.branches.set(parent, branch);
    }
    if (branch.complete || branch.loading) return;
    branch.loading = true;
    branch.failed = false;
    const generation = this.generation;
    try {
      const page = await this.fetcher(parent, branch.next);
      if (generation !== this.generation) return;
      for (const node of page.items) {
        if (!this.nodes.has(node.id)) branch.children.push(node.id);
        this.nodes.set(node.id, node);
      }
      branch.next = page.next;
      branch.complete = page.next === null;
    } catch {
      if (generation === this.generation) branch.failed = true;
    } finally {
      if (generation === this.generation) branch.loading = false;
    }
  }

  /** Opens a prefix and reads its first page; a range or an address does not open. */
  async expand(id: string): Promise<void> {
    if (this.nodes.get(id)?.kind !== "prefix") return;
    this.open.add(id);
    if (!this.branches.has(id)) await this.loadMore(id);
  }

  collapse(id: string): void {
    this.open.delete(id);
  }

  /** The visible rows, depth first; a branch with pages left ends with its `more` row. */
  rows(): Row[] {
    const rows: Row[] = [];
    const walk = (parent: string, level: number): void => {
      const branch = this.branches.get(parent);
      if (!branch) return;
      const setsize = branch.complete ? branch.children.length : -1;
      branch.children.forEach((id, index) => {
        const node = this.nodes.get(id)!;
        const expanded = node.kind === "prefix" ? this.open.has(id) : null;
        rows.push({
          kind: "node",
          key: id,
          node,
          level,
          parent,
          posinset: index + 1,
          setsize,
          expanded,
        });
        if (expanded) walk(id, level + 1);
      });
      if (!branch.complete) {
        rows.push({
          kind: "more",
          key: `more:${parent}`,
          parent,
          level,
          loading: branch.loading,
          failed: branch.failed,
        });
      }
    };
    walk(ROOT, 1);
    return rows;
  }
}

/**
 * The row a key moves the focus to, in the `treeview` pattern of the APG: arrows up and
 * down through the visible rows, right opens then enters, left closes then goes up, Home
 * and End to the first and last rows. `null`: the focus stays, the key acts in place
 * (open or close), or the key is not the tree's.
 */
export function moveFocus(
  rows: readonly Row[],
  active: number,
  key: string,
): { to?: number; open?: string; close?: string } | null {
  if (rows.length === 0) return null;
  const row = rows[active];
  switch (key) {
    case "ArrowDown":
      return { to: Math.min(active + 1, rows.length - 1) };
    case "ArrowUp":
      return { to: Math.max(active - 1, 0) };
    case "Home":
      return { to: 0 };
    case "End":
      return { to: rows.length - 1 };
    case "ArrowRight":
      if (row?.kind !== "node" || row.expanded === null) return null;
      if (!row.expanded) return { open: row.key };
      return rows[active + 1]?.level === row.level + 1 ? { to: active + 1 } : null;
    case "ArrowLeft": {
      if (row?.kind === "node" && row.expanded) return { close: row.key };
      const parent = row?.parent;
      if (!parent) return null;
      const index = rows.findIndex((r) => r.kind === "node" && r.key === parent);
      return index >= 0 ? { to: index } : null;
    }
    default:
      return null;
  }
}
