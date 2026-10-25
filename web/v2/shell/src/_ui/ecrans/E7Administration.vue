<script setup lang="ts">
// E7 — Administration (design « Maquettes »; 021, « Catalogue des écrans », Administration;
// spec 018 : permissions servies `module.action` avec leur libellé; 024/M8, M13, M15):
// roles, and the chosen role in the right panel (its permissions one per line, label then
// served name; its bindings, which scroll in the panel; each row ends with its actions, in
// a column of fixed place); the same role edited (024-11): a removal and an addition of a
// permission and a removal of a binding wait in the draft, marked on their rows, the draft
// counter at 3; bindings with their scope and period, a new one in the same right panel;
// a new secret of a service account shown once.
import {
  Button,
  Cluster,
  CodeBadge,
  DataGrid,
  Dialog,
  Icon,
  IconButton,
  InlineAlert,
  KeyLabel,
  Stack,
  StatusBadge,
  TextField,
  type GridColumn,
  type GridSort,
  type RowState,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
import AVenir from "./AVenir.vue";
import Barre from "./EcranBarre.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";

const props = defineProps<{ vue: string }>();
/** The chosen role in the panel: read (`roles`) or with operations in the draft (`edition`). */
const roleShown = props.vue === "roles" || props.vue === "edition";
const editing = props.vue === "edition";

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
/** The edited role is marked as in the draft in the table. */
const roleState = (row: Role): RowState | undefined =>
  editing && row.key === "lecteur-reseau" ? "draft" : undefined;
/** A row waiting in the draft: removed (still shown, struck) or added (at the end). */
type Pending = "removed" | "added" | undefined;
const PENDING = {
  removed: { icon: "minus", label: "Retrait en brouillon" },
  added: { icon: "plus", label: "Ajout en brouillon" },
} as const;

interface Grant {
  code: string;
  label: string;
  pending?: Pending;
}
const permissions: Grant[] = [
  { code: "core.prefix.read", label: "Lire les préfixes" },
  { code: "core.address.read", label: "Lire les adresses" },
  { code: "core.range.read", label: "Lire les plages" },
  { code: "core.site.read", label: "Lire les sites", pending: editing ? "removed" : undefined },
  { code: "vlan.read", label: "Lire les VLAN" },
  { code: "vlan.group.read", label: "Lire les groupes de VLAN" },
  { code: "machines.read", label: "Lire les machines" },
  { code: "machines.interface.read", label: "Lire les interfaces" },
  ...(editing
    ? [{ code: "core.domain.read", label: "Lire les domaines", pending: "added" as const }]
    : []),
];

interface RoleBinding {
  subject: string;
  scope: string;
  pending?: Pending;
  /** Why this binding cannot be changed by the user (024/M14). */
  reason?: string;
}
// A global scope is wider than the bindings of the user: refused (core.binding.escalation).
const WIDER = "Portée plus large que vos liaisons (core.binding.escalation).";
const roleBindings: RoleBinding[] = [
  { subject: "Équipe supervision", scope: "Site : site-ouest" },
  { subject: "Équipe réseau", scope: "Globale", reason: WIDER },
  { subject: "Lou Bernard", scope: "Entité : Atelier Nord" },
  { subject: "Sam Leroy", scope: "Site : site-est", pending: editing ? "removed" : undefined },
  { subject: "Équipe sauvegarde", scope: "Domaine : dc-nord" },
  { subject: "Alex Morel", scope: "Site : site-ouest" },
  { subject: "Équipe astreinte", scope: "Globale", reason: WIDER },
  { subject: "Noa Petit", scope: "Entité : Atelier Sud" },
  { subject: "export-dns", scope: "Domaine : dc-nord" },
];
const roleActions: ToolbarAction[] = [
  { id: "edit", label: "Modifier", icon: "pencil", shortcut: ["e"], priority: 2 },
  { id: "copy", label: "Dupliquer", icon: "copy", priority: 1 },
  { id: "delete", label: "Supprimer…", icon: "trash", reason: "Le rôle a 9 liaisons." },
];
/** The operations of the draft in the view `edition` (its tab « Brouillon 3 »). */
const roleOperations = [
  { what: "Retirer la permission", target: "core.site.read" },
  { what: "Ajouter la permission", target: "core.domain.read" },
  { what: "Retirer la liaison", target: "Sam Leroy · Site : site-est" },
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
    :draft="editing ? roleOperations.length : 0"
    :panel="props.vue === 'secret' ? undefined : 'detail'"
    :crumbs="[
      { label: 'Administration', to: '/admin' },
      props.vue === 'liaisons'
        ? { label: 'Liaisons' }
        : props.vue === 'secret'
          ? { label: 'Comptes de service' }
          : { label: 'Rôles', to: '/admin/roles' },
      ...(roleShown ? [{ label: 'lecteur-reseau', mono: true }] : []),
    ]"
  >
    <template v-if="roleShown">
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
          :row-state="roleState"
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

    <template #detail>
      <Stack v-if="roleShown" gap="4">
        <Stack gap="2">
          <h2 class="k-ecran-subtitle">
            Lecteur réseau <span class="k-mono k-ecran-muted">lecteur-reseau</span>
          </h2>
          <Barre label="Actions du rôle" :actions="roleActions" align="start" />
        </Stack>
        <section aria-labelledby="k-ecran-permissions">
          <Stack gap="2">
            <h3 id="k-ecran-permissions" class="k-ecran-subtitle">
              Permissions · 8
              <span v-if="editing" class="k-ecran-muted k-ecran-poids">(brouillon : −1, +1)</span>
            </h3>
            <ul class="k-ecran-list k-ecran-lignes k-ecran-lignes--valeur">
              <li v-for="grant in permissions" :key="grant.code" :data-pending="grant.pending">
                <span class="k-ecran-ligne__texte">
                  <Icon
                    v-if="grant.pending"
                    :name="PENDING[grant.pending].icon"
                    meaningful
                    :label="PENDING[grant.pending].label"
                    class="k-ecran-ligne__marque"
                  />
                  <span class="k-ecran-ligne__libelle">{{ grant.label }}</span>
                </span>
                <CodeBadge :code="grant.code" />
                <span class="k-ecran-ligne__actions">
                  <IconButton
                    v-if="grant.pending === 'removed'"
                    icon="undo-2"
                    size="sm"
                    :label="`Annuler le retrait de ${grant.code}`"
                  />
                  <IconButton
                    v-else-if="grant.pending === 'added'"
                    icon="x"
                    size="sm"
                    :label="`Retirer l'ajout de ${grant.code} du brouillon`"
                  />
                  <IconButton
                    v-else
                    icon="trash"
                    size="sm"
                    :label="`Retirer la permission ${grant.code}`"
                  />
                </span>
              </li>
            </ul>
            <span>
              <Button size="sm" variant="ghost" icon="plus">Ajouter une permission</Button>
            </span>
          </Stack>
        </section>
        <section aria-labelledby="k-ecran-liaisons">
          <Stack gap="2">
            <h3 id="k-ecran-liaisons" class="k-ecran-subtitle">
              Liaisons · 9
              <span v-if="editing" class="k-ecran-muted k-ecran-poids">(brouillon : −1)</span>
            </h3>
            <ul class="k-ecran-list k-ecran-lignes">
              <li
                v-for="binding in roleBindings"
                :key="binding.subject"
                :data-pending="binding.pending"
              >
                <span class="k-ecran-ligne__texte">
                  <Icon
                    v-if="binding.pending"
                    :name="PENDING[binding.pending].icon"
                    meaningful
                    :label="PENDING[binding.pending].label"
                    class="k-ecran-ligne__marque"
                  />
                  <span class="k-ecran-ligne__libelle">{{ binding.subject }}</span>
                </span>
                <span class="k-ecran-muted">{{ binding.scope }}</span>
                <span class="k-ecran-ligne__actions">
                  <IconButton
                    v-if="binding.pending === 'removed'"
                    icon="undo-2"
                    size="sm"
                    :label="`Annuler le retrait de la liaison de ${binding.subject}`"
                  />
                  <template v-else>
                    <IconButton
                      icon="pencil"
                      size="sm"
                      :label="`Modifier la liaison de ${binding.subject}`"
                      :reason="binding.reason"
                    />
                    <IconButton
                      icon="trash"
                      size="sm"
                      :label="`Retirer la liaison de ${binding.subject}`"
                      :reason="binding.reason"
                    />
                  </template>
                </span>
              </li>
            </ul>
            <span>
              <Button size="sm" variant="ghost" icon="plus">Ajouter une liaison</Button>
            </span>
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
    <template v-if="props.vue === 'liaisons'" #detail-footer>
      <Cluster gap="2" justify="end">
        <Button variant="ghost">Annuler</Button>
        <Button variant="primary" icon="check">Créer la liaison</Button>
      </Cluster>
    </template>
    <template v-if="editing" #draft>
      <Stack as="ol" gap="2" class="k-ecran-list" aria-label="Opérations du brouillon">
        <li v-for="op in roleOperations" :key="op.target">
          <Cluster justify="between" :wrap="false" gap="2">
            <Cluster gap="2" :wrap="false" class="k-ecran-op">
              <Icon name="draft" />
              <code>lecteur-reseau</code>
              <span class="k-ecran-op__change"
                >{{ op.what }} <span class="k-ecran-muted">({{ op.target }})</span></span
              >
            </Cluster>
            <IconButton
              icon="x"
              size="sm"
              :label="`Retirer l'opération « ${op.what} » ${op.target}`"
            />
          </Cluster>
        </li>
      </Stack>
    </template>
  </Cadre>
</template>

<style>
@layer components {
  /* One entry per line, in aligned columns: a subject and its scope; a label and its
   * served name (024-11: the description on the left, the value on the right); then the
   * actions of the row, in a last column at the end, at the same place on every row. */
  .k-ecran-lignes {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr) max-content;
    gap: var(--k-space-1) var(--k-space-3);
    align-items: center;
  }

  .k-ecran-lignes > li {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
    min-block-size: var(--k-row-height);
    padding-inline-start: var(--k-space-2);
    border-radius: var(--k-radius-sm);
  }

  .k-ecran-lignes--valeur {
    grid-template-columns: minmax(0, 1fr) max-content max-content;
  }

  /* A row waiting in the draft: the draft look of the grids (accent background and bar),
   * its sign before the text; a removal stays in place, struck. */
  .k-ecran-lignes > li[data-pending] {
    background-color: var(--k-accent-soft);
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-ecran-lignes > li[data-pending="removed"] .k-ecran-ligne__libelle {
    color: var(--k-text-muted);
    text-decoration: line-through;
  }

  .k-ecran-lignes > li[data-pending="removed"] .k-ecran-ligne__marque {
    color: var(--k-diff-removed);
  }

  .k-ecran-lignes > li[data-pending="added"] .k-ecran-ligne__marque {
    color: var(--k-diff-added);
  }

  .k-ecran-ligne__texte {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
  }

  .k-ecran-ligne__marque {
    flex: none;
  }

  /* The actions of a row, at the end of their column whatever their number. */
  .k-ecran-ligne__actions {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    justify-self: end;
  }

  .k-ecran-poids {
    font-weight: var(--k-weight-regular);
  }
}
</style>
