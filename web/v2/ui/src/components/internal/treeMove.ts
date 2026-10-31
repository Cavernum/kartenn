// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The contract of moves in a tree (spec 020, design « Contrat de déplacement »; REQ-020-31,
// 32), shared by `TreeGrid` and `Tree`. A tree whose parents are deduced (`derived`)
// offers no move at all: no `draggable`, Ctrl+X and Ctrl+V do nothing, no « Déplacer
// vers… ». A tree of declared parents checks each target by the host's `canMove` before it
// shows it: during a drag, the hovered target is marked accepted only once checked, a
// refused one shows its reason and code and takes no drop (`dragover` not prevented); Cut
// (Ctrl+X) then Paste into (Ctrl+V) do the same from the keyboard. A move accepted emits an
// intent only: the tree never moves a node, it renders the parents of its model. The nodes
// are taken by their id from the model of the moment, never from a copy (spike 005).

import { ref } from "vue";
import { fill, t } from "../../i18n.ts";
import { filterOptions } from "../Combobox/combobox.ts";
import type { ListOption } from "../Select/select.ts";
import type { MoveCheck, MoveIntent, ParentMode, TreeNode } from "../TreeGrid/tree.ts";

/** The most targets « Déplacer vers… » lists at once: the text narrows them. */
const TARGETS_LISTED = 50;

/** What a tree gives the contract of moves. */
export interface TreeMoveHost {
  mode(): ParentMode;
  canMove: ((node: TreeNode, target: TreeNode | null) => MoveCheck) | undefined;
  /** The node of this id in the model of now. */
  nodeOf(id: string): TreeNode | undefined;
  move(intent: MoveIntent): void;
  /** The status region of the tree: a refusal, a cut, a move asked. */
  announce(text: string): void;
}

/**
 * The value of the first level among the targets of « Déplacer vers… » (an option needs a
 * string that is not empty; no node id of kartenn starts with a colon).
 */
export const ROOT_TARGET = ":root";

/** The refusal of a check, said in words: its reason, then its code. */
export const sayRefusal = (check: MoveCheck): string =>
  check.ok ? "" : check.code ? `${check.reason} (${check.code})` : check.reason;

export function useTreeMove(host: TreeMoveHost) {
  /** The node being dragged, and the target under it with its check. */
  const dragging = ref<string | null>(null);
  const over = ref<{ id: string; check: MoveCheck } | null>(null);
  /** The node cut (Ctrl+X), waiting for its new parent (Ctrl+V). */
  const cut = ref<string | null>(null);

  const declared = () => host.mode() === "declared";

  /**
   * The check of a move of `nodeId` into `targetId` (`null`: the first level): a node is
   * never its own parent; the rest is the host's (`canMove`), and none means no move.
   */
  function check(nodeId: string, targetId: string | null): MoveCheck {
    const node = host.nodeOf(nodeId);
    const target = targetId === null ? null : host.nodeOf(targetId);
    if (!node || target === undefined) return { ok: false, reason: t("moveGone") };
    if (target?.id === node.id) return { ok: false, reason: t("moveSelf") };
    return host.canMove?.(node, target) ?? { ok: false, reason: t("moveNone") };
  }

  /** Asks the host to move: the intent, with the nodes of now. */
  function ask(nodeId: string, targetId: string | null): boolean {
    const result = check(nodeId, targetId);
    const node = host.nodeOf(nodeId);
    if (!result.ok || !node) {
      if (!result.ok) host.announce(fill("moveRefusedSay", { reason: sayRefusal(result) }));
      return false;
    }
    const target = targetId === null ? null : (host.nodeOf(targetId) ?? null);
    host.move({ node, target });
    host.announce(fill("moveAsked", { node: node.label, target: target?.label ?? t("moveRoot") }));
    return true;
  }

  function onDragStart(id: string, event: DragEvent): void {
    if (!declared()) {
      event.preventDefault();
      return;
    }
    dragging.value = id;
    over.value = null;
    if (event.dataTransfer) {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData("text/plain", host.nodeOf(id)?.label ?? id);
    }
  }

  /**
   * The pointer enters or moves over a target (`dragenter`, `dragover`): checked first,
   * then marked; only an accepted target takes the drop (`preventDefault`), a refused one
   * says why, once. A refused target is left by the platform at once (its `dragenter` not
   * cancelled, the drag's target becomes the body, with a `dragleave`): its mark stays
   * until another row replaces it, the pointer is over the tree but no row
   * (`onDragElsewhere`), or the drag ends.
   */
  function onDragOver(id: string, event: DragEvent): void {
    if (dragging.value === null) return;
    if (over.value?.id !== id) {
      const result = check(dragging.value, id);
      over.value = { id, check: result };
      if (!result.ok) host.announce(fill("dropRefusedSay", { reason: sayRefusal(result) }));
    }
    if (over.value.check.ok) {
      event.preventDefault();
      if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    }
  }

  /** The pointer over the tree but no row (its header, the row that reads a page). */
  function onDragElsewhere(event: DragEvent): void {
    if (dragging.value === null) return;
    const row = (event.target as Element | null)?.closest?.("[draggable='true']");
    if (!row) over.value = null;
  }

  /** A drop: on an accepted target, the intent; anywhere else, nothing. */
  function onDrop(id: string, event: DragEvent): void {
    const from = dragging.value;
    const accepted = over.value?.id === id && over.value.check.ok;
    dragging.value = null;
    over.value = null;
    if (from === null || !accepted) return;
    event.preventDefault();
    ask(from, id);
  }

  function onDragEnd(): void {
    dragging.value = null;
    over.value = null;
  }

  /** Ctrl+X on a node: cut, said with the way on. False when the tree offers no move. */
  function cutNode(id: string): boolean {
    if (!declared()) return false;
    cut.value = id;
    host.announce(fill("cutSay", { node: host.nodeOf(id)?.label ?? id }));
    return true;
  }

  /** Ctrl+V on a target: the cut node into it, if accepted; refused, it stays cut. */
  function pasteInto(targetId: string | null): boolean {
    if (!declared() || cut.value === null) return false;
    if (ask(cut.value, targetId)) cut.value = null;
    return true;
  }

  /** Escape while a node is cut: no longer cut. False when there was nothing to cancel. */
  function cancelCut(): boolean {
    if (cut.value === null) return false;
    cut.value = null;
    host.announce(t("cutCancelled"));
    return true;
  }

  /**
   * The targets of « Déplacer vers… » for `nodeId` that match `text` (label or key): the
   * accepted ones only, the first level first when accepted; `limit` at most (the text
   * narrows them, the host's check runs on the matching nodes only).
   */
  function targets(
    nodeId: string,
    nodes: Iterable<TreeNode>,
    text: string,
    limit = TARGETS_LISTED,
  ): ListOption[] {
    if (!declared()) return [];
    const out: ListOption[] = [];
    const root = { value: ROOT_TARGET, label: t("moveRoot") };
    if (check(nodeId, null).ok && filterOptions([root], text).length) out.push(root);
    for (const node of nodes) {
      if (out.length >= limit) break;
      const option = { value: node.id, label: node.label, detail: node.objectKey };
      if (filterOptions([option], text).length && check(nodeId, node.id).ok) out.push(option);
    }
    return out;
  }

  /** The drop state of a row during a drag: accepted, refused (with its check), none. */
  function dropOf(id: string): { ok: true } | { ok: false; reason: string; code?: string } | null {
    return over.value?.id === id ? over.value.check : null;
  }

  return {
    dragging,
    over,
    cut,
    declared,
    check,
    ask,
    onDragStart,
    onDragOver,
    onDragElsewhere,
    onDrop,
    onDragEnd,
    cutNode,
    pasteInto,
    cancelCut,
    targets,
    dropOf,
  };
}
