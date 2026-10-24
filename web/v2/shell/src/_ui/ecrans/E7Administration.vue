<script setup lang="ts">
// E7 — Administration (design « Maquettes »; 021, « Catalogue des écrans », Administration;
// spec 018 : permissions servies `module.action` avec leur libellé): roles and the
// permissions of the chosen role, bindings with their scope and period, a new secret of a
// service account shown once.
import {
  Button,
  Cluster,
  CodeBadge,
  Dialog,
  DataGrid,
  Grid,
  InlineAlert,
  KeyLabel,
  Split,
  Stack,
  StatusBadge,
  TextField,
  type GridColumn,
} from "@kartenn/ui";
import AVenir from "./AVenir.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";

const props = defineProps<{ vue: string }>();

interface Role extends Record<string, unknown> {
  key: string;
  label: string | null;
  builtin: boolean;
  permissions: number;
  bindings: number;
}
const roles: Role[] = [
  { key: "reader", label: "Lecteur", builtin: true, permissions: 14, bindings: 42 },
  { key: "editor", label: "Éditeur", builtin: true, permissions: 31, bindings: 12 },
  { key: "admin", label: "Administrateur", builtin: true, permissions: 61, bindings: 2 },
  { key: "lecteur-reseau", label: "Lecteur réseau", builtin: false, permissions: 6, bindings: 5 },
  { key: "instructeur-flux", label: "Instructeur", builtin: false, permissions: 3, bindings: 4 },
  { key: "import-ipam", label: null, builtin: false, permissions: 8, bindings: 1 },
];
const roleColumns: GridColumn[] = [
  { id: "key", label: "Rôle", width: "fill", sortable: true },
  { id: "builtin", label: "Intégré", width: "sm" },
  { id: "permissions", label: "Permissions", width: "sm", align: "end" },
  { id: "bindings", label: "Liaisons", width: "sm", align: "end" },
];
const permissions = [
  { code: "core.prefix.read", label: "Lire les préfixes" },
  { code: "core.address.read", label: "Lire les adresses" },
  { code: "vlan.read", label: "Lire les VLAN" },
  { code: "vlan.group.read", label: "Lire les groupes de VLAN" },
  { code: "machines.read", label: "Lire les machines" },
  { code: "machines.interface.read", label: "Lire les interfaces" },
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
  { id: "subject", label: "Sujet", width: "fill", sortable: true },
  { id: "role", label: "Rôle", width: "md" },
  { id: "scope", label: "Portée", width: "lg" },
  { id: "from", label: "Du", width: "md", mono: true },
  { id: "to", label: "Au", width: "md", mono: true },
];

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
  { id: "name", label: "Compte", width: "fill", mono: true },
  { id: "state", label: "État", width: "md" },
  { id: "secrets", label: "Secrets", width: "sm", align: "end" },
  { id: "lastUse", label: "Dernier usage", width: "lg", mono: true },
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
    <Split v-if="props.vue === 'roles'" side="end" side-size="lg" gap="3">
      <Panneau title="Rôles" detail="6" main flush>
        <template #actions>
          <Button variant="primary" icon="plus" :shortcut="['c']">Créer</Button>
        </template>
        <DataGrid
          label="Rôles"
          :columns="roleColumns"
          :rows="roles"
          row-key="key"
          selectable
          :selected="['lecteur-reseau']"
          :sort="{ column: 'key', direction: 'ascending' }"
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
      <template #side>
        <Panneau title="Lecteur réseau" detail="lecteur-reseau" mono>
          <template #actions>
            <Button icon="pencil" :shortcut="['e']">Modifier</Button>
          </template>
          <h3 class="k-ecran-subtitle">Permissions · 6</h3>
          <Stack as="ul" gap="2" class="k-ecran-list">
            <li v-for="permission in permissions" :key="permission.code">
              <Stack gap="1">
                <span>{{ permission.label }}</span>
                <CodeBadge :code="permission.code" />
              </Stack>
            </li>
          </Stack>
          <Button variant="ghost" icon="plus">Ajouter une permission</Button>
        </Panneau>
      </template>
    </Split>

    <template v-else-if="props.vue === 'liaisons'">
      <Panneau title="Liaisons" detail="4" main flush>
        <template #actions>
          <Button variant="primary" icon="plus" :shortcut="['c']">Créer</Button>
        </template>
        <DataGrid
          label="Liaisons"
          :columns="bindingColumns"
          :rows="bindings"
          row-key="id"
          :sort="{ column: 'subject', direction: 'ascending' }"
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
      <Panneau title="Nouvelle liaison">
        <Grid :columns="4" gap="3" align="start">
          <TextField label="Sujet" model-value="Équipe supervision" />
          <AVenir component="Select" task="18">
            <TextField label="Rôle" readonly mono model-value="admin" />
          </AVenir>
          <AVenir component="Select" task="18">
            <TextField label="Portée" readonly model-value="Globale" />
          </AVenir>
          <TextField label="Du" mono model-value="2026-01-15" />
        </Grid>
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
        <template #footer>
          <span class="k-ecran-muted">La liaison prend effet à la date « Du ».</span>
          <Cluster gap="2">
            <Button variant="ghost">Annuler</Button>
            <Button variant="primary" icon="check">Créer la liaison</Button>
          </Cluster>
        </template>
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
  </Cadre>
</template>
