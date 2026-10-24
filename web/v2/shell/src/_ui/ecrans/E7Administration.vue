<script setup lang="ts">
// E7 — Administration (design « Maquettes »; 021, « Catalogue des écrans », Administration;
// spec 018 : permissions servies `module.action` avec leur libellé; 024/M8, M13, M15):
// roles, and the chosen role in the right panel (its permissions one per line, served name
// and label; its bindings, which scroll in the panel); bindings with their scope and
// period, a new one in the same right panel; a new secret of a service account shown once.
import {
  Button,
  Cluster,
  CodeBadge,
  DataGrid,
  Dialog,
  InlineAlert,
  KeyLabel,
  SidePanel,
  Stack,
  StatusBadge,
  TextField,
  type GridColumn,
  type GridSort,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
import AVenir from "./AVenir.vue";
import Barre from "./EcranBarre.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";

const props = defineProps<{ vue: string }>();
const panelTabs = [
  { value: "detail", label: "Détail" },
  { value: "draft", label: "Brouillon", count: 0 },
];

interface Role extends Record<string, unknown> {
  key: string;
  label: string | null;
  builtin: boolean;
  permissions: number;
  bindings: number;
  about: string;
}
const role = (
  key: string,
  label: string | null,
  builtin: boolean,
  permissions: number,
  bindings: number,
  about: string,
): Role => ({ key, label, builtin, permissions, bindings, about });
const roles: Role[] = [
  role("reader", "Lecteur", true, 14, 42, "Lecture de tout le référentiel"),
  role("editor", "Éditeur", true, 31, 12, "Édition du référentiel, sans l'administration"),
  role("admin", "Administrateur", true, 61, 2, "Tous les droits"),
  role("lecteur-reseau", "Lecteur réseau", false, 8, 9, "Lecture du réseau et des machines"),
  role("instructeur-flux", "Instructeur", false, 3, 4, "Instruction des demandes de flux"),
  role("import-ipam", null, false, 8, 1, "Compte d'import"),
];
// The key first and of fixed width, the description last: the right panel covers the end
// of the table (024/M8), not its key columns.
const roleColumns: GridColumn[] = [
  { id: "key", label: "Rôle", width: "lg", sortable: true, filterable: true },
  { id: "builtin", label: "Intégré", width: "sm", sortable: true, filterable: true },
  { id: "permissions", label: "Permissions", width: "sm", align: "end", sortable: true },
  { id: "bindings", label: "Liaisons", width: "sm", align: "end", sortable: true },
  { id: "about", label: "Description", width: "fill", filterable: true },
];
const createActions: ToolbarAction[] = [
  { id: "create", label: "Créer", icon: "plus", variant: "primary", shortcut: ["c"] },
];
const roleSort = ref<GridSort>({ column: "key", direction: "ascending" });
const permissions = [
  { code: "core.prefix.read", label: "Lire les préfixes" },
  { code: "core.address.read", label: "Lire les adresses" },
  { code: "core.range.read", label: "Lire les plages" },
  { code: "core.site.read", label: "Lire les sites" },
  { code: "vlan.read", label: "Lire les VLAN" },
  { code: "vlan.group.read", label: "Lire les groupes de VLAN" },
  { code: "machines.read", label: "Lire les machines" },
  { code: "machines.interface.read", label: "Lire les interfaces" },
];
const roleBindings = [
  { subject: "Équipe supervision", scope: "Site : site-ouest" },
  { subject: "Équipe réseau", scope: "Globale" },
  { subject: "Lou Bernard", scope: "Entité : Atelier Nord" },
  { subject: "Sam Leroy", scope: "Site : site-est" },
  { subject: "Équipe sauvegarde", scope: "Domaine : dc-nord" },
  { subject: "Alex Morel", scope: "Site : site-ouest" },
  { subject: "Équipe astreinte", scope: "Globale" },
  { subject: "Noa Petit", scope: "Entité : Atelier Sud" },
  { subject: "export-dns", scope: "Domaine : dc-nord" },
];
const roleActions: ToolbarAction[] = [
  { id: "edit", label: "Modifier", icon: "pencil", shortcut: ["e"], priority: 3 },
  { id: "add", label: "Ajouter une permission", icon: "plus", priority: 2 },
  { id: "copy", label: "Dupliquer", icon: "copy", priority: 1 },
  { id: "delete", label: "Supprimer…", icon: "trash", reason: "Le rôle a 9 liaisons." },
];

interface Binding extends Record<string, unknown> {
  id: string;
  subject: string;
  kind: string;
  role: string;
  roleLabel: string | null;
  scope: string;
  from: string;
  to: string;
}
const bindings: Binding[] = [
  {
    id: "b1",
    subject: "Camille Martin",
    kind: "personne",
    role: "admin",
    roleLabel: "Administrateur",
    scope: "Globale",
    from: "2025-03-17",
    to: "—",
  },
  {
    id: "b2",
    subject: "Lou Bernard",
    kind: "personne",
    role: "editor",
    roleLabel: "Éditeur",
    scope: "Entité : Atelier Nord",
    from: "2025-06-01",
    to: "—",
  },
  {
    id: "b3",
    subject: "Équipe supervision",
    kind: "groupe",
    role: "lecteur-reseau",
    roleLabel: "Lecteur réseau",
    scope: "Site : site-ouest",
    from: "2025-09-12",
    to: "2026-03-31",
  },
  {
    id: "b4",
    subject: "sync-ipam",
    kind: "compte de service",
    role: "import-ipam",
    roleLabel: null,
    scope: "Domaine : dc-nord",
    from: "2025-03-17",
    to: "—",
  },
];
const bindingColumns: GridColumn[] = [
  { id: "subject", label: "Sujet", width: "lg", sortable: true, filterable: true },
  { id: "role", label: "Rôle", width: "md", sortable: true, filterable: true },
  { id: "scope", label: "Portée", width: "md", filterable: true },
  { id: "from", label: "Du", width: "sm", mono: true, sortable: true },
  { id: "to", label: "Au", width: "fill", mono: true, sortable: true },
];
const bindingSort = ref<GridSort>({ column: "subject", direction: "ascending" });

interface Account extends Record<string, unknown> {
  name: string;
  state: "active" | "disabled";
  secrets: number;
  lastUse: string;
}
const accounts: Account[] = [
  { name: "sync-ipam", state: "active", secrets: 2, lastUse: "2026-01-15 09:58" },
  { name: "export-dns", state: "active", secrets: 1, lastUse: "2026-01-15 06:00" },
  { name: "audit-annuel", state: "disabled", secrets: 0, lastUse: "2025-12-31 23:10" },
];
const accountColumns: GridColumn[] = [
  { id: "name", label: "Compte", width: "fill", mono: true, sortable: true },
  { id: "state", label: "État", width: "md", filterable: true },
  { id: "secrets", label: "Secrets", width: "sm", align: "end", sortable: true },
  { id: "lastUse", label: "Dernier usage", width: "lg", mono: true, sortable: true },
];
const SECRET = "ksk_live_7Q2mX9vR4tB8nL1cW6pZ3hJ5";
</script>

<template>
  <Cadre
    :title="
      props.vue === 'liaisons'
        ? 'Liaisons'
        : props.vue === 'secret'
          ? 'Comptes de service'
          : 'Rôles'
    "
    :active="props.vue === 'liaisons' ? 'bindings' : props.vue === 'secret' ? 'accounts' : 'roles'"
    space="admin"
    :draft="0"
    :crumbs="[
      { label: 'Administration', to: '/admin' },
      props.vue === 'liaisons'
        ? { label: 'Liaisons' }
        : props.vue === 'secret'
          ? { label: 'Comptes de service' }
          : { label: 'Rôles', to: '/admin/roles' },
      ...(props.vue === 'roles' ? [{ label: 'lecteur-reseau', mono: true }] : []),
    ]"
  >
    <template v-if="props.vue === 'roles'">
      <Panneau title="Rôles" detail="6" main flush>
        <template #toolbar>
          <Barre label="Actions des rôles" :actions="createActions" align="start" />
        </template>
        <DataGrid
          v-model:sort="roleSort"
          label="Rôles"
          :columns="roleColumns"
          :rows="roles"
          row-key="key"
          selectable
          :selected="['lecteur-reseau']"
        >
          <template #cell="{ row, column, value }">
            <KeyLabel v-if="column.id === 'key'" :value="row.key" :label="row.label" show-key />
            <template v-else-if="column.id === 'builtin'">{{
              row.builtin ? "oui" : "non"
            }}</template>
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
      </Panneau>
    </template>

    <template v-else-if="props.vue === 'liaisons'">
      <Panneau title="Liaisons" detail="4" main flush>
        <template #toolbar>
          <Barre label="Actions des liaisons" :actions="createActions" align="start" />
        </template>
        <DataGrid
          v-model:sort="bindingSort"
          label="Liaisons"
          :columns="bindingColumns"
          :rows="bindings"
          row-key="id"
        >
          <template #cell="{ row, column, value }">
            <template v-if="column.id === 'subject'"
              >{{ row.subject }} <span class="k-ecran-muted">· {{ row.kind }}</span></template
            >
            <KeyLabel v-else-if="column.id === 'role'" :value="row.role" :label="row.roleLabel" />
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
      </Panneau>
    </template>

    <template v-else>
      <Panneau title="Comptes de service" detail="3" main flush>
        <template #actions>
          <Button variant="primary" icon="plus" :shortcut="['c']">Créer</Button>
        </template>
        <DataGrid
          label="Comptes de service"
          :columns="accountColumns"
          :rows="accounts"
          row-key="name"
        >
          <template #cell="{ row, column, value }">
            <StatusBadge
              v-if="column.id === 'state'"
              :state="row.state === 'active' ? 'ok' : 'neutral'"
              :label="row.state === 'active' ? 'Actif' : 'Désactivé'"
            />
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
      </Panneau>
      <Dialog
        :open="true"
        title="Nouveau secret pour sync-ipam"
        description="Copiez-le maintenant : il ne sera plus jamais affiché."
      >
        <Stack gap="3">
          <InlineAlert tone="warn" title="Affiché une seule fois">
            kartenn n'en garde qu'une empreinte. Perdu, il se remplace par un nouveau secret.
          </InlineAlert>
          <CodeBadge :code="SECRET" />
          <p class="k-ecran-muted">Expire le <time datetime="2027-01-15">2027-01-15</time>.</p>
        </Stack>
        <template #actions>
          <Button variant="primary" icon="check">J'ai copié le secret</Button>
        </template>
      </Dialog>
    </template>

    <template v-if="props.vue !== 'secret'" #drawer>
      <SidePanel label="Panneau droit" :tabs="panelTabs" tab="detail">
        <template #detail>
          <Stack v-if="props.vue === 'roles'" gap="4">
            <Stack gap="2">
              <h2 class="k-ecran-subtitle">
                Lecteur réseau <span class="k-mono k-ecran-muted">lecteur-reseau</span>
              </h2>
              <Barre label="Actions du rôle" :actions="roleActions" align="start" />
            </Stack>
            <section aria-labelledby="k-ecran-permissions">
              <Stack gap="2">
                <h3 id="k-ecran-permissions" class="k-ecran-subtitle">Permissions · 8</h3>
                <ul class="k-ecran-list k-ecran-lignes">
                  <li v-for="permission in permissions" :key="permission.code">
                    <CodeBadge :code="permission.code" />
                    <span>{{ permission.label }}</span>
                  </li>
                </ul>
              </Stack>
            </section>
            <section aria-labelledby="k-ecran-liaisons">
              <Stack gap="2">
                <h3 id="k-ecran-liaisons" class="k-ecran-subtitle">Liaisons · 9</h3>
                <ul class="k-ecran-list k-ecran-lignes">
                  <li v-for="binding in roleBindings" :key="binding.subject">
                    <span>{{ binding.subject }}</span>
                    <span class="k-ecran-muted">{{ binding.scope }}</span>
                  </li>
                </ul>
              </Stack>
            </section>
          </Stack>
          <Stack v-else gap="3">
            <h2 class="k-ecran-subtitle">Nouvelle liaison</h2>
            <TextField label="Sujet" model-value="Équipe supervision" />
            <AVenir component="Select" task="18">
              <TextField label="Rôle" readonly mono model-value="admin" />
            </AVenir>
            <AVenir component="Select" task="18">
              <TextField label="Portée" readonly model-value="Globale" />
            </AVenir>
            <TextField
              label="Du"
              mono
              model-value="2026-01-15"
              hint="La liaison prend effet à cette date."
            />
            <InlineAlert
              tone="ko"
              title="Liaison refusée"
              :items="[
                {
                  message: 'Vous ne pouvez accorder un rôle plus large que les vôtres.',
                  code: 'core.binding.escalation',
                },
              ]"
            />
          </Stack>
        </template>
        <template #draft>
          <p class="k-ecran-muted">Le brouillon est vide.</p>
        </template>
        <template v-if="props.vue === 'liaisons'" #footer>
          <Cluster gap="2" justify="end">
            <Button variant="ghost">Annuler</Button>
            <Button variant="primary" icon="check">Créer la liaison</Button>
          </Cluster>
        </template>
      </SidePanel>
    </template>
  </Cadre>
</template>

<style>
@layer components {
  /* One entry per line, in two aligned columns: a served name and its label, a subject and
   * its scope. */
  .k-ecran-lignes {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: var(--k-space-1) var(--k-space-3);
    align-items: center;
  }

  .k-ecran-lignes > li {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
    min-block-size: var(--k-row-height);
  }
}
</style>
