<script setup lang="ts">
// E12 — VLAN (design « Maquettes »; 026, REQ-026-6: the screens of the VLAN are screens of
// the core, no longer a plugin; behaviour of the 008, REQ-008-12 to 21). Four views: the
// groups (anchor, VID range, occupation in a `BarList`); a group (its occupation in
// slices used, reserved and free, the next free VID, from the smallest or from a VID, its
// VLAN); the card of a VLAN (VID, name, status, group, the networks that carry it, read
// under the rights of the caller: a network one cannot read is neither listed nor
// counted); creating a VLAN for a network, from the list of prefixes and its VLAN column
// (the property `core.vlan` of a network), in the right panel. Names of the core:
// permissions `core.vlan_group.*` and `core.vlan.*`, codes `core.vlan.*`.
import {
  BarList,
  Button,
  CodeBadge,
  Cluster,
  DataGrid,
  Icon,
  InlineAlert,
  KeyLabel,
  Menu,
  Split,
  SplitButton,
  Stack,
  StatusBadge,
  Tabs,
  TextField,
  type ChartPoint,
  type GridColumn,
  type GridSort,
  type IconName,
  type MenuItem,
  type RowState,
  type SplitAction,
  type State,
  type TabItem,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
import { RouterLink, useRoute, type RouteLocationRaw } from "vue-router";
import AVenir from "./AVenir.vue";
import Attributs from "./EcranAttributs.vue";
import Barre from "./EcranBarre.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { DOMAIN } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();

// Links between the views keep the switches of the catalogue (mode, density, language).
const SWITCHES = ["mode", "densite", "langue"] as const;
function view(vue: string): RouteLocationRaw {
  const query: Record<string, string> = { vue };
  for (const key of SWITCHES) {
    const value = route.query[key];
    if (typeof value === "string") query[key] = value;
  }
  return { path: "/_ui/ecrans/E12", query };
}

// --- Groups (REQ-008-12, 19, 20) --------------------------------------------------------

/** What a group is anchored on: exactly one entity, site or addressing domain. */
type AnchorKind = "entity" | "site" | "domain";
const ANCHOR: Record<AnchorKind, { label: string; icon: IconName }> = {
  entity: { label: "Entité", icon: "users" },
  site: { label: "Site", icon: "globe" },
  domain: { label: "Domaine", icon: "domain" },
};
interface Group extends Record<string, unknown> {
  key: string;
  anchorKind: AnchorKind;
  anchor: string;
  range: string;
  vlans: number;
  occupation: number;
  free: number;
}
const groups: Group[] = [
  {
    key: "campus-ouest",
    anchorKind: "site",
    anchor: "site-ouest",
    range: "100–199",
    vlans: 87,
    occupation: 87,
    free: 13,
  },
  {
    key: "campus-est",
    anchorKind: "site",
    anchor: "site-est",
    range: "200–299",
    vlans: 41,
    occupation: 41,
    free: 59,
  },
  {
    key: "labo",
    anchorKind: "domain",
    anchor: "dmz-labo",
    range: "900–999",
    vlans: 6,
    occupation: 6,
    free: 94,
  },
  {
    key: "atelier-nord",
    anchorKind: "entity",
    anchor: "atelier-nord",
    range: "2–99",
    vlans: 19,
    occupation: 19,
    free: 79,
  },
];
const groupColumns: GridColumn[] = [
  { id: "key", label: "Groupe", width: "fill", mono: true, sortable: true },
  { id: "anchor", label: "Ancre", width: "md", filterable: true, priority: 2 },
  { id: "range", label: "Plage de VID", width: "md", mono: true, priority: 2 },
  { id: "vlans", label: "VLAN", width: "sm", align: "end", sortable: true, priority: 3 },
  { id: "occupation", label: "Occupation", width: "sm", align: "end", sortable: true },
  { id: "free", label: "Libres", width: "sm", align: "end", sortable: true, priority: 3 },
];
const groupSort = ref<GridSort | undefined>({ column: "key", direction: "ascending" });
const occupation: ChartPoint[] = groups.map((g) => ({
  key: g.key,
  label: g.key,
  value: g.occupation,
}));
const percent = (value: number) => `${value} %`;
const groupsActions: ToolbarAction[] = [
  {
    id: "create",
    label: "Créer un groupe",
    icon: "plus",
    variant: "primary",
    shortcut: ["c"],
    priority: 3,
    forbidden: "core.vlan_group.edit",
  },
  { id: "export", label: "Exporter", priority: 1 },
];

// --- One group: occupation in slices, next free VID, its VLAN (REQ-008-20) ---------------

type SliceKind = "used" | "reserved" | "free";
const SLICE: Record<SliceKind, { state: State; label: string }> = {
  used: { state: "ok", label: "Utilisés" },
  reserved: { state: "neutral", label: "Réservés" },
  free: { state: "info", label: "Libres" },
};
interface Slice extends Record<string, unknown> {
  key: string;
  kind: SliceKind;
  count: number;
}
/** The slices of campus-ouest (VID 100–199): 75 used, 12 reserved, 13 free. */
const slices: Slice[] = [
  { key: "100–104", kind: "used", count: 5 },
  { key: "105–109", kind: "free", count: 5 },
  { key: "110–121", kind: "used", count: 12 },
  { key: "122–129", kind: "free", count: 8 },
  { key: "130–141", kind: "reserved", count: 12 },
  { key: "142–199", kind: "used", count: 58 },
];
const sliceColumns: GridColumn[] = [
  { id: "key", label: "VID", width: "fill", mono: true },
  { id: "kind", label: "État", width: "md" },
];
const counts: ChartPoint[] = (["used", "reserved", "free"] as const).map((kind) => ({
  key: kind,
  label: SLICE[kind].label,
  value: slices.filter((s) => s.kind === kind).reduce((n, s) => n + s.count, 0),
}));
const fromVid = ref("120");

type VlanStatus = "active" | "reserved" | "deprecated";
const STATUS: Record<VlanStatus, { state: State; label: string }> = {
  active: { state: "ok", label: "Actif" },
  reserved: { state: "neutral", label: "Réservé" },
  deprecated: { state: "warn", label: "Déprécié" },
};
interface Vlan extends Record<string, unknown> {
  vid: string;
  name: string;
  status: VlanStatus;
  networks: string;
}
const vlans: Vlan[] = [
  { vid: "100", name: "admin", status: "active", networks: "192.0.2.0/27" },
  { vid: "101", name: "serveurs", status: "active", networks: "2 réseaux" },
  { vid: "102", name: "voix", status: "deprecated", networks: "—" },
  { vid: "103", name: "sauvegarde", status: "active", networks: "198.51.100.0/26" },
  { vid: "104", name: "impression-a", status: "active", networks: "—" },
  { vid: "110", name: "wifi-invites", status: "active", networks: "203.0.113.0/24" },
  { vid: "120", name: "serveurs-b", status: "active", networks: "192.0.2.0/26" },
  { vid: "121", name: "supervision", status: "active", networks: "3 réseaux" },
  { vid: "130", name: "reserve-wifi", status: "reserved", networks: "—" },
];
const vlanColumns: GridColumn[] = [
  { id: "vid", label: "VID", width: "sm", mono: true, sortable: true },
  { id: "name", label: "Nom", width: "fill", mono: true, sortable: true },
  { id: "status", label: "Statut", width: "md", sortable: true, filterable: true },
  { id: "networks", label: "Réseaux", width: "lg", mono: true },
];
const vlanSort = ref<GridSort | undefined>({ column: "vid", direction: "ascending" });
const vlanRow = (row: Vlan): RowState | undefined => (row.vid === "104" ? "draft" : undefined);
const groupActions: ToolbarAction[] = [
  {
    id: "create",
    label: "Créer un VLAN",
    icon: "plus",
    variant: "primary",
    shortcut: ["c"],
    priority: 3,
  },
  {
    id: "edit",
    label: "Modifier le groupe",
    icon: "pencil",
    priority: 2,
    forbidden: "core.vlan_group.edit",
  },
  { id: "export", label: "Exporter", priority: 1 },
];

// --- The card of a VLAN: the networks that carry it (REQ-008-17, 18, 21) -----------------

const vlanTab = ref("networks");
const vlanTabs: TabItem[] = [
  { value: "networks", label: "Réseaux", count: 3 },
  { value: "history", label: "Historique", count: 3 },
  { value: "anomalies", label: "Anomalies", count: 1 },
  { value: "interfaces", label: "Interfaces", icon: "server", count: 6 },
];
interface Network extends Record<string, unknown> {
  key: string;
  name: string;
  site: string;
  coherence: "ok" | "site";
}
const networks: Network[] = [
  { key: "192.0.2.64/26", name: "Supervision", site: "site-ouest", coherence: "ok" },
  { key: "2001:db8:10:121::/64", name: "Supervision", site: "site-ouest", coherence: "ok" },
  { key: "198.51.100.64/26", name: "Supervision B", site: "site-est", coherence: "site" },
];
const networkColumns: GridColumn[] = [
  { id: "key", label: "Réseau", width: "fill", mono: true, sortable: true },
  { id: "name", label: "Nom", width: "md", sortable: true, priority: 2 },
  { id: "site", label: "Site", width: "md", mono: true, filterable: true, priority: 3 },
  { id: "coherence", label: "Cohérence", width: "lg" },
];
const networkSort = ref<GridSort | undefined>({ column: "key", direction: "ascending" });
const vlanMenu: MenuItem[] = [
  { id: "copy", label: "Copier l'identifiant", icon: "copy" },
  { id: "date", label: "Lire à une autre date…", icon: "calendar" },
  { id: "group", label: "Changer de groupe…", icon: "move" },
  { kind: "separator" },
  { id: "close", label: "Clore…", icon: "trash", danger: true, forbidden: "core.vlan.apply" },
];

// --- Creating a VLAN for a network, from the list of prefixes (REQ-008-14, 17) -----------

type Nature = "container" | "network";
const NATURE: Record<Nature, string> = { container: "Conteneur", network: "Réseau" };
interface Prefix extends Record<string, unknown> {
  key: string;
  name: string;
  nature: Nature;
  site: string;
  /** The VLAN of a network (`core.vlan`): VID and name; none on a container. */
  vlan: string;
}
const prefixes: Prefix[] = [
  { key: "192.0.2.0/24", name: "Salle A", nature: "container", site: "site-ouest", vlan: "" },
  {
    key: "192.0.2.0/26",
    name: "Serveurs",
    nature: "network",
    site: "site-ouest",
    vlan: "120 serveurs-b",
  },
  {
    key: "192.0.2.64/26",
    name: "Supervision",
    nature: "network",
    site: "site-ouest",
    vlan: "121 supervision",
  },
  { key: "192.0.2.128/26", name: "Réserve", nature: "container", site: "site-ouest", vlan: "" },
  { key: "192.0.2.192/26", name: "Impression", nature: "network", site: "site-ouest", vlan: "—" },
  { key: "198.51.100.0/24", name: "Salle B", nature: "container", site: "site-est", vlan: "" },
  {
    key: "198.51.100.64/26",
    name: "Supervision B",
    nature: "network",
    site: "site-est",
    vlan: "121 supervision",
  },
];
const prefixColumns: GridColumn[] = [
  { id: "key", label: "Préfixe", width: "fill", mono: true, sortable: true },
  { id: "name", label: "Nom", width: "md", sortable: true, priority: 2 },
  { id: "nature", label: "Nature", width: "md", filterable: true, priority: 3 },
  { id: "site", label: "Site", width: "md", mono: true, filterable: true, priority: 3 },
  { id: "vlan", label: "VLAN", width: "md", mono: true, sortable: true, filterable: true },
];
const prefixSort = ref<GridSort | undefined>({ column: "key", direction: "ascending" });
const prefixActions: ToolbarAction[] = [
  { id: "create", label: "Créer", icon: "plus", variant: "primary", shortcut: ["c"], priority: 3 },
  { id: "export", label: "Exporter", priority: 1 },
];
const NETWORK = "192.0.2.192/26";
const vid = ref("105");
const name = ref("impression");
const status = ref<VlanStatus>("active");
const createActions: SplitAction[] = [
  { id: "draft", label: "Ajouter au brouillon", icon: "draft" },
  { id: "apply", label: "Appliquer", icon: "circle-check" },
  { id: "review", label: "Soumettre en revue", icon: "send" },
];
const createAction = ref("draft");
</script>

<template>
  <!-- The groups of VLAN, their anchor, range and occupation. -->
  <Cadre
    v-if="props.vue === 'groupes'"
    title="Groupes de VLAN"
    active="vlans"
    space="referential"
    :crumbs="[{ label: 'Référentiel', to: '/' }, { label: 'VLAN' }]"
  >
    <Split side="end" side-size="sm" gap="3">
      <Panneau title="Groupes de VLAN" :detail="String(groups.length)" main flush>
        <template #toolbar>
          <Barre label="Actions des groupes" :actions="groupsActions" />
        </template>
        <DataGrid
          v-model:sort="groupSort"
          label="Groupes de VLAN"
          :columns="groupColumns"
          :rows="groups"
          row-key="key"
          stack
        >
          <template #cell="{ row, column, value }">
            <RouterLink v-if="column.id === 'key'" :to="view('groupe')">{{ row.key }}</RouterLink>
            <Cluster v-else-if="column.id === 'anchor'" gap="1" :wrap="false">
              <Icon
                :name="ANCHOR[row.anchorKind].icon"
                meaningful
                :label="ANCHOR[row.anchorKind].label"
              /><code>{{ row.anchor }}</code>
            </Cluster>
            <template v-else-if="column.id === 'occupation'">{{
              percent(row.occupation)
            }}</template>
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
        <template #footer>
          <span class="k-num">4 groupes · 153 VLAN</span>
          <span>Au présent</span>
        </template>
      </Panneau>
      <template #side>
        <Panneau title="Occupation" detail="VID utilisés ou réservés">
          <BarList
            label="Occupation des groupes de VLAN"
            :items="occupation"
            :max="100"
            mono
            :format="percent"
            point-label="Groupe"
            value-label="Occupation"
          />
        </Panneau>
      </template>
    </Split>
  </Cadre>

  <!-- One group: its occupation in slices, the next free VID, its VLAN. -->
  <Cadre
    v-else-if="props.vue === 'groupe'"
    title="Groupe de VLAN campus-ouest"
    active="vlans"
    space="referential"
    :crumbs="[
      { label: 'Référentiel', to: '/' },
      { label: 'VLAN', to: '/vlans' },
      { label: 'campus-ouest', mono: true },
    ]"
  >
    <Split side="end" side-size="md" gap="3">
      <Panneau title="campus-ouest" detail="groupe de VLAN · VID 100–199" main flush>
        <template #toolbar>
          <Barre label="Actions du groupe" :actions="groupActions">
            <span class="k-num">87 VLAN · prochain VID libre 105</span>
          </Barre>
        </template>
        <DataGrid
          v-model:sort="vlanSort"
          label="VLAN du groupe campus-ouest"
          :columns="vlanColumns"
          :rows="vlans"
          row-key="vid"
          :row-state="vlanRow"
          stack
        >
          <template #cell="{ row, column, value }">
            <RouterLink v-if="column.id === 'name' && row.vid === '121'" :to="view('vlan')">{{
              row.name
            }}</RouterLink>
            <StatusBadge
              v-else-if="column.id === 'status'"
              :state="STATUS[row.status].state"
              :label="STATUS[row.status].label"
            />
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
        <template #footer>
          <span class="k-num">9 VLAN sur 87 · 1 en brouillon</span>
          <span>Au présent</span>
        </template>
      </Panneau>
      <template #side>
        <Stack gap="3">
          <Panneau title="Attributs">
            <Attributs
              :items="[
                { key: 'anchor', label: 'Ancre' },
                { key: 'range', label: 'Plage de VID', value: '100–199', mono: true },
                { key: 'owner', label: 'Propriétaire', value: 'Atelier Nord' },
              ]"
            >
              <template #anchor>
                <Cluster gap="1" :wrap="false">
                  <Icon name="globe" meaningful label="Site" /><code>site-ouest</code>
                </Cluster>
              </template>
            </Attributs>
          </Panneau>
          <Panneau title="Occupation" detail="87 %">
            <BarList
              label="Occupation du groupe campus-ouest"
              :items="counts"
              :max="100"
              point-label="État"
              value-label="VID"
            />
            <Attributs
              :items="[
                { key: 'next', label: 'Prochain VID libre', value: '105', mono: true },
                { key: 'from', label: 'À partir de 120', value: '122', mono: true },
              ]"
            />
            <TextField
              v-model="fromVid"
              label="Prochain libre à partir du VID"
              mono
              size="sm"
              hint="Dans la plage 100–199."
            />
            <DataGrid
              label="Tranches de VID du groupe campus-ouest"
              :columns="sliceColumns"
              :rows="slices"
              row-key="key"
              stack
            >
              <template #cell="{ row, column, value }">
                <StatusBadge
                  v-if="column.id === 'kind'"
                  :state="SLICE[row.kind].state"
                  :label="`${row.count} ${SLICE[row.kind].label.toLowerCase()}`"
                />
                <template v-else>{{ value }}</template>
              </template>
            </DataGrid>
          </Panneau>
        </Stack>
      </template>
    </Split>
  </Cadre>

  <!-- The card of a VLAN: the networks that carry it, under the rights of the caller. -->
  <Cadre
    v-else-if="props.vue === 'vlan'"
    title="VLAN 121 supervision"
    active="vlans"
    space="referential"
    :crumbs="[
      { label: 'Référentiel', to: '/' },
      { label: 'VLAN', to: '/vlans' },
      { label: 'campus-ouest', to: '/vlan-groups/campus-ouest', mono: true },
      { label: '121 supervision', mono: true },
    ]"
  >
    <Panneau title="VLAN 121 supervision" detail="VLAN · actif · groupe campus-ouest" main>
      <template #actions>
        <Button icon="pencil" :shortcut="['e']">Modifier</Button>
        <Menu label="Actions" icon="ellipsis" align="end" :items="vlanMenu" />
      </template>
      <Split side="end" side-size="sm" gap="3">
        <Tabs v-model="vlanTab" :tabs="vlanTabs" label="Sections de la fiche">
          <template #networks>
            <Stack gap="3">
              <DataGrid
                v-model:sort="networkSort"
                label="Réseaux qui portent le VLAN 121"
                :columns="networkColumns"
                :rows="networks"
                row-key="key"
                stack
              >
                <template #cell="{ row, column, value }">
                  <a v-if="column.id === 'key'" :href="`/prefixes/${row.key}`">{{ row.key }}</a>
                  <StatusBadge
                    v-else-if="column.id === 'coherence' && row.coherence === 'ok'"
                    state="ok"
                    label="Cohérent"
                  />
                  <Cluster v-else-if="column.id === 'coherence'" gap="1">
                    <StatusBadge state="warn" label="Site hors de l'ancre" />
                    <CodeBadge code="core.vlan.site_mismatch" />
                  </Cluster>
                  <template v-else>{{ value }}</template>
                </template>
              </DataGrid>
              <p class="k-ecran-muted">
                Seuls les réseaux que vous pouvez lire sont listés et comptés.
              </p>
              <InlineAlert tone="warn" title="Un réseau hors du site du groupe">
                Le groupe campus-ouest est ancré sur le site <code>site-ouest</code> ;
                <code>198.51.100.64/26</code> est sur <code>site-est</code>.
                <CodeBadge code="core.vlan.site_mismatch" />
              </InlineAlert>
            </Stack>
          </template>
        </Tabs>
        <template #side>
          <Panneau title="Attributs">
            <Attributs
              :items="[
                { key: 'vid', label: 'VID', value: '121', mono: true },
                { key: 'name', label: 'Nom', value: 'supervision', mono: true },
                { key: 'status', label: 'Statut' },
                { key: 'group', label: 'Groupe' },
                { key: 'domain', label: 'Domaine', value: DOMAIN.label },
                { key: 'id', label: 'Identifiant' },
              ]"
            >
              <template #status><StatusBadge state="ok" label="Actif" /></template>
              <template #group>
                <RouterLink :to="view('groupe')" class="k-mono">campus-ouest</RouterLink>
              </template>
              <template #id><CodeBadge code="01JD4M2V6N8Q0R2T4W6Y8A0C2E" /></template>
            </Attributs>
          </Panneau>
        </template>
      </Split>
    </Panneau>
  </Cadre>

  <!-- Creating a VLAN for a network, from the list of prefixes and its VLAN column. -->
  <Cadre
    v-else
    title="Préfixes"
    active="prefixes"
    space="referential"
    panel="detail"
    :crumbs="[
      { label: 'Référentiel', to: '/' },
      { label: DOMAIN.label, to: `/domains/${DOMAIN.key}` },
      { label: 'Préfixes' },
    ]"
  >
    <Panneau title="Préfixes" :detail="DOMAIN.label" main flush>
      <template #toolbar>
        <Barre label="Actions des préfixes" :actions="prefixActions" align="start" />
      </template>
      <DataGrid
        v-model:sort="prefixSort"
        label="Préfixes du domaine, avec leur VLAN"
        :columns="prefixColumns"
        :rows="prefixes"
        row-key="key"
        selectable="single"
        :selected="[NETWORK]"
      >
        <template #cell="{ row, column, value }">
          <KeyLabel v-if="column.id === 'nature'" :value="row.nature" :label="NATURE[row.nature]" />
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num">7 préfixes sur 1 208</span>
        <span>Colonne VLAN : propriété core.vlan des réseaux</span>
      </template>
    </Panneau>
    <template #detail>
      <Stack gap="3">
        <h2 class="k-ecran-subtitle">Créer un VLAN pour <code>192.0.2.192/26</code></h2>
        <AVenir component="Select" task="18">
          <TextField
            label="Groupe"
            model-value="campus-ouest"
            mono
            readonly
            hint="Groupes ancrés sur le site du réseau (site-ouest), son entité ou son domaine."
          />
        </AVenir>
        <TextField
          v-model="vid"
          label="VID"
          mono
          required
          hint="Prochain VID libre du groupe campus-ouest (100–199)."
        />
        <TextField v-model="name" label="Nom" mono required />
        <Cluster gap="1" role="group" aria-labelledby="k-e12-status">
          <span id="k-e12-status" class="k-e12-label">Statut</span>
          <Button
            v-for="s in ['active', 'reserved'] as const"
            :key="s"
            size="sm"
            :pressed="status === s"
            @click="status = s"
            >{{ STATUS[s].label }}</Button
          >
        </Cluster>
        <Attributs
          :items="[
            { key: 'network', label: 'Réseau', value: NETWORK, mono: true },
            { key: 'property', label: 'Propriété posée' },
          ]"
        >
          <template #property><KeyLabel value="core.vlan" label="VLAN" show-key /></template>
        </Attributs>
        <InlineAlert tone="info" title="Deux opérations au brouillon">
          Créer le VLAN {{ vid }} {{ name }} dans campus-ouest, puis le poser sur
          <code>{{ NETWORK }}</code
          >.
        </InlineAlert>
      </Stack>
    </template>
    <template #detail-footer>
      <Cluster justify="end">
        <SplitButton
          v-model:action="createAction"
          :actions="createActions"
          :shortcut="['Control', 'Enter']"
        />
      </Cluster>
    </template>
  </Cadre>
</template>

<style>
@layer components {
  /* The term before a group of toggles, as the label of a field. */
  .k-e12-label {
    flex-basis: 100%;
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
  }
}
</style>
