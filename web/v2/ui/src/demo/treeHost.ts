// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The host of a tree of declared parents in the catalogue (`TreeGrid`, `Tree`; design
// « Contrat de déplacement »): it checks a move (`canMove`: never under one of its own
// descendants), takes the intent, shows the move pending, then, after a while (the
// server), moves the node in its data or refuses it with a code; the tree only renders.

import { shallowRef, type ShallowRef } from "vue";
import {
  indexNodes,
  type MoveCheck,
  type MoveIntent,
  type MoveState,
  type TreeNode,
} from "../components/TreeGrid/tree.ts";

/** Whether `target` is `node` or one of its descendants. */
export function isUnder(node: TreeNode, target: TreeNode | null): boolean {
  if (!target) return false;
  if (node.id === target.id) return true;
  return (node.children ?? []).some((child) => isUnder(child, target));
}

/** The rule of the catalogue: a node never goes under itself or one of its descendants. */
export function noCycle(node: TreeNode, target: TreeNode | null): MoveCheck {
  return isUnder(node, target)
    ? {
        ok: false,
        reason: "Un nœud ne peut pas aller sous l'un de ses descendants.",
        code: "tree.cycle",
      }
    : { ok: true };
}

/**
 * The model after the move of `id` under `target` (`null`: the first level): a new tree,
 * the node at the end of its new parent's children; unchanged if either is gone or the
 * move would make a cycle (a late answer on a model that changed meanwhile).
 */
export function moveNode(
  nodes: readonly TreeNode[],
  id: string,
  target: string | null,
): TreeNode[] {
  const index = indexNodes(nodes);
  const node = index.get(id)?.node;
  const to = target === null ? null : index.get(target)?.node;
  if (!node || to === undefined || isUnder(node, to)) return [...nodes];
  const without = (list: readonly TreeNode[]): TreeNode[] =>
    list
      .filter((n) => n.id !== id)
      .map((n) => (n.children ? { ...n, children: without(n.children) } : n));
  const into = (list: readonly TreeNode[]): TreeNode[] =>
    list.map((n) =>
      n.id === target
        ? { ...n, children: [...(n.children ?? []), node] }
        : n.children
          ? { ...n, children: into(n.children) }
          : n,
    );
  const rest = without(nodes);
  return target === null ? [...rest, node] : into(rest);
}

/** A host of moves: its model, its states, and what the tree is given. */
export interface MoveHost {
  nodes: ShallowRef<TreeNode[]>;
  canMove: (node: TreeNode, target: TreeNode | null) => MoveCheck;
  onMove: (intent: MoveIntent) => void;
  moveState: (node: TreeNode) => MoveState | undefined;
}

/**
 * The host of the catalogue: moves applied after `delay` ms; a move into a node of
 * `locked` refused by the « server » with its code (the tree's check accepts it).
 */
export function moveHost(start: TreeNode[], locked: readonly string[] = [], delay = 600): MoveHost {
  const nodes = shallowRef(start);
  const states = shallowRef(new Map<string, MoveState>());
  const setState = (id: string, state: MoveState | undefined) => {
    const next = new Map(states.value);
    if (state) next.set(id, state);
    else next.delete(id);
    states.value = next;
  };
  return {
    nodes,
    canMove: noCycle,
    onMove({ node, target }) {
      setState(node.id, "pending");
      setTimeout(() => {
        if (target && locked.includes(target.id)) {
          setState(node.id, {
            refused: {
              ok: false,
              reason: `${target.label} est verrouillé.`,
              code: "entity.locked",
            },
          });
          return;
        }
        nodes.value = moveNode(nodes.value, node.id, target?.id ?? null);
        setState(node.id, undefined);
      }, delay);
    },
    moveState: (node) => states.value.get(node.id),
  };
}
