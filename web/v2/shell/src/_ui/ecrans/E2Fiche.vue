<script setup lang="ts">
// E2 — Fiche d'un préfixe (design « Maquettes »; 021, « Catalogue des écrans », Fiche
// d'objet et Historique): title and actions, tabs of the core then of the plugins,
// resolved properties with the explanation of an inherited value, occupancy and next free,
// history of bitemporal versions, menu of actions. Refused actions stay visible, greyed,
// their tooltip naming the missing permission (024/M14): no permanent box of the rights.
import {
  Button,
  CodeBadge,
  DataGrid,
  EnvChip,
  IconButton,
  InlineAlert,
  KeyLabel,
  Menu,
  Popover,
  Split,
  Stack,
  StatusBadge,
  Tabs,
  type GridColumn,
  type GridSort,
  type MenuItem,
  type TabItem,
} from "@kartenn/ui";
import { ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { DOMAIN, ENV, ID } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();
const tab = ref(props.vue === "historique" ? "history" : "properties");
const propertySort = ref<GridSort | undefined>({ column: "key", direction: "ascending" });
const versionSort = ref<GridSort | undefined>({ column: "version", direction: "descending" });
const tabs: TabItem[] = [
  { value: "properties", label: "Propriétés", count: 5 },
  { value: "content", label: "Contenu", count: 12 },
  { value: "history", label: "Historique", count: 4 },
  { value: "anomalies", label: "Anomalies", count: 1 },
  { value: "vlan", label: "VLAN" },
  { value: "machines", label: "Machines", icon: "server", count: 6 },
  { value: "flows", label: "Flux", icon: "flow", state: "loading" },
];

interface Property extends Record<string, unknown> {
  key: string;
  label: string;
  value: string;
  source: "inherited" | "direct" | "inferred" | "none" | "masked";
  origin: string;
}
const properties: Property[] = [
  {
    key: "core.owner",
    label: "Propriétaire",
    value: "Atelier Nord",
    source: "inherited",
    origin: "de 192.0.2.0/24",
  },
  { key: "core.site", label: "Site", value: "site-ouest", source: "direct", origin: "" },
  {
    key: "policy.environment",
    label: "Environnement",
    value: "prod",
    source: "inferred",
    origin: "règle env-par-site",
  },
  { key: "dns.ttl", label: "TTL DNS", value: "", source: "none", origin: "" },
  {
    key: "core.contact",
    label: "Contact",
    value: "",
    source: "masked",
    origin: "droit de lecture manquant",
  },
];
const propertyColumns: GridColumn[] = [
  { id: "key", label: "Propriété", width: "fill", sortable: true, filterable: true },
  { id: "value", label: "Valeur", width: "md", sortable: true },
  { id: "source", label: "Source", width: "lg", sortable: true, filterable: true },
];
const SOURCE: Record<Property["source"], string> = {
  inherited: "Héritée",
  direct: "Directe",
  inferred: "Inférée",
  none: "",
  masked: "Masquée",
};

interface Version extends Record<string, unknown> {
  version: string;
  valid: string;
  known: string;
  changeset: string;
  motive: string;
  author: string;
}
const versions: Version[] = [
  {
    version: "v4",
    valid: "2025-12-02 →",
    known: "2025-12-02",
    changeset: "01JD4K8B2C",
    motive: "Bascule des VIP salle B",
    author: "Camille Martin",
  },
  {
    version: "v3",
    valid: "2025-09-12 → 2025-12-02",
    known: "2025-09-12",
    changeset: "01JC9Q2H7R",
    motive: "Rattachement au site ouest",
    author: "Lou Bernard",
  },
  {
    version: "v2",
    valid: "2025-06-01 → 2025-09-12",
    known: "2025-06-03",
    changeset: "01JB1T5W3E",
    motive: "Correction rétroactive du propriétaire",
    author: "Lou Bernard",
  },
  {
    version: "v1",
    valid: "2025-03-17 → 2025-06-01",
    known: "2025-03-17",
    changeset: "01JA0N8D4F",
    motive: "Import initial",
    author: "import-ipam",
  },
];
const versionColumns: GridColumn[] = [
  { id: "version", label: "Version, changeset", width: "md", mono: true, sortable: true },
  { id: "valid", label: "Valide", width: "lg", mono: true, sortable: true },
  { id: "known", label: "Connue du", width: "md", mono: true, sortable: true },
  { id: "motive", label: "Motif", width: "fill", filterable: true },
];

const actions: MenuItem[] = [
  { id: "edit", label: "Modifier", icon: "pencil", shortcut: ["e"] },
  { id: "copy", label: "Copier l'identifiant", icon: "copy" },
  { id: "date", label: "Lire à une autre date…", icon: "calendar" },
  {
    id: "domain",
    label: "Changer de domaine…",
    icon: "move",
    disabled: true,
    reason: "Le domaine d'un objet ne change pas après sa création (core.domain.immutable).",
  },
  { kind: "separator" },
  { kind: "label", label: "Plugins" },
  { id: "compliance", label: "Vérifier la conformité", icon: "clipboard-check" },
  { id: "flow", label: "Demander un flux…", icon: "flow", forbidden: "flow.request" },
  { kind: "separator" },
  { id: "close", label: "Clore…", icon: "trash", danger: true, forbidden: "core.prefix.apply" },
];
</script>

<template>
  <Cadre
    title="Préfixe 192.0.2.64/26"
    active="prefixes"
    space="referential"
    :crumbs="[
      { label: 'Référentiel', to: '/' },
      { label: DOMAIN.label, to: `/domains/${DOMAIN.key}` },
      { label: 'Préfixes', to: '/prefixes' },
      { label: '192.0.2.64/26', mono: true },
    ]"
  >
    <Panneau title="192.0.2.64/26" detail="préfixe · réseau · Supervision" main>
      <template #actions>
        <Button icon="pencil" :shortcut="['e']">Modifier</Button>
        <Menu
          label="Actions"
          icon="ellipsis"
          align="end"
          :items="actions"
          :default-open="props.vue === 'actions'"
        />
      </template>
      <Split side="end" side-size="sm" gap="3">
        <Tabs v-model="tab" :tabs="tabs" label="Sections de la fiche">
          <template #properties>
            <Stack gap="3">
              <DataGrid
                v-model:sort="propertySort"
                label="Propriétés résolues de 192.0.2.64/26"
                :columns="propertyColumns"
                :rows="properties"
                row-key="key"
                :active="{ row: 'core.owner', column: 'source' }"
              >
                <template #cell="{ row, column }">
                  <KeyLabel
                    v-if="column.id === 'key'"
                    :value="row.key"
                    :label="row.label"
                    show-key
                  />
                  <template v-else-if="column.id === 'value'">
                    <EnvChip v-if="row.key === 'policy.environment'" v-bind="ENV.prod" />
                    <span v-else-if="row.source === 'none'">—</span>
                    <span v-else-if="row.source === 'masked'">••••••</span>
                    <template v-else>{{ row.value }}</template>
                  </template>
                  <StatusBadge v-else-if="row.source === 'none'" state="warn" label="Sans valeur" />
                  <template v-else
                    >{{ SOURCE[row.source] }}
                    <span v-if="row.origin" class="k-ecran-muted">{{ row.origin }}</span>
                    <!-- The explanation of an inherited value, beside it (021, Fiche d'objet). -->
                    <Popover
                      v-if="row.source === 'inherited'"
                      :title="`${row.label} : valeur héritée`"
                      :default-open="props.vue === 'explication'"
                    >
                      <template #trigger>
                        <IconButton
                          icon="info"
                          :label="`Expliquer la valeur : ${row.label}`"
                          size="sm"
                          variant="ghost"
                        />
                      </template>
                      <p>
                        Héritée de <code>192.0.2.0/24</code> (Salle A), où elle est posée
                        directement depuis la version v3 ; aucune règle ne la remplace ici.
                      </p>
                      <p>Posée par le changeset <CodeBadge :code="ID.changeset" />.</p>
                    </Popover></template
                  >
                </template>
              </DataGrid>
              <InlineAlert tone="warn" title="Une propriété attendue manque">
                Le TTL DNS (<code>dns.ttl</code>) est attendu sur les réseaux de production.
                <CodeBadge code="prop.assignment.required" />
              </InlineAlert>
            </Stack>
          </template>
          <template #history>
            <DataGrid
              v-model:sort="versionSort"
              label="Versions de 192.0.2.64/26"
              :columns="versionColumns"
              :rows="versions"
              row-key="version"
            >
              <template #cell="{ row, column, value }">
                <template v-if="column.id === 'version'"
                  >{{ row.version }}
                  <a :href="`/changesets/${row.changeset}`">{{ row.changeset }}</a></template
                >
                <template v-else>{{ value }}</template>
              </template>
            </DataGrid>
          </template>
        </Tabs>
        <template #side>
          <Stack gap="3">
            <Panneau title="Attributs">
              <Attributs
                :items="[
                  { key: 'domain', label: 'Domaine', value: DOMAIN.label },
                  { key: 'nature', label: 'Nature' },
                  { key: 'p2p', label: 'Point à point', value: 'non' },
                  { key: 'anycast', label: 'Anycast', value: 'non' },
                  { key: 'id', label: 'Identifiant' },
                ]"
              >
                <template #nature><KeyLabel value="network" label="Réseau" /></template>
                <template #id><CodeBadge :code="ID.prefix" /></template>
              </Attributs>
            </Panneau>
            <Panneau title="Occupation" detail="45 %">
              <Attributs
                :items="[
                  { key: 'used', label: 'Utilisées', value: '29 sur 62' },
                  { key: 'ranges', label: 'Plages', value: '1 (25 adresses)' },
                  { key: 'free', label: 'Prochain libre', value: '192.0.2.103', mono: true },
                ]"
              />
              <div>
                <Button size="sm" icon="plus" forbidden="core.address.edit"
                  >Réserver 192.0.2.103</Button
                >
              </div>
            </Panneau>
            <Panneau title="VLAN porté" detail="core.vlan" mono>
              <Attributs
                :items="[
                  { key: 'vid', label: 'VID', value: '121', mono: true },
                  { key: 'name', label: 'Nom' },
                  { key: 'group', label: 'Groupe', value: 'campus-ouest', mono: true },
                ]"
              >
                <template #name>
                  <RouterLink
                    :to="{ path: '/_ui/ecrans/E12', query: { ...route.query, vue: 'vlan' } }"
                    >supervision</RouterLink
                  >
                </template>
              </Attributs>
            </Panneau>
          </Stack>
        </template>
      </Split>
    </Panneau>
  </Cadre>
</template>
