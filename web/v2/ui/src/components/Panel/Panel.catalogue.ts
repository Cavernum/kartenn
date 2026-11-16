// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h, type Component, type FunctionalComponent } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoColumn from "../../demo/DemoColumn.vue";
import DemoHost from "../../demo/DemoHost.vue";
import { environments, prefixes } from "../../fixtures/index.ts";
import Button from "../Button/Button.vue";
import DataGrid from "../DataGrid/DataGrid.vue";
import type { GridColumn } from "../DataGrid/grid.ts";
import TextField from "../TextField/TextField.vue";
import Toolbar from "../Toolbar/Toolbar.vue";
import type { ToolbarAction } from "../Toolbar/toolbar.ts";
import PanelSfc from "./Panel.vue";

/** The props of a state, passed through as they are. */
const Panel = PanelSfc as Component;

const columns: GridColumn[] = [
  { id: "prefix", label: "Préfixe", width: "md", mono: true },
  { id: "label", label: "Libellé", width: "fill" },
  { id: "env", label: "Environnement", width: "md" },
];
const LABELS = ["Serveurs de bases", "Postes du siège", "Supervision", "Sauvegardes", "Invités"];
const rows = prefixes.slice(0, 5).map((prefix, i) => ({
  prefix,
  label: LABELS[i]!,
  env: environments[i % 3]!.label,
}));
/** More than 200 rows: the grid is virtualised. */
const many = Array.from({ length: 1_000 }, (_, i) => ({
  prefix: `10.${Math.floor(i / 256)}.${i % 256}.0/24`,
  label: `${LABELS[i % LABELS.length]} ${i + 1}`,
  env: environments[i % 3]!.label,
}));

const actions: ToolbarAction[] = [
  { id: "create", label: "Créer", icon: "plus", variant: "primary", priority: 3 },
  { id: "import", label: "Importer", icon: "arrow-down", priority: 2 },
  { id: "export", label: "Exporter", icon: "arrow-up" },
];

const body = () => h("p", "Les préfixes du domaine, avec leur environnement et leur occupation.");
const create = () => h(Button, { icon: "plus" }, () => "Nouveau préfixe");
const grid = (gridRows: object[]) =>
  h(DataGrid as Component, {
    label: "Préfixes du domaine",
    columns,
    rows: gridRows,
    rowKey: "prefix",
  });

/** A titled panel, its one action at the end of the header. */
const rest: FunctionalComponent = (props) => h(Panel, props, { actions: create, default: body });

/** A toolbar in the header: it takes the rest of the line and folds into « … ». */
const withToolbar: FunctionalComponent = (props) =>
  h(Panel, props, {
    actions: () => h(Toolbar, { label: "Actions sur les préfixes", actions, align: "end" }),
    default: body,
  });

/** The bar under the header, then the body, then the footer. */
const parts: FunctionalComponent = (props) =>
  h(Panel, props, {
    toolbar: () => h(TextField, { label: "Rechercher", type: "search", icon: "search" }),
    default: () => grid(rows),
    footer: () => [h("span", "5 préfixes"), h(Button, { variant: "ghost" }, () => "Exporter")],
  });

/** A narrow tile: the title goes to the line, the action to a line of its own. */
const narrow: FunctionalComponent = (props) =>
  h(DemoColumn, { size: "sm" }, () => h(Panel, props, { actions: create, default: body }));

/** A panel narrower than its actions: the title stays whole, the toolbar folds into « … ». */
const folded: FunctionalComponent = (props) =>
  h(DemoColumn, { size: "md" }, () => h(withToolbar, props));

/**
 * A list screen: the host gives the panel its height (the shell's surface), the virtualised
 * grid takes the rest of it, uncapped, and is its one scroller.
 */
const list: FunctionalComponent = (props) =>
  h(DemoHost, () =>
    h(Panel, props, {
      toolbar: () => h(TextField, { label: "Rechercher", type: "search", icon: "search" }),
      default: () => grid(many),
    }),
  );

/** The same panel without a host that gives it a height: the grid keeps a list's height. */
const listAlone: FunctionalComponent = (props) =>
  h(Panel, props, {
    toolbar: () => h(TextField, { label: "Rechercher", type: "search", icon: "search" }),
    default: () => grid(many),
  });

export default {
  component: Panel,
  pattern: "section (titre h2 ; région nommée pour le panneau principal seul)",
  states: [
    {
      name: "rest",
      description: "Panneau titré, son détail après le titre, son action au bout de l'en-tête",
      props: { title: "Préfixes", detail: "5" },
      demo: rest,
      wide: true,
    },
    {
      name: "main",
      description:
        "Panneau principal : titre plus grand, région nommée par lui ; détail en chasse fixe",
      props: { title: "Préfixe", detail: "192.0.2.0/24", mono: true, main: true },
      demo: rest,
      wide: true,
    },
    {
      name: "toolbar",
      description: "Barre d'actions dans l'en-tête : elle prend le reste de la ligne",
      props: { title: "Préfixes", detail: "5" },
      demo: withToolbar,
      wide: true,
    },
    {
      name: "toolbar-folded",
      description:
        "Panneau plus étroit que ses actions : le titre reste entier, la barre replie dans « … »",
      props: { title: "Préfixes", detail: "5" },
      demo: folded,
    },
    {
      name: "parts",
      description: "Barre sous l'en-tête, corps sans marge (une grille), pied",
      props: { title: "Préfixes", detail: "5", flush: true },
      demo: parts,
      wide: true,
    },
    {
      name: "long",
      description: "Titre plus long que la tuile : il va à la ligne, l'action sur une ligne à elle",
      props: {
        title: "srv-2001-db8-0-1-supervision-collecteur-principal-nord-01.example.net",
        detail: "12 interfaces",
      },
      demo: narrow,
    },
    {
      name: "list",
      description:
        "Écran de liste : l'hôte donne la hauteur, la grille virtualisée en prend le reste et défile seule",
      props: {
        title: "Préfixes",
        detail: many.length.toLocaleString("fr-FR"),
        main: true,
        flush: true,
        list: true,
      },
      demo: list,
      wide: true,
    },
    {
      name: "list-alone",
      description:
        "Panneau de liste sans hôte qui le remplit (onglet, tiroir) : la grille garde la hauteur d'une liste",
      props: {
        title: "Préfixes",
        detail: many.length.toLocaleString("fr-FR"),
        list: true,
        flush: true,
      },
      demo: listAlone,
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
