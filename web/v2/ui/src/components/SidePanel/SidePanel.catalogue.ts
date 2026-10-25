// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref, type Component } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoFrame from "../../demo/DemoFrame.vue";
import { prefixes } from "../../fixtures/index.ts";
import Cluster from "../Cluster/Cluster.vue";
import CommitForm from "../CommitForm/CommitForm.vue";
import DataGrid from "../DataGrid/DataGrid.vue";
import type { GridColumn } from "../DataGrid/grid.ts";
import DraftCounter from "../DraftCounter/DraftCounter.vue";
import SplitButton from "../SplitButton/SplitButton.vue";
import type { SplitAction } from "../SplitButton/split.ts";
import Stack from "../Stack/Stack.vue";
import StatusBadge from "../StatusBadge/StatusBadge.vue";
import type { TabItem } from "../Tabs/tabs.ts";
import SidePanel from "./SidePanel.vue";

const PANEL = "k-demo-panel";
const tabs: TabItem[] = [
  { value: "detail", label: "Détail" },
  { value: "draft", label: "Brouillon", count: 3 },
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
const draftActions: SplitAction[] = [
  { id: "apply", label: "Appliquer", icon: "circle-check" },
  { id: "review", label: "Soumettre en revue", icon: "send" },
  { id: "save", label: "Enregistrer le brouillon", icon: "draft" },
];

/**
 * The frame of the panel: a surface (title, draft counter, table) and the panel over its
 * end; the counter opens the panel on its draft tab and, open, closes it; closed, the
 * panel leaves its strip (`rail`) at the end of the frame.
 */
const demo = defineComponent({
  props: {
    open: Boolean,
    tab: { type: String, default: "detail" },
    width: { type: Number, default: 420 },
    cover: Boolean,
  },
  setup(props) {
    const open = ref(props.open);
    const tab = ref(props.tab);
    const width = ref(props.width);
    const summary = ref("Maintenance du référentiel");
    const action = ref("apply");
    function toggle(): void {
      if (open.value) open.value = false;
      else {
        open.value = true;
        tab.value = "draft";
      }
    }
    return () =>
      h(
        DemoFrame,
        { size: "md" },
        {
          default: () => [
            // The counter at the start: the panel covers the end of the surface.
            h(Cluster, { gap: "3", wrap: false }, () => [
              h(DraftCounter, {
                count: 3,
                expanded: open.value,
                controls: PANEL,
                onToggle: toggle,
              }),
              h("h2", { class: "k-cat-subtitle" }, "Contenu de 192.0.2.0/24"),
            ]),
            h(DataGrid as Component, { label: "Préfixes", columns, rows, rowKey: "prefix" }),
          ],
          panel: () =>
            h(
              SidePanel,
              {
                id: PANEL,
                label: "Panneau droit",
                tabs,
                cover: props.cover,
                rail: true,
                open: open.value,
                "onUpdate:open": (v: boolean) => (open.value = v),
                tab: tab.value,
                "onUpdate:tab": (v: string) => (tab.value = v),
                width: width.value,
                "onUpdate:width": (v: number) => (width.value = v),
              },
              {
                detail: () =>
                  h(Stack, { gap: "2" }, () => [
                    h("p", { class: "k-mono" }, "192.0.2.64/26"),
                    h("p", "Supervision · Production"),
                    h("p", "Occupation : 45 %"),
                  ]),
                draft: () =>
                  h(Stack, { as: "ol", gap: "2" }, () =>
                    ["192.0.2.64/26", "192.0.2.200", "198.51.100.0/24"].map((key) =>
                      h("li", { class: "k-mono" }, [
                        h(StatusBadge, { state: "neutral", label: "Brouillon" }),
                        ` ${key}`,
                      ]),
                    ),
                  ),
                footer: ({ tab: current }: { tab: string }) =>
                  current === "draft"
                    ? [
                        h("p", [h(StatusBadge, { state: "ok", label: "Simulation valide" })]),
                        h(CommitForm, {
                          summary: summary.value,
                          "onUpdate:summary": (v: string) => (summary.value = v),
                          required: true,
                        }),
                        h(SplitButton, {
                          actions: draftActions,
                          action: action.value,
                          "onUpdate:action": (v: string) => (action.value = v),
                          shortcut: ["Control", "Enter"],
                        }),
                      ]
                    : null,
              },
            ),
        },
      );
  },
});

export default {
  component: SidePanel,
  pattern: "région complementary (non modale) + Tabs ; poignée Window Splitter",
  states: [
    {
      name: "detail",
      description: "Ouvert sur l'onglet Détail, en surimpression de la fin de la surface",
      props: { open: true, tab: "detail" },
      demo,
      wide: true,
    },
    {
      name: "draft",
      description: "Onglet Brouillon : simulation, motif (CommitForm), action (SplitButton)",
      props: { open: true, tab: "draft" },
      demo,
      wide: true,
    },
    {
      name: "resized",
      description: "Redimensionné à 560 px (poignée)",
      props: { open: true, tab: "detail", width: 560 },
      demo,
      wide: true,
    },
    {
      name: "cover",
      description: "Fenêtre étroite : le panneau couvre toute la surface, sans poignée",
      props: { open: true, tab: "draft", cover: true },
      demo,
      wide: true,
    },
    {
      name: "closed",
      description:
        "Replié : une languette au bord de fin, dans le flux, dont le chevron le rouvre (le compteur aussi)",
      props: { open: false },
      demo,
      wide: true,
    },
    {
      name: "rail-focus",
      description: "Focus sur le chevron de la languette « Ouvrir le panneau »",
      props: { open: false },
      demo,
      wide: true,
      interaction: "focus",
      target: ".k-side-panel-rail__open",
    },
    {
      name: "handle-focus",
      description: "Focus sur la poignée (flèches, Début, Fin)",
      props: { open: true, tab: "detail" },
      demo,
      wide: true,
      interaction: "focus",
      target: "[role='separator']",
    },
  ],
} satisfies CatalogueEntry;
