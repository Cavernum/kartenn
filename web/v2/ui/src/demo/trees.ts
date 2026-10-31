// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The trees of the catalogue (`TreeGrid`, `Tree`): entities of declared parents for the
// keyboard scripts, a hundred thousand nodes, children read page by page, and a drag held
// over a target for the captures.

import { ref, shallowRef } from "vue";
import type { TreeNode, TreeNodeState } from "../components/TreeGrid/tree.ts";

/** Entities whose parent the user chooses (a new tree each call: the host moves in it). */
export function keyboardTree(): TreeNode[] {
  const entity = (id: string, label: string, owner: string, children?: TreeNode[]): TreeNode => ({
    id,
    label,
    icon: "folder",
    cells: { owner },
    ...(children ? { children } : {}),
  });
  return [
    entity("n1", "Infrastructure", "Équipe réseau", [
      entity("n11", "Réseaux", "Équipe réseau", [
        entity("n111", "Cœur", "Équipe réseau"),
        entity("n112", "Accès", "Équipe réseau"),
      ]),
      entity("n12", "Serveurs", "Équipe système"),
      entity("n13", "Stockage", "Équipe système"),
    ]),
    entity("n2", "Postes de travail", "Support", [
      entity("n21", "Assistance", "Support"),
      entity("n22", "Ateliers", "Support"),
    ]),
    entity("n3", "Archives", "Documentation"),
  ];
}

/** A hundred domains of a thousand children each: 100 100 nodes. */
export function virtualTree(): TreeNode[] {
  return Array.from({ length: 100 }, (_, d) => ({
    id: `v${d + 1}`,
    label: `Domaine ${d + 1}`,
    icon: "domain" as const,
    cells: { n: String(d + 1) },
    children: Array.from({ length: 1000 }, (_, c) => ({
      id: `v${d + 1}-${c + 1}`,
      label: `Nœud ${d + 1}.${c + 1}`,
      cells: { n: String(c + 1) },
    })),
  }));
}

/**
 * The groups of VLAN of two sites, five per page; the host asked for a page shows it
 * loading until `deliver` (a button of the demo: the capture holds still on the page
 * asked, the script delivers).
 */
export function pagedTree() {
  const vlans = (site: string, count: number, from: number) =>
    Array.from({ length: count }, (_, i) => ({
      id: `${site}-${i + 1}`,
      label: `VLAN ${from + i}`,
      objectKey: String(from + i),
    }));
  const all: Record<string, TreeNode[]> = { g1: vlans("g1", 12, 100), g2: vlans("g2", 3, 200) };
  const shown = ref<Record<string, number | undefined>>({ g1: 5 });
  const waiting = ref<string[]>([]);
  const asked = ref(0);
  const site = (id: string, label: string): TreeNode => {
    const count = shown.value[id];
    return count === undefined
      ? { id, label, icon: "domain", hasChildren: true }
      : {
          id,
          label,
          icon: "domain",
          children: all[id]!.slice(0, count),
          more: count < all[id]!.length,
        };
  };
  const build = () => [site("g1", "Site A"), site("g2", "Site B")];
  const nodes = shallowRef(build());
  return {
    nodes,
    asked,
    waiting,
    nodeState: (node: TreeNode): TreeNodeState | undefined =>
      waiting.value.includes(node.id) ? "loading-children" : undefined,
    load(parent: string | null) {
      asked.value += 1;
      if (parent && all[parent] && !waiting.value.includes(parent))
        waiting.value = [...waiting.value, parent];
    },
    /** The pages asked, given. */
    deliver() {
      const next = { ...shown.value };
      for (const id of waiting.value) next[id] = Math.min(all[id]!.length, (next[id] ?? 0) + 5);
      shown.value = next;
      waiting.value = [];
      nodes.value = build();
    },
  };
}

/**
 * A drag of the row `from` held over the row `to` inside `root` (the capture of a target
 * state): the events of the platform, with their transfer.
 */
export function dragOver(root: HTMLElement | null, from: string, to: string): void {
  const row = (id: string) => root?.querySelector<HTMLElement>(`[data-node="${id}"]`);
  const source = row(from);
  const target = row(to);
  if (!source || !target || typeof DataTransfer === "undefined") return;
  const dataTransfer = new DataTransfer();
  source.dispatchEvent(
    new DragEvent("dragstart", { bubbles: true, cancelable: true, dataTransfer }),
  );
  target.dispatchEvent(
    new DragEvent("dragenter", { bubbles: true, cancelable: true, dataTransfer }),
  );
  target.dispatchEvent(
    new DragEvent("dragover", { bubbles: true, cancelable: true, dataTransfer }),
  );
}
