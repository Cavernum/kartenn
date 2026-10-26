// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoFrame from "../../demo/DemoFrame.vue";
import { prefixes } from "../../fixtures/index.ts";
import Button from "../Button/Button.vue";
import DataGrid from "../DataGrid/DataGrid.vue";
import type { GridColumn } from "../DataGrid/grid.ts";
import Grid from "../Grid/Grid.vue";
import Stack from "../Stack/Stack.vue";
import StatusBadge from "../StatusBadge/StatusBadge.vue";
import type { TabItem } from "../Tabs/tabs.ts";
import BottomPanel from "./BottomPanel.vue";

const tabs: TabItem[] = [
  { value: "detail", label: "Détail" },
  { value: "history", label: "Historique", count: 3 },
];
const columns: GridColumn[] = [
  { id: "prefix", label: "Préfixe", width: "md", mono: true },
  { id: "label", label: "Libellé", width: "fill" },
  { id: "used", label: "Occupation", width: "sm", align: "end" },
];
const LABELS = ["Serveurs", "Supervision", "Réserve", "Salle B", "Invités", "Sauvegardes"];
const rows = prefixes.slice(0, 6).map((prefix, i) => ({
  prefix,
  label: LABELS[i]!,
  used: `${[91, 45, 12, 38, 100, 7][i]} %`,
}));
const ATTRIBUTES: [string, string][] = [
  ["Préfixe", "192.0.2.64/26"],
  ["Libellé", "Supervision"],
  ["Environnement", "Production"],
  ["Occupation", "45 %"],
  ["Propriétaire", "Atelier Nord"],
  ["VLAN", "121"],
];

/**
 * The frame of the panel: a surface (title, table) and the bottom panel under it; the table
 * keeps its whole width, the surface its height less the panel's. Closed, nothing remains:
 * the button of the surface reopens it.
 */
const demo = defineComponent({
  props: {
    open: Boolean,
    tab: { type: String, default: "detail" },
    height: { type: Number, default: 240 },
    sheet: Boolean,
  },
  setup(props) {
    const open = ref(props.open);
    const tab = ref(props.tab);
    const height = ref(props.height);
    return () =>
      h(
        DemoFrame,
        { size: props.sheet ? "phone" : "md", bottom: true },
        {
          default: () => [
            h("h2", { class: "k-cat-subtitle" }, "Contenu de 192.0.2.0/24"),
            h(DataGrid as Component, { label: "Préfixes", columns, rows, rowKey: "prefix" }),
            h(
              Button,
              { size: "sm", onClick: () => (open.value = true) },
              () => "Détail de 192.0.2.64/26",
            ),
          ],
          panel: () =>
            h(
              BottomPanel,
              {
                label: "Panneau du bas",
                tabs,
                sheet: props.sheet,
                open: open.value,
                "onUpdate:open": (v: boolean) => (open.value = v),
                tab: tab.value,
                "onUpdate:tab": (v: string) => (tab.value = v),
                height: height.value,
                "onUpdate:height": (v: number) => (height.value = v),
              },
              {
                // The detail in several columns: as many as the width holds.
                detail: () =>
                  h(Grid, { as: "dl", columns: 3, gap: "2" }, () =>
                    ATTRIBUTES.map(([term, value]) =>
                      h(Stack, { gap: "0" }, () => [h("dt", term), h("dd", value)]),
                    ),
                  ),
                history: () =>
                  h(Stack, { as: "ol", gap: "2" }, () =>
                    ["Créé", "Libellé changé", "VLAN ajouté"].map((what) =>
                      h("li", [
                        h(StatusBadge, { state: "neutral", label: "Appliqué" }),
                        ` ${what}`,
                      ]),
                    ),
                  ),
              },
            ),
        },
      );
  },
});

export default {
  component: BottomPanel,
  pattern:
    "région complementary (non modale) + Tabs ; poignée Window Splitter horizontale ; en feuille, Dialog (Modal)",
  states: [
    {
      name: "open",
      description:
        "Ouvert sous la surface, sur l'onglet Détail en plusieurs colonnes ; la table garde toute sa largeur",
      props: { open: true, tab: "detail" },
      demo,
      wide: true,
    },
    {
      name: "tabs",
      description: "Deuxième onglet (Historique)",
      props: { open: true, tab: "history" },
      demo,
      wide: true,
    },
    {
      name: "resized",
      description: "Redimensionné à 160 px (poignée ou menu « Hauteur du panneau » : Basse)",
      props: { open: true, tab: "detail", height: 160 },
      demo,
      wide: true,
    },
    {
      name: "sheet",
      description:
        "Fenêtre compacte : la même feuille plein écran que le panneau de droite, « Retour » au début",
      props: { open: true, tab: "detail", sheet: true },
      demo,
      wide: true,
    },
    {
      name: "closed",
      description: "Fermé : il ne laisse rien ; la surface reprend toute la hauteur",
      props: { open: false },
      demo,
      wide: true,
    },
    {
      name: "handle-focus",
      description: "Focus sur la poignée (Haut, Bas, Début, Fin) : trait de 2 px et sa pastille",
      props: { open: true, tab: "detail" },
      demo,
      wide: true,
      interaction: "focus",
      target: "[role='separator']",
    },
  ],
} satisfies CatalogueEntry;
