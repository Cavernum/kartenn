// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Cluster from "../Cluster/Cluster.vue";
import SidebarNav from "./SidebarNav.vue";
import type { SidebarGroup } from "./sidebar.ts";

/** The spaces of spec 021 (« Cadre »), shortened. */
const groups: SidebarGroup[] = [
  {
    id: "referential",
    label: "Référentiel",
    entries: [
      { id: "home", label: "Accueil", icon: "house", to: "/" },
      { id: "topology", label: "Topographie", icon: "folder-tree", to: "/topology" },
      { id: "vlans", label: "VLAN", icon: "vlan", to: "/vlans" },
      { id: "domains", label: "Domaines", icon: "domain", to: "/domains" },
      { id: "prefixes", label: "Préfixes", icon: "prefix", to: "/prefixes" },
      { id: "addresses", label: "Adresses", icon: "address", to: "/addresses" },
      { id: "anomalies", label: "Anomalies", icon: "triangle-alert", to: "/anomalies", badge: 4 },
    ],
  },
  {
    id: "changes",
    label: "Changesets",
    entries: [
      { id: "changesets", label: "Changesets", icon: "changeset", to: "/changesets" },
      {
        id: "proposals",
        label: "À instruire",
        icon: "git-pull-request",
        to: "/proposals",
        badge: 2,
      },
      { id: "draft", label: "Brouillon", icon: "draft", to: "/draft" },
    ],
  },
  {
    id: "admin",
    label: "Administration",
    entries: [{ id: "admin", label: "Administration", icon: "settings", to: "/admin" }],
  },
];

/**
 * The sidebar of the keyboard script (e2e/components/sidebar-nav.spec.ts), collapsed at
 * first, its state kept by the host (the shell keeps it in `localStorage`); « Rechercher »
 * says that it would open the palette.
 */
const Keyboard = defineComponent({
  setup() {
    const collapsed = ref(true);
    const palette = ref(0);
    return () =>
      h(Cluster, { gap: "3", align: "start", wrap: false }, () => [
        h(SidebarNav, {
          brand: "kartenn",
          groups,
          active: "prefixes",
          collapsed: collapsed.value,
          "onUpdate:collapsed": (next: boolean) => (collapsed.value = next),
          onPalette: () => palette.value++,
        }),
        h("p", { class: "k-cat-note", "data-palette": "" }, `Palette demandée : ${palette.value}`),
      ]);
  },
});

export default {
  component: SidebarNav,
  pattern: "Disclosure dans un repère navigation",
  states: [
    {
      name: "expanded",
      description: "Dépliée, entrée active, compteurs",
      props: { brand: "kartenn", groups, active: "prefixes" },
    },
    {
      name: "group-closed",
      description: "Un espace replié",
      props: { brand: "kartenn", groups, active: "prefixes", closed: ["changes"] },
    },
    {
      name: "collapsed",
      description: "Repliée en icônes (noms gardés pour les technologies d'assistance)",
      props: { brand: "kartenn", groups, active: "prefixes", collapsed: true },
    },
    {
      name: "collapsed-focus",
      description:
        "Repliée, focus clavier sur une entrée : infobulle de son nom et de son compteur, après elle",
      props: { brand: "kartenn", groups, active: "prefixes", collapsed: true },
      interaction: "focus",
      target: "a[href='/anomalies']",
    },
    {
      name: "collapsed-hover",
      description: "Repliée, survol du bouton de repli : son nom et son raccourci",
      props: { brand: "kartenn", groups, active: "prefixes", collapsed: true },
      interaction: "hover",
      target: ".k-sidebar__collapse",
    },
    {
      name: "hover",
      description: "Survol d'une entrée",
      props: { brand: "kartenn", groups, active: "prefixes" },
      interaction: "hover",
      target: "a[href='/addresses']",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { brand: "kartenn", groups, active: "prefixes" },
      interaction: "focus",
      target: "a[href='/domains']",
    },
    {
      name: "keyboard",
      description:
        "Au clavier : repliée, noms gardés et infobulles au focus ; dépliée, espaces en Disclosure",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
