<script setup lang="ts">
// E27 — Fiches : onglet et actions (spec 017, task 1; points `tab` and `action` of the two
// plugins on the cards of the core, REQ-017-13, 15, 19, 33; U3; 021/X40): the tab « Flux »
// of an address and of a prefix (`GET /v1/{addresses,prefixes}/{id}/flows`: the flows
// whose object is an end, or one of whose ends contains it — the end in cause said), a
// pending request listed without a card (no identifier yet) but with its changeset; the
// menu « Actions » of an address, the actions of the core then those of the plugins:
// « Vérifier un flux » (`policy-check`, the compliance query with the source prefilled,
// E19), « Attendu d'inférence » (`policy-expected`, E18), « Demander un flux »
// (`flow-request-from`, the proposal prefilled, E21). The tabs of machines and interfaces
// are E11's.
import {
  Button,
  DataGrid,
  EnvChip,
  KeyLabel,
  Menu,
  Split,
  Stack,
  StatusBadge,
  Tabs,
  type GridColumn,
  type MenuItem,
  type State,
  type TabItem,
} from "@kartenn/ui";
import { ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { DOMAIN, ENV, SERVICES } from "./donnees.ts";
import { CHANGESET, SPACES_017, lien } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();
const prefix = props.vue === "prefixe";
const OBJECT = prefix ? "192.0.2.64/26" : "192.0.2.14";
const tab = ref(props.vue === "actions" ? "properties" : "flows");
const tabs: TabItem[] = [
  { value: "properties", label: "Propriétés", count: 5 },
  { value: "history", label: "Historique", count: 3 },
  { value: "flows", label: "Flux", icon: "flow", count: prefix ? 3 : 6 },
];

type FlowState = "active" | "requested" | "noncompliant";
const STATE: Record<FlowState, { state: State; label: string }> = {
  active: { state: "ok", label: "Actif" },
  requested: { state: "review", label: "Demandé" },
  noncompliant: { state: "ko", label: "Non conforme" },
};
interface Row extends Record<string, unknown> {
  key: string;
  direction: "in" | "out";
  flow: string;
  other: string;
  service: string;
  state: FlowState;
  by: string;
}
const rows: Row[] = prefix
  ? [
      {
        key: "r1",
        direction: "in",
        flow: "flux-1029",
        other: "srv-app-07 · eth0",
        service: "ssh",
        state: "noncompliant",
        by: "le préfixe lui-même",
      },
      {
        key: "r2",
        direction: "in",
        flow: "flux-1130",
        other: "bastion-01 · eth0",
        service: "ssh",
        state: "active",
        by: "192.0.2.72, qu'il contient",
      },
      {
        key: "r3",
        direction: "in",
        flow: "",
        other: "srv-web-02 · eth0",
        service: "https",
        state: "requested",
        by: "192.0.2.70, qu'il contient",
      },
    ]
  : [
      {
        key: "r1",
        direction: "in",
        flow: "flux-1041",
        other: "srv-web-02 · eth0",
        service: "api",
        state: "active",
        by: "son interface srv-app-01 · eth0",
      },
      {
        key: "r2",
        direction: "in",
        flow: "flux-1046",
        other: "192.0.2.16/28",
        service: "https",
        state: "active",
        by: "son interface srv-app-01 · eth0",
      },
      {
        key: "r3",
        direction: "in",
        flow: "flux-1120",
        other: "198.51.100.20",
        service: "https",
        state: "active",
        by: "192.0.2.0/26, qui la contient",
      },
      {
        key: "r4",
        direction: "in",
        flow: "",
        other: "srv-rec-03 · eth0",
        service: "https",
        state: "requested",
        by: "son interface srv-app-01 · eth0",
      },
      {
        key: "r5",
        direction: "out",
        flow: "flux-1042",
        other: "192.0.2.30",
        service: "postgres",
        state: "active",
        by: "son interface srv-app-01 · eth0",
      },
      {
        key: "r6",
        direction: "out",
        flow: "flux-1044",
        other: "198.51.100.40",
        service: "nfs",
        state: "active",
        by: "son interface srv-app-01 · eth0",
      },
    ];
const columns: GridColumn[] = [
  { id: "direction", label: "Sens", width: "sm", sortable: true },
  { id: "flow", label: "Flux", width: "sm", mono: true },
  { id: "other", label: "Autre extrémité", width: "md", mono: true, wrap: true },
  { id: "service", label: "Service", width: "md", priority: 3 },
  { id: "state", label: "État", width: "md", sortable: true },
  { id: "by", label: "Par", width: "fill", wrap: true, priority: 2 },
];

const menu: MenuItem[] = [
  { id: "edit", label: "Modifier", icon: "pencil", shortcut: ["e"] },
  { id: "copy", label: "Copier l'identifiant", icon: "copy" },
  { id: "date", label: "Lire à une autre date…", icon: "calendar" },
  { kind: "separator" },
  { kind: "label", label: "Plugins" },
  { id: "check", label: "Vérifier un flux", icon: "clipboard-check" },
  { id: "expected", label: "Attendu d'inférence…", icon: "git-compare" },
  { id: "request", label: "Demander un flux…", icon: "flow" },
  { kind: "separator" },
  { id: "close", label: "Clore…", icon: "trash", danger: true, forbidden: "core.address.apply" },
];
const register = lien("E11", "registre", route.query);
</script>

<template>
  <Cadre
    :title="prefix ? `Préfixe ${OBJECT}` : `Adresse ${OBJECT}`"
    :active="prefix ? 'prefixes' : 'addresses'"
    space="referential"
    :spaces="SPACES_017"
    :crumbs="[
      { label: 'Référentiel', to: '/' },
      { label: DOMAIN.label, to: `/domains/${DOMAIN.key}` },
      { label: prefix ? 'Préfixes' : 'Adresses', to: prefix ? '/prefixes' : '/addresses' },
      { label: OBJECT, mono: true },
    ]"
  >
    <Panneau
      :title="OBJECT"
      :detail="prefix ? 'préfixe · réseau · Supervision' : 'adresse · hôte · srv-app-01 · eth0'"
      main
    >
      <template #actions>
        <Button icon="pencil" :shortcut="['e']">Modifier</Button>
        <Menu
          label="Actions"
          icon="ellipsis"
          align="end"
          :items="menu"
          :default-open="props.vue === 'actions'"
        />
      </template>
      <Split side="end" side-size="sm" gap="3">
        <Tabs v-model="tab" :tabs="tabs" label="Sections de la fiche">
          <template #properties>
            <p class="k-ecran-muted">Propriétés résolues (E2).</p>
          </template>
          <template #flows>
            <Stack gap="2">
              <DataGrid :label="`Flux de ${OBJECT}`" :columns="columns" :rows="rows" row-key="key">
                <template #cell="{ row, column, value }">
                  <template v-if="column.id === 'direction'">{{
                    row.direction === "in" ? "Entrant" : "Sortant"
                  }}</template>
                  <template v-else-if="column.id === 'flow'">
                    <RouterLink v-if="row.flow" :to="lien('E22', 'fiche', route.query)">{{
                      row.flow
                    }}</RouterLink>
                    <a v-else :href="`/changesets/${CHANGESET.request}`" class="k-mono"
                      >changeset</a
                    >
                  </template>
                  <KeyLabel
                    v-else-if="column.id === 'service'"
                    :value="row.service"
                    :label="SERVICES[row.service]?.label ?? null"
                    show-key
                  />
                  <StatusBadge
                    v-else-if="column.id === 'state'"
                    :state="STATE[row.state].state"
                    :label="STATE[row.state].label"
                  />
                  <template v-else>{{ value }}</template>
                </template>
              </DataGrid>
              <p class="k-ecran-muted">
                Une demande en attente n'a pas encore de fiche : son changeset la montre.
                <RouterLink :to="register">Tous ses flux dans le registre</RouterLink>.
              </p>
            </Stack>
          </template>
        </Tabs>
        <template #side>
          <Panneau title="Attributs">
            <Attributs
              :items="[
                { key: 'domain', label: 'Domaine', value: DOMAIN.label },
                { key: 'env', label: 'Environnement' },
                { key: 'owner', label: 'Propriétaire', value: 'Atelier Nord' },
              ]"
            >
              <template #env><EnvChip v-bind="ENV.prod" /></template>
            </Attributs>
          </Panneau>
        </template>
      </Split>
    </Panneau>
  </Cadre>
</template>
