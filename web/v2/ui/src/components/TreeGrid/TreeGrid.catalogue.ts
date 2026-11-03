// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, onMounted, ref, type Component } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { dragOver, keyboardTree, pagedTree, virtualTree } from "../../demo/trees.ts";
import { settleDemo } from "../../demo/settle.ts";
import { moveHost } from "../../demo/treeHost.ts";
import Button from "../Button/Button.vue";
import Cluster from "../Cluster/Cluster.vue";
import Stack from "../Stack/Stack.vue";
import type { CellRef, CellState, GridColumn, GridCommit } from "../DataGrid/grid.ts";
import TreeGrid from "./TreeGrid.vue";
import { flatten, type MoveState, type TreeNode, type TreeNodeState } from "./tree.ts";

const columns: GridColumn[] = [
  { id: "env", label: "Environnement", width: "md" },
  { id: "used", label: "Occupation", width: "sm", align: "end" },
];

/** An inclusion tree of documentation prefixes. */
const nodes: TreeNode[] = [
  {
    id: "d1",
    label: "Centre de données nord",
    icon: "domain",
    children: [
      {
        id: "p1",
        label: "Serveurs",
        objectKey: "192.0.2.0/24",
        icon: "prefix",
        cells: { env: "Production", used: "62 %" },
        children: [
          {
            id: "p2",
            label: "Bases",
            objectKey: "192.0.2.0/26",
            icon: "prefix",
            cells: { env: "Production", used: "12 %" },
          },
          {
            id: "p3",
            label: "Supervision",
            objectKey: "192.0.2.64/26",
            icon: "prefix",
            cells: { env: "Recette", used: "45 %" },
          },
        ],
      },
      {
        id: "p4",
        label: "Postes",
        objectKey: "198.51.100.0/24",
        icon: "prefix",
        hasChildren: true,
        cells: { env: "Production", used: "87 %" },
      },
      {
        id: "p5",
        label: "Invités",
        objectKey: "203.0.113.0/24",
        icon: "prefix",
        cells: { env: "Développement", used: "3 %" },
      },
    ],
  },
];

const base = {
  label: "Arbre des préfixes",
  treeLabel: "Préfixe",
  nodes,
  columns,
  expanded: ["d1", "p1"],
};
const stateBy =
  (states: Record<string, TreeNodeState>) =>
  (node: TreeNode): TreeNodeState | undefined =>
    states[node.id];

/**
 * A draft moves « Supervision » under « Invités » (its value changed): its row at its
 * present place, and a preview row under its parent after the draft, as a host draws it.
 */
const previewed: TreeNode[] = nodes.map((domain) =>
  domain.children
    ? {
        ...domain,
        children: domain.children.map((node) =>
          node.id === "p5"
            ? {
                ...node,
                children: [
                  {
                    id: "preview:p3",
                    label: "Supervision",
                    objectKey: "203.0.113.64/26",
                    icon: "prefix",
                    cells: { env: "Recette", used: "45 %" },
                  },
                ],
              }
            : node,
        ),
      }
    : domain,
);

/** The entities of a declared tree: a parent chosen by the user. */
const entityColumns: GridColumn[] = [{ id: "owner", label: "Responsable", width: "md" }];

/** A drag held over a target, for the capture: the tree checks it, then marks it. */
const dragged = (target: string) =>
  defineComponent({
    setup() {
      const host = moveHost(keyboardTree());
      const root = ref<HTMLElement | null>(null);
      onMounted(() => dragOver(root.value, "n11", target));
      return () =>
        h("div", { ref: root }, [
          h(TreeGrid as Component, {
            label: "Entités",
            treeLabel: "Entité",
            nodes: host.nodes.value,
            columns: entityColumns,
            expanded: ["n1", "n11", "n2"],
            parentMode: "declared",
            canMove: host.canMove,
            moveState: host.moveState,
          }),
        ]);
    },
  });

/**
 * The keyboard script (e2e/components/tree-grid.spec.ts): entities of declared parents,
 * the label and the owner editable (the owner refused empty, the « server » refusing
 * « Personne »), moves checked (never under a descendant) and applied after a while, a
 * move into « Archives » refused by the « server » with its code; « Déplacer vers… » on
 * the chosen entity, as a screen's action would.
 */
const keyboard = defineComponent({
  setup() {
    const host = moveHost(keyboardTree(), ["n3"]);
    const expanded = ref(["n1", "n11", "n2"]);
    const selected = ref<string | undefined>("n12");
    const editing = ref<CellRef | undefined>(undefined);
    const states = ref(new Map<string, CellState>());
    const tree = ref<{ openMove(id: string): boolean } | null>(null);
    const said = ref("");
    const key = (node: TreeNode, column: GridColumn) => `${node.id}\n${column.id}`;
    const committed = ref("");
    function onCommit(commit: GridCommit): void {
      const k = `${commit.row}\n${commit.column}`;
      committed.value = `${commit.value}${commit.now ? " (appliquer maintenant)" : ""}`;
      const next = new Map(states.value);
      next.set(k, "pending");
      states.value = next;
      setTimeout(() => {
        const after = new Map(states.value);
        if (commit.value === "Personne")
          after.set(k, { error: "Responsable inconnu.", code: "entity.owner-unknown" });
        else {
          after.set(k, "draft");
          host.nodes.value = rename(host.nodes.value, commit);
        }
        states.value = after;
      }, 400);
    }
    return () =>
      h(Stack, { gap: "2", align: "stretch" }, () => [
        h("span", [
          h(
            Button,
            {
              size: "sm",
              icon: "move",
              reason: selected.value ? undefined : "Choisissez d'abord une entité.",
              onClick: () => selected.value && tree.value?.openMove(selected.value),
            },
            () => "Déplacer vers…",
          ),
        ]),
        h(TreeGrid as Component, {
          ref: tree,
          label: "Entités",
          treeLabel: "Entité",
          nodes: host.nodes.value,
          columns: entityColumns,
          expanded: expanded.value,
          "onUpdate:expanded": (next: string[]) => (expanded.value = next),
          selected: selected.value,
          "onUpdate:selected": (next: string) => (selected.value = next),
          editing: editing.value,
          "onUpdate:editing": (next: CellRef | undefined) => (editing.value = next),
          parentMode: "declared",
          canMove: host.canMove,
          moveState: host.moveState,
          onMove: host.onMove,
          // Every cell, but the label of the first entity (Enter activates its row).
          editable: (node: TreeNode, column: GridColumn) => node.id !== "n1" || column.id !== "",
          validate: (_: TreeNode, column: GridColumn, value: string) =>
            value.trim() === "" ? `${column.label} : une valeur est attendue.` : undefined,
          cellState: (node: TreeNode, column: GridColumn) => states.value.get(key(node, column)),
          onCommit,
          onActivate: (id: string) => (said.value = `Ouvrir ${id}`),
        }),
        said.value ? h("p", { "data-demo": "activated" }, said.value) : null,
        committed.value ? h("p", { "data-demo": "commit" }, committed.value) : null,
      ]);
  },
});

/** The label or a cell of a node changed by a commit. */
function rename(list: TreeNode[], commit: GridCommit): TreeNode[] {
  return list.map((node) => {
    const children = node.children ? rename(node.children, commit) : undefined;
    if (node.id !== commit.row) return children ? { ...node, children } : node;
    return commit.column === ""
      ? { ...node, label: commit.value, ...(children ? { children } : {}) }
      : {
          ...node,
          cells: { ...node.cells, [commit.column]: commit.value },
          ...(children ? { children } : {}),
        };
  });
}

/**
 * A hundred thousand nodes (a hundred domains of a thousand children), the first open:
 * a window of the rows in the document, the focused row kept; moves by the keyboard.
 */
const virtual = defineComponent({
  setup() {
    const host = moveHost(virtualTree(), [], 300);
    const expanded = ref(["v1"]);
    return () =>
      h(TreeGrid as Component, {
        label: "Cent mille nœuds",
        treeLabel: "Nœud",
        nodes: host.nodes.value,
        columns: [{ id: "n", label: "Rang", width: "sm", align: "end" }],
        expanded: expanded.value,
        "onUpdate:expanded": (next: string[]) => (expanded.value = next),
        parentMode: "declared",
        canMove: host.canMove,
        moveState: host.moveState,
        onMove: host.onMove,
      });
  },
});

/** Children read page by page: the host gives a page of five each time it is asked. */
const paged = defineComponent({
  setup() {
    const pages = pagedTree();
    const expanded = ref(["g1"]);
    return () =>
      h(Stack, { gap: "2", align: "stretch" }, () => [
        h(TreeGrid as Component, {
          label: "Groupes de VLAN",
          treeLabel: "Groupe",
          nodes: pages.nodes.value,
          expanded: expanded.value,
          "onUpdate:expanded": (next: string[]) => (expanded.value = next),
          nodeState: pages.nodeState,
          onLoad: pages.load,
        }),
        h(Cluster, { gap: "2" }, () => [
          h(Button, { size: "sm", onClick: pages.deliver }, () => "Envoyer les pages demandées"),
          h("span", { "data-demo": "pages" }, `Pages demandées : ${pages.asked.value}`),
        ]),
      ]);
  },
});

const editingTree = defineComponent({
  setup() {
    const editing = ref<CellRef | undefined>({ row: "p3", column: "env" });
    return () =>
      h(TreeGrid as Component, {
        ...base,
        editing: editing.value,
        "onUpdate:editing": (next: CellRef | undefined) => (editing.value = next),
        editable: (_: TreeNode, column: GridColumn) => column.id === "env",
        cellState: (node: TreeNode, column: GridColumn): CellState | undefined =>
          node.id === "p2" && column.id === "env"
            ? "pending"
            : node.id === "p5" && column.id === "env"
              ? { error: "Environnement inconnu.", code: "prefix.env-unknown" }
              : node.id === "p1" && column.id === "used"
                ? "review"
                : undefined,
      });
  },
});

const moveStates: Record<string, MoveState> = {
  n13: "pending",
  n21: {
    refused: { ok: false, reason: "Archives est verrouillé.", code: "entity.locked" },
  },
};

export default {
  component: TreeGrid,
  pattern: "Treegrid",
  states: [
    {
      name: "expanded",
      description: "Domaine et préfixe dépliés, un nœud sélectionné",
      props: { ...base, selected: "p2" },
      wide: true,
    },
    {
      name: "loading",
      description: "Chargement : une ligne de squelettes par nœud attendu, l'arbre occupé",
      props: { ...base, loading: true },
      wide: true,
    },
    {
      name: "settle",
      description:
        "Du squelette aux nœuds : les lignes prennent la place de leurs squelettes, rien ne bouge (CLS 0)",
      props: {},
      demo: settleDemo((loading) =>
        h(TreeGrid as Component, {
          ...base,
          loading,
          loadingRows: flatten(nodes, new Set(base.expanded)).length,
        }),
      ),
      wide: true,
    },
    {
      name: "loading-children",
      description: "Chargement des enfants d'un nœud",
      props: {
        ...base,
        expanded: ["d1", "p4"],
        nodeState: stateBy({ p4: "loading-children" }),
      },
      wide: true,
    },
    {
      name: "paged",
      description:
        "Enfants paginés : leur nombre inconnu (aria-setsize -1), la ligne de la suite lit la page suivante en venant en vue",
      props: {},
      demo: paged,
      wide: true,
    },
    {
      name: "draft",
      description: "Nœud en brouillon",
      props: { ...base, nodeState: stateBy({ p3: "draft" }) },
      wide: true,
    },
    {
      name: "moved-preview",
      description:
        "Nœud que le brouillon déplace : sa ligne au présent (déplacé dans le brouillon) et sa ligne d'aperçu à la place d'après",
      props: {
        ...base,
        nodes: previewed,
        expanded: ["d1", "p1", "p5"],
        nodeState: stateBy({ p3: "moved", "preview:p3": "preview" }),
      },
      wide: true,
    },
    {
      name: "editing",
      description:
        "Édition en place : éditeur ouvert, cellule en attente, en revue, en erreur (motif et code)",
      props: {},
      demo: editingTree,
      wide: true,
    },
    {
      name: "drop-accepted",
      description: "Glisser à parent déclaré : la cible acceptée par canMove, mise en évidence",
      props: {},
      demo: dragged("n2"),
      wide: true,
    },
    {
      name: "drop-refused",
      description:
        "Glisser sur un descendant : cible refusée, jamais mise en évidence, motif et code",
      props: {},
      demo: dragged("n111"),
      wide: true,
    },
    {
      name: "move-states",
      description: "Déplacement en attente (à sa place d'origine), déplacement refusé avec code",
      props: {
        label: "Entités",
        treeLabel: "Entité",
        nodes: keyboardTree(),
        columns: entityColumns,
        expanded: ["n1", "n2"],
        parentMode: "declared",
        moveState: (node: TreeNode) => moveStates[node.id],
      },
      wide: true,
    },
    {
      name: "add",
      description: "« + » d'un nœud : ajout direct dans le nœud choisi (survol, focus, sélection)",
      props: {
        ...base,
        selected: "p3",
        addLabel: (node: TreeNode) =>
          node.objectKey ? `Ajouter dans ${node.objectKey}` : undefined,
      },
      wide: true,
    },
    {
      name: "add-refused",
      description:
        "« + » refusé : visible, grisé, la permission manquante dite au survol et au focus (REQ-021-37)",
      props: {
        ...base,
        selected: "p5",
        addLabel: (node: TreeNode) =>
          node.objectKey ? `Ajouter dans ${node.objectKey}` : undefined,
        addBlocked: (node: TreeNode) =>
          node.id === "p5" ? { forbidden: "core.prefix.edit" } : undefined,
      },
      wide: true,
    },
    {
      name: "virtual",
      description:
        "Cent mille nœuds : une fenêtre dans le document, la ligne du focus gardée, déplacement au clavier",
      props: {},
      demo: virtual,
      wide: true,
    },
    {
      name: "keyboard",
      description:
        "Script du clavier : parents déclarés, libellé et responsable modifiables, Couper et Coller, « Déplacer vers… »",
      props: {},
      demo: keyboard,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
