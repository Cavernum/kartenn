// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The hosts of the component benches (spec 020, REQ-020-29, design « Budgets »): each
// alone on `/_ui/bancs/<Nom>` of the catalogue build, driven by the Playwright project
// `components-bench` (web/v2/e2e/bench/). Never captured, never in a production build.

import { computed, defineComponent, h, nextTick, ref, shallowRef, type Component } from "vue";
import Button from "../components/Button/Button.vue";
import Cluster from "../components/Cluster/Cluster.vue";
import DataGrid from "../components/DataGrid/DataGrid.vue";
import type { GridColumn, GridCommit, GridSort } from "../components/DataGrid/grid.ts";
import Stack from "../components/Stack/Stack.vue";
import TextField from "../components/TextField/TextField.vue";
import Tree from "../components/Tree/Tree.vue";
import type { MoveCheck, MoveIntent, MoveState, TreeNode } from "../components/TreeGrid/tree.ts";
import TreeGrid from "../components/TreeGrid/TreeGrid.vue";
import { environments } from "../fixtures/index.ts";
import { ANSWER_EVENT, fullTarget, lockedTarget, STATE_MEASURE } from "./benchNames.ts";
import { moveNode, noCycle } from "./treeHost.ts";
import { virtualTree } from "./trees.ts";

/**
 * The host of the catalogue (`moveHost`), its answer after `delay` ms said to the bench
 * (`ANSWER_EVENT`). It refuses a part of the targets, so that the trees render refusals:
 * the check of the tree (`canMove`, with its reason) under a `lockedTarget`, the
 * « server » (`moveState` refused, with its code) under a `fullTarget`.
 */
function benchHost(start: TreeNode[], delay = 300) {
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
    canMove(node: TreeNode, target: TreeNode | null): MoveCheck {
      return lockedTarget(target?.id)
        ? { ok: false, reason: `${target!.label} est verrouillé.`, code: "entity.locked" }
        : noCycle(node, target);
    },
    moveState: (node: TreeNode) => states.value.get(node.id),
    onMove({ node, target }: MoveIntent) {
      setState(node.id, "pending");
      setTimeout(() => {
        // The answer ready (the new model built: the host's work, not the tree's), then
        // said to the bench, then given to the tree.
        const refused = fullTarget(target?.id);
        const next = refused ? nodes.value : moveNode(nodes.value, node.id, target?.id ?? null);
        window.dispatchEvent(new Event(ANSWER_EVENT));
        if (refused) {
          const reason = `${target!.label} est plein.`;
          setState(node.id, { refused: { ok: false, reason, code: "entity.full" } });
          return;
        }
        nodes.value = next;
        setState(node.id, undefined);
      }, delay);
    },
    /** A refusal given, then taken back, on the last node: a change of `moveState` alone. */
    async toggleState(): Promise<void> {
      const id = "v100-1000";
      const start = performance.now();
      setState(
        id,
        states.value.has(id)
          ? undefined
          : { refused: { ok: false, reason: "Refus du banc.", code: "bench.refused" } },
      );
      await nextTick();
      performance.measure(STATE_MEASURE, { start, end: performance.now() });
    },
  };
}

/**
 * A tree of a hundred thousand nodes (`virtualTree`), its model built before the tree is
 * shown: « Afficher l'arbre » mounts it, all closed; then five domains (5 100 rows) or
 * all of them (100 100 rows) open by a button, and moves by the keyboard; « Changer
 * l'état d'un nœud » changes `moveState` alone (`STATE_MEASURE`).
 */
function treeBench(tree: Component, more: Record<string, unknown> = {}): Component {
  return defineComponent({
    setup() {
      const host = benchHost(virtualTree());
      const shown = ref(false);
      const expanded = ref<string[]>([]);
      const domains = (count: number) => host.nodes.value.slice(0, count).map((node) => node.id);
      return () =>
        h(Stack, { gap: "3", align: "stretch" }, () => [
          h(Cluster, { gap: "2" }, () => [
            h(Button, { onClick: () => (shown.value = true) }, () => "Afficher l'arbre"),
          ]),
          shown.value
            ? h(tree, {
                ...more,
                label: "Cent mille nœuds",
                nodes: host.nodes.value,
                expanded: expanded.value,
                "onUpdate:expanded": (next: string[]) => (expanded.value = next),
                parentMode: "declared",
                canMove: host.canMove,
                moveState: host.moveState,
                onMove: host.onMove,
              })
            : null,
          h(Cluster, { gap: "2" }, () => [
            h(
              Button,
              { onClick: () => (expanded.value = domains(5)) },
              () => "Ouvrir cinq domaines",
            ),
            h(
              Button,
              { onClick: () => (expanded.value = domains(host.nodes.value.length)) },
              () => "Tout ouvrir",
            ),
            h(Button, { onClick: host.toggleState }, () => "Changer l'état d'un nœud"),
          ]),
        ]);
    },
  });
}

const LABELS = ["Serveurs de bases", "Postes du siège", "Supervision", "Sauvegardes", "Invités"];
const OWNERS = ["Équipe réseau", "Équipe postes", "Exploitation"];

/** Ten thousand prefixes, as the state `virtual` of `DataGrid`. */
function gridRows() {
  return Array.from({ length: 10_000 }, (_, i) => ({
    prefix: `10.${Math.floor(i / 256)}.${i % 256}.0/24`,
    label: `${LABELS[i % LABELS.length]} ${i + 1}`,
    env: environments[i % 3]!.label ?? "",
    used: `${(i * 37) % 101} %`,
    owner: OWNERS[i % 3]!,
  }));
}
type Row = ReturnType<typeof gridRows>[number];

const gridColumns: GridColumn[] = [
  { id: "prefix", label: "Préfixe", width: "md", mono: true, sortable: true },
  { id: "label", label: "Libellé", width: "fill", sortable: true },
  { id: "env", label: "Environnement", width: "md" },
  { id: "used", label: "Occupation", width: "md", align: "end" },
  { id: "owner", label: "Responsable", width: "md" },
];

/**
 * A grid of ten thousand rows whose host filters (a field over the labels), sorts (the
 * sortable headers) and takes the edits of the labels into its data, as a screen would.
 */
const gridBench = defineComponent({
  setup() {
    const data = shallowRef(gridRows());
    const text = ref("");
    const sort = ref<GridSort | undefined>(undefined);
    const rows = computed(() => {
      const needle = text.value.trim().toLowerCase();
      const kept = needle
        ? data.value.filter((row) => row.label.toLowerCase().includes(needle))
        : data.value;
      if (!sort.value) return kept;
      const key = sort.value.column as keyof Row;
      const sign = sort.value.direction === "ascending" ? 1 : -1;
      return [...kept].sort((a, b) => (a[key] < b[key] ? -sign : a[key] > b[key] ? sign : 0));
    });
    function onCommit(commit: GridCommit): void {
      data.value = data.value.map((row) =>
        row.prefix === commit.row ? { ...row, [commit.column]: commit.value } : row,
      );
    }
    return () =>
      h(Stack, { gap: "3", align: "stretch" }, () => [
        h(TextField, {
          label: "Filtrer les libellés",
          type: "search",
          modelValue: text.value,
          "onUpdate:modelValue": (next: string) => (text.value = next),
        }),
        // The rows the filter leaves, said (WCAG 4.1.3) as a screen would.
        h("p", { role: "status" }, `${rows.value.length.toLocaleString("fr-FR")} préfixes`),
        h(DataGrid as Component, {
          label: "Dix mille préfixes",
          columns: gridColumns,
          rows: rows.value,
          rowKey: "prefix",
          sort: sort.value,
          "onUpdate:sort": (next: GridSort) => (sort.value = next),
          editable: (_: Row, column: GridColumn) => column.id === "label",
          validate: (_: Row, column: GridColumn, value: string) =>
            column.id === "label" && value.trim() === ""
              ? "Le libellé est obligatoire."
              : undefined,
          onCommit,
        }),
      ]);
  },
});

/** The benches by the name of their component (`/_ui/bancs/<Nom>`). */
export const benches: Record<string, Component> = {
  Tree: treeBench(Tree),
  TreeGrid: treeBench(TreeGrid, {
    treeLabel: "Nœud",
    columns: [{ id: "n", label: "Rang", width: "sm", align: "end" }],
  }),
  DataGrid: gridBench,
};
