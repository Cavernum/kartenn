// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, onMounted, ref, type Component } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { moveHost } from "../../demo/treeHost.ts";
import { dragOver, keyboardTree, pagedTree, virtualTree } from "../../demo/trees.ts";
import Button from "../Button/Button.vue";
import Cluster from "../Cluster/Cluster.vue";
import type { CellState, GridCommit } from "../DataGrid/grid.ts";
import Stack from "../Stack/Stack.vue";
import type { MoveState, TreeNode, TreeNodeState } from "../TreeGrid/tree.ts";
import Tree from "./Tree.vue";

/** Sites and rooms, their parent declared. */
const nodes: TreeNode[] = [
  {
    id: "s1",
    label: "Site A",
    icon: "domain",
    detail: "3 salles",
    children: [
      { id: "s11", label: "Salle 1", icon: "folder", detail: "rez-de-chaussée" },
      {
        id: "s12",
        label: "Salle 2",
        icon: "folder",
        detail: "premier étage",
        children: [
          { id: "s121", label: "Baie 2-01", objectKey: "B2-01" },
          { id: "s122", label: "Baie 2-02", objectKey: "B2-02" },
        ],
      },
      { id: "s13", label: "Salle 3", icon: "folder", hasChildren: true },
    ],
  },
  { id: "s2", label: "Site B", icon: "domain", detail: "1 salle", hasChildren: true },
  { id: "s3", label: "Site C", icon: "domain" },
];
const base = { label: "Sites", nodes, expanded: ["s1", "s12"] };
const stateBy =
  (states: Record<string, TreeNodeState>) =>
  (node: TreeNode): TreeNodeState | undefined =>
    states[node.id];

/** A draft moves « Baie 2-02 » into « Salle 1 »: its present row, its preview row. */
const previewed: TreeNode[] = nodes.map((site) =>
  site.children
    ? {
        ...site,
        children: site.children.map((room) =>
          room.id === "s11"
            ? {
                ...room,
                children: [{ id: "preview:s122", label: "Baie 2-02", objectKey: "B2-02" }],
              }
            : room,
        ),
      }
    : site,
);

/** A drag held over a target; with `preview`, the target is a preview row. */
const dragged = (target: string, preview = false) =>
  defineComponent({
    setup() {
      const host = moveHost(keyboardTree());
      const root = ref<HTMLElement | null>(null);
      onMounted(() => dragOver(root.value, "n11", target));
      return () =>
        h("div", { ref: root }, [
          h(Tree as Component, {
            label: "Entités",
            nodes: host.nodes.value,
            expanded: ["n1", "n11", "n2"],
            parentMode: "declared",
            canMove: host.canMove,
            moveState: host.moveState,
            nodeState: preview ? stateBy({ [target]: "preview" }) : undefined,
          }),
        ]);
    },
  });

/**
 * The keyboard script (e2e/components/tree.spec.ts): entities of declared parents,
 * renamed by Enter or F2 (an empty label refused, « Personne » refused by the « server »),
 * moves checked and applied after a while, a move into « Archives » refused with its code;
 * « Déplacer vers… » on the chosen entity, as a screen's action would.
 */
const keyboard = defineComponent({
  setup() {
    const host = moveHost(keyboardTree(), ["n3"]);
    const expanded = ref(["n1"]);
    const selected = ref<string | undefined>(undefined);
    const editing = ref<string | undefined>(undefined);
    const labels = ref(new Map<string, CellState>());
    const tree = ref<{ openMove(id: string): boolean } | null>(null);
    const said = ref("");
    function onCommit(commit: GridCommit): void {
      const next = new Map(labels.value);
      next.set(commit.row, "pending");
      labels.value = next;
      setTimeout(() => {
        const after = new Map(labels.value);
        if (commit.value === "Personne")
          after.set(commit.row, { error: "Nom réservé.", code: "entity.name-reserved" });
        else {
          after.set(commit.row, "draft");
          host.nodes.value = relabel(host.nodes.value, commit.row, commit.value);
        }
        labels.value = after;
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
        h(Tree as Component, {
          ref: tree,
          label: "Entités",
          nodes: host.nodes.value,
          expanded: expanded.value,
          "onUpdate:expanded": (next: string[]) => (expanded.value = next),
          selected: selected.value,
          "onUpdate:selected": (next: string) => (selected.value = next),
          editing: editing.value,
          "onUpdate:editing": (next: string | undefined) => (editing.value = next),
          parentMode: "declared",
          canMove: host.canMove,
          moveState: host.moveState,
          onMove: host.onMove,
          // Every entity is renamed, but the first (Enter activates it).
          editable: (node: TreeNode) => node.id !== "n1",
          validate: (_: TreeNode, value: string) =>
            value.trim() === "" ? "Le nom est obligatoire." : undefined,
          labelState: (node: TreeNode) => labels.value.get(node.id),
          onCommit,
          onActivate: (id: string) => (said.value = `Ouvrir ${id}`),
        }),
        said.value ? h("p", { "data-demo": "activated" }, said.value) : null,
      ]);
  },
});

function relabel(list: TreeNode[], id: string, label: string): TreeNode[] {
  return list.map((node) => {
    const children = node.children ? relabel(node.children, id, label) : undefined;
    const next = node.id === id ? { ...node, label } : node;
    return children ? { ...next, children } : next;
  });
}

const virtual = defineComponent({
  setup() {
    const host = moveHost(virtualTree(), [], 300);
    const expanded = ref(["v1"]);
    return () =>
      h(Tree as Component, {
        label: "Cent mille nœuds",
        nodes: host.nodes.value,
        expanded: expanded.value,
        "onUpdate:expanded": (next: string[]) => (expanded.value = next),
        parentMode: "declared",
        canMove: host.canMove,
        moveState: host.moveState,
        onMove: host.onMove,
      });
  },
});

const paged = defineComponent({
  setup() {
    const pages = pagedTree();
    const expanded = ref(["g1"]);
    return () =>
      h(Stack, { gap: "2", align: "stretch" }, () => [
        h(Tree as Component, {
          label: "Groupes de VLAN",
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

const moveStates: Record<string, MoveState> = {
  n13: "pending",
  n21: { refused: { ok: false, reason: "Archives est verrouillé.", code: "entity.locked" } },
};

export default {
  component: Tree,
  pattern: "Tree View",
  states: [
    {
      name: "expanded",
      description: "Site et salle dépliés, une salle sélectionnée",
      props: { ...base, selected: "s12" },
    },
    {
      name: "loading",
      description: "Chargement : des lignes de squelettes, l'arbre occupé",
      props: { ...base, loading: true, loadingRows: 4 },
    },
    {
      name: "loading-children",
      description: "Chargement des enfants d'un nœud ouvert",
      props: {
        ...base,
        expanded: ["s1", "s13"],
        nodeState: stateBy({ s13: "loading-children" }),
      },
    },
    {
      name: "paged",
      description: "Enfants paginés : aria-setsize -1, la ligne de la suite lit la page suivante",
      props: {},
      demo: paged,
    },
    {
      name: "draft",
      description: "Nœud en brouillon",
      props: { ...base, nodeState: stateBy({ s11: "draft" }) },
    },
    {
      name: "moved-preview",
      description:
        "Nœud que le brouillon déplace : sa ligne au présent (déplacé dans le brouillon) et sa ligne d'aperçu sous son parent d'après",
      props: {
        ...base,
        nodes: previewed,
        expanded: ["s1", "s11", "s12"],
        nodeState: stateBy({ s122: "moved", "preview:s122": "preview" }),
      },
    },
    {
      name: "editing",
      description: "Renommer : le libellé dans un champ (Entrée ou F2)",
      props: { ...base, selected: "s11", editing: "s11", editable: () => true },
    },
    {
      name: "drop-accepted",
      description: "Glisser à parent déclaré : la cible acceptée par canMove, mise en évidence",
      props: {},
      demo: dragged("n2"),
    },
    {
      name: "drop-refused",
      description:
        "Glisser sur un descendant : cible refusée, jamais mise en évidence, motif et code",
      props: {},
      demo: dragged("n111"),
    },
    {
      name: "drop-accepted-preview",
      description:
        "Glisser au-dessus d'une ligne d'aperçu acceptée : le contour de la cible remplace le tireté de l'aperçu",
      props: {},
      demo: dragged("n2", true),
    },
    {
      name: "drop-refused-preview",
      description:
        "Glisser au-dessus d'une ligne d'aperçu refusée : le tireté de la cible refusée, motif et code",
      props: {},
      demo: dragged("n111", true),
    },
    {
      name: "move-states",
      description: "Déplacement en attente (à sa place), déplacement refusé avec code",
      props: {
        label: "Entités",
        nodes: keyboardTree(),
        expanded: ["n1", "n2"],
        parentMode: "declared",
        moveState: (node: TreeNode) => moveStates[node.id],
      },
    },
    {
      name: "virtual",
      description:
        "Cent mille nœuds : une fenêtre dans le document, la ligne du focus gardée, déplacement au clavier",
      props: {},
      demo: virtual,
    },
    {
      name: "keyboard",
      description:
        "Script du clavier : *, recherche à la saisie, renommer, Couper et Coller, « Déplacer vers… »",
      props: {},
      demo: keyboard,
    },
  ],
} satisfies CatalogueEntry;
