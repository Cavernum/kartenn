// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What `TreeGrid` and `Tree` share around their rows (spec 020, design « Contrat de
// déplacement »; REQ-020-23, 31): the contract of moves bound to the tree's props, the
// state of a row (the host's, its move, its target during a drag) drawn and said the same
// way in both, the status and alert regions, « Déplacer vers… », and the pages of
// children with what their arrival says.

import { computed, nextTick, ref, watch, type ComputedRef, type Ref } from "vue";
import { fill, t } from "../../i18n.ts";
import type {
  MoveCheck,
  MoveIntent,
  MoveState,
  ParentMode,
  TreeNode,
  TreeNodeState,
  TreeRow,
} from "../TreeGrid/tree.ts";
import { sayRefusal, useTreeMove } from "./treeMove.ts";
import { useTreePages } from "./treePages.ts";

/** The props of a tree the shared states read. */
export interface TreeStateProps {
  nodes: TreeNode[];
  more?: boolean;
  moreLoading?: boolean;
  nodeState?: ((node: TreeNode) => TreeNodeState | undefined) | undefined;
  parentMode?: ParentMode;
  canMove?: ((node: TreeNode, target: TreeNode | null) => MoveCheck) | undefined;
  moveState?: ((node: TreeNode) => MoveState | undefined) | undefined;
}

/** A live region filled once, present empty before (said again when the text repeats). */
function useRegion() {
  const text = ref("");
  async function say(next: string): Promise<void> {
    text.value = "";
    await nextTick();
    text.value = next;
  }
  return { text, say };
}

const ROW_WORDS = {
  draft: "inDraft",
  "loading-children": "childrenLoading",
  pending: "movePending",
  refused: "moveRefused",
  cut: "cutMark",
} as const;

export function useTreeStates(
  props: TreeStateProps,
  rows: ComputedRef<TreeRow[]>,
  index: ComputedRef<Map<string, { node: TreeNode; parent: string | null }>>,
  scroller: Ref<HTMLElement | null>,
  emit: { load(parent: string | null): void; move(intent: MoveIntent): void },
) {
  const nodeOf = (id: string) => index.value.get(id)?.node;
  /** The status region (moves, pages) and the alert region (refused edits). */
  const status = useRegion();
  const alerts = useRegion();

  const moves = useTreeMove({
    mode: () => props.parentMode ?? "derived",
    canMove: props.canMove ? (node, target) => props.canMove!(node, target) : undefined,
    nodeOf,
    move: (intent) => emit.move(intent),
    announce: (text) => void status.say(text),
  });
  const declared = computed(() => props.parentMode === "declared");

  /** The state of the move of a node, as the host gives it, or cut, waiting for its parent. */
  function moveOf(node: TreeNode): "pending" | "refused" | "cut" | undefined {
    const state = props.moveState?.(node);
    if (state === "pending") return "pending";
    if (typeof state === "object" && !state.refused.ok) return "refused";
    return moves.cut.value === node.id ? "cut" : undefined;
  }
  /** The reason a row shows: its drop refused during a drag, else its move refused. */
  function refusal(node: TreeNode): { reason: string; code?: string } | undefined {
    const drop = moves.dropOf(node.id);
    if (drop && !drop.ok) return drop;
    const state = props.moveState?.(node);
    return typeof state === "object" && !state.refused.ok ? state.refused : undefined;
  }
  /** The target state of a row during a drag: accepted or refused, once checked. */
  const dropOf = (node: TreeNode) => {
    const drop = moves.dropOf(node.id);
    return drop ? (drop.ok ? "ok" : "refused") : undefined;
  };
  /** The states of a row in words (never colour alone): the host's, its move, its drop. */
  function rowWords(node: TreeNode, more: string[] = []): string {
    const words = [props.nodeState?.(node), moveOf(node)]
      .filter((state): state is keyof typeof ROW_WORDS => !!state)
      .map((state) => t(ROW_WORDS[state]));
    const drop = dropOf(node);
    if (drop) words.push(t(drop === "ok" ? "dropOk" : "dropRefused"));
    return [...words, ...more].join(", ");
  }
  // A refusal the host gives a move is said once, on the row where it shows.
  const refusedKeys = computed(() =>
    rows.value.flatMap((row) =>
      row.kind === "node" && moveOf(row.node) === "refused" ? [row.key] : [],
    ),
  );
  watch(refusedKeys, (now, before) => {
    const fresh = now.find((key) => !before.includes(key));
    const node = fresh ? nodeOf(fresh) : undefined;
    const check = node ? refusal(node) : undefined;
    if (check)
      void status.say(fill("moveRefusedSay", { reason: sayRefusal({ ok: false, ...check }) }));
  });

  /** The node « Déplacer vers… » moves, while its dialog is open. */
  const moving = ref<string | null>(null);
  const movingOpen = computed({
    get: () => moving.value !== null,
    set: (now: boolean) => {
      if (!now) moving.value = null;
    },
  });
  const movingLabel = computed(() => (moving.value ? (nodeOf(moving.value)?.label ?? "") : ""));
  /** « Déplacer vers… » for the node `id`: the host's action on the chosen node. */
  function openMove(id: string): boolean {
    if (!declared.value || !nodeOf(id)) return false;
    moving.value = id;
    return true;
  }
  const moveTargets = (text: string) =>
    moving.value === null
      ? []
      : moves.targets(
          moving.value,
          [...index.value.values()].map((entry) => entry.node),
          text,
        );
  function onMoveTo(target: string | null): void {
    if (moving.value !== null) moves.ask(moving.value, target);
  }

  const sizeOf = (parent: string | null) =>
    parent === null ? props.nodes.length : (nodeOf(parent)?.children?.length ?? 0);
  const pages = useTreePages(
    scroller,
    sizeOf,
    (parent) => emit.load(parent),
    (count, whole) => void status.say(fill(whole ? "pageLoadedLast" : "pageLoaded", { count })),
    (parent) => (parent === null ? !!props.more : !!nodeOf(parent)?.more),
  );
  /** The next page of a list is coming: the host's state of its parent, or of the roots. */
  function pageLoading(parent: string | null): boolean {
    if (parent === null) return !!props.moreLoading;
    const node = nodeOf(parent);
    return !!node && props.nodeState?.(node) === "loading-children";
  }

  return {
    nodeOf,
    status,
    alerts,
    moves,
    declared,
    moveOf,
    refusal,
    dropOf,
    rowWords,
    moving,
    movingOpen,
    movingLabel,
    openMove,
    moveTargets,
    onMoveTo,
    pages,
    pageLoading,
  };
}
