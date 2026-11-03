// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DataGrid from "../DataGrid/DataGrid.vue";
import type { GridColumn } from "../DataGrid/grid.ts";
import Stack from "../Stack/Stack.vue";
import type { TreeNode } from "../TreeGrid/tree.ts";
import TreeGrid from "../TreeGrid/TreeGrid.vue";
import NoValue from "./NoValue.vue";

const columns: GridColumn[] = [
  { id: "name", label: "Interface", width: "md", mono: true },
  { id: "mac", label: "Adresse MAC", width: "md", mono: true },
  { id: "note", label: "Note", width: "fill" },
];

/** A `DataGrid` and a `TreeGrid`: a cell `null` said absent, a cell `""` left blank. */
const grids: FunctionalComponent = () =>
  h(Stack, { gap: "3" }, () => [
    h(DataGrid as Component, {
      label: "Interfaces",
      columns,
      rowKey: "name",
      rows: [
        { name: "eth0", mac: "00-00-5E-00-53-01", note: "Gestion" },
        { name: "eth1", mac: null, note: "" },
      ],
    }),
    h(TreeGrid as Component, {
      label: "Hébergement",
      treeLabel: "Machine",
      columns: [{ id: "site", label: "Site", width: "md" }],
      expanded: ["c1"],
      nodes: [
        {
          id: "c1",
          label: "grappe-nord",
          icon: "cluster",
          cells: { site: "Site A" },
          children: [
            { id: "m1", label: "hv-nord-01", icon: "server", cells: { site: "Site A" } },
            { id: "m2", label: "hv-nord-02", icon: "server", cells: { site: null } },
          ],
        },
      ] satisfies TreeNode[],
    }),
  ]);

export default {
  component: NoValue,
  pattern: "texte (tiret caché aux lecteurs d'écran, mots lus à sa place)",
  states: [
    {
      name: "default",
      description: "Valeur absente : un tiret montré, « non renseigné » pour les lecteurs d'écran",
      props: {},
    },
    {
      name: "label",
      description:
        "Mots de la vue (une relation vide, un sans-objet) : le même tiret, « Aucun VLAN » lu",
      props: { label: "Aucun VLAN" },
    },
    {
      name: "grids",
      description:
        "Cellule null d'une DataGrid et d'une TreeGrid : dite absente ; une cellule \"\" reste vide",
      props: {},
      demo: grids,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
