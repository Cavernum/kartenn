<script setup lang="ts">
// E7 — Administration (design « Maquettes »; 021, « Catalogue des écrans », Administration;
// spec 018 : permissions servies `module.action` avec leur libellé; 024/M8, M13, M15; spec
// 025 : droits par groupes). Rights go to kartenn groups only (025, G1, G4): the chosen
// group in the right panel, its members (identities, groups of an IdP) then its roles on
// scopes, one per line, each row ended by its actions in a column of fixed place, « Ajouter
// » under each list; the same group edited, its operations waiting in the draft and marked
// on their rows (024-11); a group whose scope is wider than the user's, its actions refused
// by reason (025, G5); the builtin group `secours`; an identity, read only for its rights
// (groups and by which group of its token, effective rights, groups of its IdP at its last
// login, without date: not kept, 025, Q-025-1);
// a role and its permissions; a new secret of a service account shown once.
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
  Toolbar,
  type GridColumn,
  type GridSort,
  type RowState,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
import Attributs from "./EcranAttributs.vue";
import Barre from "./EcranBarre.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import {
  ACCOUNTS,
  ASTREINTE_BINDINGS,
  ASTREINTE_MEMBERS,
  GROUPS,
  IDENTITIES,
  IDENTITY_GROUPS,
  IDENTITY_RIGHTS,
  IDP_GROUP_TARGETS,
  MEMBER_KIND,
  ROLES,
  ROLE_GROUPS,
  ROLE_PERMISSIONS,
  SECOURS_BINDINGS,
  SECOURS_MEMBERS,
  SUPERVISION_BINDINGS,
  SUPERVISION_MEMBERS,
  SUPERVISION_NEW_BINDING,
  SUPERVISION_NEW_MEMBER,
  type GroupBinding,
  type GroupMember,
} from "./donnees.ts";

const props = defineProps<{ vue: string }>();
/** The entry of the admin space shown by the view. */
type Tab = "groups" | "identities" | "roles" | "accounts";
const TAB: Record<string, Tab> = {
  groupes: "groups",
  "groupe-edition": "groups",
  "groupe-refus": "groups",
  secours: "groups",
  identite: "identities",
  roles: "roles",
  secret: "accounts",
};
const tab: Tab = TAB[props.vue] ?? "groups";
const TITLE: Record<Tab, string> = {
  groups: "Groupes",
  identities: "Identités",
  roles: "Rôles",
  accounts: "Comptes de service",
};
const editing = props.vue === "groupe-edition";

const createActions: ToolbarAction[] = [
  { id: "create", label: "Créer", icon: "plus", variant: "primary", shortcut: ["c"] },
];

/** A row waiting in the draft: removed (still shown, struck) or added (at the end). */
type Pending = "removed" | "added" | undefined;
const PENDING = {
  removed: { icon: "minus", label: "Retrait en brouillon" },
  added: { icon: "plus", label: "Ajout en brouillon" },
} as const;

// Refusals (024/M14): the action stays, focusable, its reason in a tooltip. The user
// administers the entity « Atelier Nord »: a group bound on a wider scope cannot change
// members (025, G5), nor its bindings (005, K7); a builtin group follows the sync (G7).
const WIDER_GROUP = "Périmètre plus large que le vôtre (core.group.escalation).";
const WIDER_BINDING = "Périmètre plus large que le vôtre (core.binding.escalation).";
const BUILTIN = "Groupe livré : ses rôles suivent la synchronisation (core.group.builtin).";
const BUILTIN_MEMBERS =
  "Groupe livré : ses membres se gèrent par un administrateur global (core.group.builtin).";
// An object whose writes are all refused for one reason says it at its top (021, « refus
// annoncé en tête »): a notice and the code of the rule; each action keeps its tooltip.
interface ReadOnly {
  title: string;
  code: string;
}

interface MemberRow extends GroupMember {
  pending?: Pending;
}
interface BindingRow extends GroupBinding {
  pending?: Pending;
}
/** The group shown in the panel, with the reasons that refuse its actions. */
interface ShownGroup {
  key: string;
  label: string;
  builtin: boolean;
  members: MemberRow[];
  bindings: BindingRow[];
  memberReason?: string;
  bindingReason?: string;
  addBindingReason?: string;
  editReason?: string;
  closeReason?: string;
  readOnly?: ReadOnly;
}
const supervision = (pending: boolean): ShownGroup => ({
  key: "supervision",
  label: "Équipe supervision",
  builtin: false,
  members: [
    ...SUPERVISION_MEMBERS.map((m) =>
      pending && m.name === "Sam Leroy" ? { ...m, pending: "removed" as const } : m,
    ),
    ...(pending ? [{ ...SUPERVISION_NEW_MEMBER, pending: "added" as const }] : []),
  ],
  bindings: [
    ...SUPERVISION_BINDINGS.map((b) =>
      pending && b.role === "reader" ? { ...b, pending: "removed" as const } : b,
    ),
    ...(pending ? [{ ...SUPERVISION_NEW_BINDING, pending: "added" as const }] : []),
  ],
});
const GROUP_SHOWN: Record<string, ShownGroup> = {
  groupes: supervision(false),
  "groupe-edition": supervision(true),
  "groupe-refus": {
    key: "astreinte",
    label: "Astreinte",
    builtin: false,
    members: ASTREINTE_MEMBERS,
    bindings: ASTREINTE_BINDINGS,
    memberReason: WIDER_GROUP,
    bindingReason: WIDER_BINDING,
    addBindingReason: WIDER_BINDING,
    editReason: WIDER_GROUP,
    closeReason: WIDER_GROUP,
    readOnly: {
      title: "Lecture seule : ce groupe est lié sur un périmètre plus large que le vôtre.",
      code: "core.group.escalation",
    },
  },
  secours: {
    key: "secours",
    label: "Secours",
    builtin: true,
    members: SECOURS_MEMBERS,
    bindings: SECOURS_BINDINGS,
    memberReason: BUILTIN_MEMBERS,
    bindingReason: BUILTIN,
    addBindingReason: BUILTIN,
    editReason: BUILTIN,
    closeReason: BUILTIN,
    readOnly: {
      title: "Groupe livré : seuls ses membres se gèrent, par un administrateur global.",
      code: "core.group.builtin",
    },
  },
};
const group = GROUP_SHOWN[props.vue];
const groupActions: ToolbarAction[] = [
  {
    id: "edit",
    label: "Modifier",
    icon: "pencil",
    shortcut: ["e"],
    priority: 2,
    ...(group?.editReason ? { reason: group.editReason } : {}),
  },
  {
    id: "close",
    label: "Clore…",
    icon: "trash",
    ...(group?.closeReason ? { reason: group.closeReason } : {}),
  },
];
/** A name cut at ` / ` (`émetteur / groupe`): it wraps there, never inside a key. */
const parts = (name: string): string[] => name.split(" / ");
const memberCount = (g: ShownGroup): number =>
  g.members.filter((m) => m.pending !== "added").length;
const bindingCount = (g: ShownGroup): number =>
  g.bindings.filter((b) => b.pending !== "added").length;

const groupColumns: GridColumn[] = [
  { id: "key", label: "Groupe", width: "lg", sortable: true, filterable: true },
  { id: "builtin", label: "Livré", width: "sm", sortable: true, filterable: true, priority: 3 },
  { id: "members", label: "Membres", width: "sm", align: "end", sortable: true, priority: 2 },
  { id: "roles", label: "Rôles", width: "sm", align: "end", sortable: true, priority: 3 },
  { id: "about", label: "Description", width: "fill", filterable: true, priority: 2 },
];
const groupSort = ref<GridSort>({ column: "key", direction: "ascending" });
const groupState = (row: Record<string, unknown>): RowState | undefined =>
  editing && row.key === "supervision" ? "draft" : undefined;
/** The operations of the draft in the view `groupe-edition` (its tab « Brouillon 4 »). */
const groupOperations = [
  { what: "Retirer le membre", target: "Sam Leroy" },
  { what: "Ajouter le membre", target: SUPERVISION_NEW_MEMBER.name },
  { what: "Retirer le rôle", target: "reader · Domaine : dc-nord" },
  { what: "Ajouter le rôle", target: "editor · Site : site-ouest" },
];

// The key first and of fixed width, the description last: the right panel covers the end
// of the table (024/M8), not its key columns.
const roleColumns: GridColumn[] = [
  { id: "key", label: "Rôle", width: "lg", sortable: true, filterable: true },
  { id: "builtin", label: "Intégré", width: "sm", sortable: true, filterable: true, priority: 3 },
  {
    id: "permissions",
    label: "Permissions",
    width: "sm",
    align: "end",
    sortable: true,
    priority: 2,
  },
  { id: "groups", label: "Groupes", width: "sm", align: "end", sortable: true, priority: 3 },
  { id: "about", label: "Description", width: "fill", filterable: true, priority: 2 },
];
const roleSort = ref<GridSort>({ column: "key", direction: "ascending" });
const roleActions: ToolbarAction[] = [
  { id: "edit", label: "Modifier", icon: "pencil", shortcut: ["e"], priority: 2 },
  { id: "copy", label: "Dupliquer", icon: "copy", priority: 1 },
  { id: "delete", label: "Supprimer…", icon: "trash", reason: "Le rôle est lié à 4 groupes." },
];

const identityColumns: GridColumn[] = [
  { id: "name", label: "Identité", width: "lg", sortable: true, filterable: true },
  { id: "state", label: "État", width: "md", filterable: true },
  { id: "groups", label: "Groupes", width: "sm", align: "end", sortable: true, priority: 3 },
  { id: "issuer", label: "Émetteur", width: "fill", mono: true, filterable: true, priority: 2 },
];
/**
 * The chosen identity, and the groups of its IdP at its last login (no date: not kept,
 * 025, Q-025-1), each with the kartenn group it gives, if any.
 */
const chosenIdentity = IDENTITIES.find((i) => i.key === "lou.bernard")!;
const lastLoginGroups = chosenIdentity.lastLoginIdpGroups.map((name) => ({
  name: `${chosenIdentity.issuer} / ${name}`,
  group: IDP_GROUP_TARGETS[name] ?? null,
}));
const identitySort = ref<GridSort>({ column: "name", direction: "ascending" });
const identityActions: ToolbarAction[] = [
  { id: "disable", label: "Désactiver", icon: "user-x", priority: 2 },
  { id: "revoke", label: "Révoquer les sessions", icon: "log-out", priority: 1 },
];

const accountColumns: GridColumn[] = [
  { id: "name", label: "Compte", width: "fill", mono: true, sortable: true },
  { id: "state", label: "État", width: "md", filterable: true },
  { id: "groups", label: "Groupes", width: "sm", align: "end", sortable: true, priority: 3 },
  { id: "secrets", label: "Secrets", width: "sm", align: "end", sortable: true, priority: 2 },
  { id: "lastUse", label: "Dernier usage", width: "lg", mono: true, sortable: true, priority: 2 },
];
const SECRET = "ksk_live_7Q2mX9vR4tB8nL1cW6pZ3hJ5";
</script>

<template>
  <Cadre
    :title="TITLE[tab]"
    :active="tab"
    space="admin"
    :draft="editing ? groupOperations.length : 0"
    :panel="props.vue === 'secret' ? undefined : 'detail'"
    :crumbs="[
      { label: 'Administration', to: '/admin' },
      tab === 'accounts' ? { label: TITLE[tab] } : { label: TITLE[tab], to: `/admin/${tab}` },
      ...(group ? [{ label: group.key, mono: true }] : []),
      ...(tab === 'identities' ? [{ label: 'lou.bernard', mono: true }] : []),
      ...(tab === 'roles' ? [{ label: 'lecteur-reseau', mono: true }] : []),
    ]"
  >
    <template v-if="group">
      <Panneau title="Groupes" :detail="String(GROUPS.length)" main flush>
        <!-- In the title row, as in every main panel; at its start: the right panel may
             cover the end of the surface. -->
        <template #actions>
          <Toolbar label="Actions des groupes" :actions="createActions" align="start" />
        </template>
        <DataGrid
          v-model:sort="groupSort"
          label="Groupes"
          :columns="groupColumns"
          :rows="GROUPS"
          row-key="key"
          selectable="single"
          :selected="[group.key]"
          :row-state="groupState"
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

    <template v-else-if="tab === 'identities'">
      <Panneau title="Identités" :detail="String(IDENTITIES.length)" main flush>
        <DataGrid
          v-model:sort="identitySort"
          label="Identités"
          :columns="identityColumns"
          :rows="IDENTITIES"
          row-key="key"
          selectable="single"
          :selected="['lou.bernard']"
          stack
        >
          <template #cell="{ row, column, value }">
            <StatusBadge
              v-if="column.id === 'state'"
              :state="row.state === 'active' ? 'ok' : 'neutral'"
              :label="row.state === 'active' ? 'Active' : 'Désactivée'"
            />
            <template v-else>{{ value }}</template>
          </template>
        </DataGrid>
      </Panneau>
    </template>

    <template v-else-if="tab === 'roles'">
      <Panneau title="Rôles" :detail="String(ROLES.length)" main flush>
        <template #actions>
          <Toolbar label="Actions des rôles" :actions="createActions" align="start" />
        </template>
        <DataGrid
          v-model:sort="roleSort"
          label="Rôles"
          :columns="roleColumns"
          :rows="ROLES"
          row-key="key"
          selectable="single"
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

    <template v-else>
      <Panneau title="Comptes de service" :detail="String(ACCOUNTS.length)" main flush>
        <template #actions>
          <Button variant="primary" icon="plus" :shortcut="['c']">Créer</Button>
        </template>
        <DataGrid
          label="Comptes de service"
          :columns="accountColumns"
          :rows="ACCOUNTS"
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
      <!-- The focus opens on the copy of the secret, never on the cross: closed by mistake,
           a secret shown once is lost (relecture du 2026-10-26). -->
      <Dialog :open="true" title="Nouveau secret pour sync-ipam">
        <Stack gap="3">
          <InlineAlert tone="warn" title="Copiez-le maintenant : il ne sera plus jamais affiché">
            kartenn n'en garde qu'une empreinte ; perdu, il se remplace par un nouveau secret.
          </InlineAlert>
          <CodeBadge :code="SECRET" data-autofocus />
          <Attributs :items="[{ key: 'expires', label: 'Expire le', value: '2027-01-15' }]">
            <template #expires><time datetime="2027-01-15">2027-01-15</time></template>
          </Attributs>
        </Stack>
        <template #actions>
          <Button variant="primary" icon="check">J'ai copié le secret</Button>
        </template>
      </Dialog>
    </template>

    <template #detail>
      <Stack v-if="group" gap="4">
        <Stack gap="2">
          <h2 class="k-ecran-subtitle">
            {{ group.label }} <span class="k-mono k-ecran-muted">{{ group.key }}</span>
          </h2>
          <span v-if="group.builtin">
            <StatusBadge state="info" label="Groupe livré" />
          </span>
          <InlineAlert v-if="group.readOnly" tone="warn" icon="lock" :title="group.readOnly.title">
            <CodeBadge :code="group.readOnly.code" />
          </InlineAlert>
          <Barre label="Actions du groupe" :actions="groupActions" align="start" />
        </Stack>
        <section aria-labelledby="k-ecran-membres">
          <Stack gap="2">
            <h3 id="k-ecran-membres" class="k-ecran-subtitle">
              Membres · {{ memberCount(group) }}
              <span v-if="editing" class="k-ecran-muted k-ecran-poids">(brouillon : −1, +1)</span>
            </h3>
            <ul class="k-ecran-list k-ecran-lignes">
              <li v-for="member in group.members" :key="member.name" :data-pending="member.pending">
                <span class="k-ecran-ligne__texte">
                  <Icon
                    v-if="member.pending"
                    :name="PENDING[member.pending].icon"
                    meaningful
                    :label="PENDING[member.pending].label"
                    class="k-ecran-ligne__marque"
                  />
                  <span
                    class="k-ecran-ligne__libelle"
                    :class="{ 'k-mono': member.kind === 'idp' || member.kind === 'service' }"
                    ><template v-for="(part, i) in parts(member.name)" :key="i"
                      ><template v-if="i"> / </template
                      ><span class="k-ecran-insecable">{{ part }}</span></template
                    ></span
                  >
                </span>
                <span class="k-ecran-muted k-ecran-ligne__valeur"
                  ><span>{{ MEMBER_KIND[member.kind] }}</span
                  ><span v-if="member.until"
                    >jusqu'au
                    <time class="k-num" :datetime="member.until">{{ member.until }}</time></span
                  ></span
                >
                <span class="k-ecran-ligne__actions">
                  <IconButton
                    v-if="member.pending === 'removed'"
                    icon="undo-2"
                    size="sm"
                    :label="`Annuler le retrait de ${member.name}`"
                  />
                  <IconButton
                    v-else-if="member.pending === 'added'"
                    icon="x"
                    size="sm"
                    :label="`Retirer l'ajout de ${member.name} du brouillon`"
                  />
                  <IconButton
                    v-else
                    icon="trash"
                    size="sm"
                    :label="`Retirer ${member.name} du groupe`"
                    :reason="group.memberReason"
                  />
                </span>
              </li>
            </ul>
            <span>
              <Button size="sm" variant="ghost" icon="plus" :reason="group.memberReason"
                >Ajouter un membre</Button
              >
            </span>
          </Stack>
        </section>
        <section aria-labelledby="k-ecran-roles">
          <Stack gap="2">
            <h3 id="k-ecran-roles" class="k-ecran-subtitle">
              Rôles sur périmètres · {{ bindingCount(group) }}
              <span v-if="editing" class="k-ecran-muted k-ecran-poids">(brouillon : −1, +1)</span>
            </h3>
            <ul class="k-ecran-list k-ecran-lignes">
              <li
                v-for="binding in group.bindings"
                :key="`${binding.role} ${binding.scope}`"
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
                  <span class="k-ecran-ligne__libelle"
                    >{{ binding.roleLabel }}
                    <code class="k-ecran-muted k-ecran-ligne__cle">{{ binding.role }}</code></span
                  >
                </span>
                <span class="k-ecran-muted k-ecran-ligne__valeur"
                  ><span>{{ binding.scope }}</span
                  ><span v-if="binding.until"
                    >jusqu'au
                    <time class="k-num" :datetime="binding.until">{{ binding.until }}</time></span
                  ></span
                >
                <span class="k-ecran-ligne__actions">
                  <IconButton
                    v-if="binding.pending === 'removed'"
                    icon="undo-2"
                    size="sm"
                    :label="`Annuler le retrait du rôle ${binding.role} · ${binding.scope}`"
                  />
                  <IconButton
                    v-else-if="binding.pending === 'added'"
                    icon="x"
                    size="sm"
                    :label="`Retirer l'ajout du rôle ${binding.role} · ${binding.scope} du brouillon`"
                  />
                  <template v-else>
                    <IconButton
                      icon="pencil"
                      size="sm"
                      :label="`Modifier le rôle ${binding.role} · ${binding.scope}`"
                      :reason="group.bindingReason"
                    />
                    <IconButton
                      icon="trash"
                      size="sm"
                      :label="`Retirer le rôle ${binding.role} · ${binding.scope}`"
                      :reason="group.bindingReason"
                    />
                  </template>
                </span>
              </li>
            </ul>
            <span>
              <Button size="sm" variant="ghost" icon="plus" :reason="group.addBindingReason"
                >Ajouter un rôle</Button
              >
            </span>
          </Stack>
        </section>
      </Stack>

      <Stack v-else-if="tab === 'identities'" gap="4">
        <Stack gap="2">
          <h2 class="k-ecran-subtitle">
            Lou Bernard <span class="k-mono k-ecran-muted">lou.bernard</span>
          </h2>
          <span><StatusBadge state="ok" label="Active" /></span>
          <Barre label="Actions de l'identité" :actions="identityActions" align="start" />
        </Stack>
        <section aria-labelledby="k-ecran-groupes">
          <Stack gap="2">
            <h3 id="k-ecran-groupes" class="k-ecran-subtitle">
              Groupes · {{ IDENTITY_GROUPS.length }}
            </h3>
            <ul class="k-ecran-list k-ecran-lignes k-ecran-lignes--lecture">
              <li v-for="g in IDENTITY_GROUPS" :key="g.key">
                <KeyLabel :value="g.key" :label="g.label" stacked />
                <span v-if="g.via" class="k-ecran-muted"
                  >via
                  <span class="k-mono"
                    ><template v-for="(part, i) in parts(g.via)" :key="i"
                      ><template v-if="i"> / </template
                      ><span class="k-ecran-insecable">{{ part }}</span></template
                    ></span
                  ></span
                >
                <span v-else class="k-ecran-muted">direct</span>
              </li>
            </ul>
          </Stack>
        </section>
        <section aria-labelledby="k-ecran-droits">
          <Stack gap="2">
            <h3 id="k-ecran-droits" class="k-ecran-subtitle">
              Droits effectifs · {{ IDENTITY_RIGHTS.length }}
            </h3>
            <ul class="k-ecran-list k-ecran-lignes k-ecran-lignes--lecture">
              <li v-for="right in IDENTITY_RIGHTS" :key="`${right.role} ${right.scope}`">
                <KeyLabel :value="right.role" :label="right.roleLabel" stacked />
                <span class="k-ecran-muted k-ecran-ligne__valeur"
                  ><span>{{ right.scope }}</span
                  ><span>via le groupe {{ right.group }}</span></span
                >
              </li>
            </ul>
          </Stack>
        </section>
        <section aria-labelledby="k-ecran-session">
          <Stack gap="2">
            <h3 id="k-ecran-session" class="k-ecran-subtitle">
              Groupes de l'IdP lors de sa dernière connexion · {{ lastLoginGroups.length }}
            </h3>
            <p v-if="!lastLoginGroups.length" class="k-ecran-muted">
              Aucun : cette identité ne s'est jamais connectée par un IdP.
            </p>
            <ul v-else class="k-ecran-list k-ecran-lignes k-ecran-lignes--lecture">
              <li v-for="g in lastLoginGroups" :key="g.name">
                <span class="k-mono"
                  ><template v-for="(part, i) in parts(g.name)" :key="i"
                    ><template v-if="i"> / </template
                    ><span class="k-ecran-insecable">{{ part }}</span></template
                  ></span
                >
                <span class="k-ecran-muted">{{ g.group ?? "aucun groupe kartenn" }}</span>
              </li>
            </ul>
          </Stack>
        </section>
      </Stack>

      <Stack v-else-if="tab === 'roles'" gap="4">
        <Stack gap="2">
          <h2 class="k-ecran-subtitle">
            Lecteur réseau <span class="k-mono k-ecran-muted">lecteur-reseau</span>
          </h2>
          <Barre label="Actions du rôle" :actions="roleActions" align="start" />
        </Stack>
        <section aria-labelledby="k-ecran-permissions">
          <Stack gap="2">
            <h3 id="k-ecran-permissions" class="k-ecran-subtitle">
              Permissions · {{ ROLE_PERMISSIONS.length }}
            </h3>
            <ul class="k-ecran-list k-ecran-lignes">
              <li v-for="grant in ROLE_PERMISSIONS" :key="grant.code">
                <span class="k-ecran-ligne__libelle">{{ grant.label }}</span>
                <CodeBadge :code="grant.code" />
                <span class="k-ecran-ligne__actions">
                  <IconButton
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
        <section aria-labelledby="k-ecran-lies">
          <Stack gap="2">
            <h3 id="k-ecran-lies" class="k-ecran-subtitle">
              Groupes liés · {{ ROLE_GROUPS.length }}
            </h3>
            <ul class="k-ecran-list k-ecran-lignes k-ecran-lignes--lecture">
              <li v-for="g in ROLE_GROUPS" :key="g.key">
                <KeyLabel :value="g.key" :label="g.label" stacked />
                <span class="k-ecran-muted">{{ g.scope }}</span>
              </li>
            </ul>
          </Stack>
        </section>
      </Stack>
    </template>
    <template v-if="editing" #draft>
      <Stack as="ol" gap="2" class="k-ecran-list" aria-label="Opérations du brouillon">
        <li v-for="op in groupOperations" :key="op.what" class="k-ecran-draft-item">
          <Cluster justify="between" :wrap="false" gap="2">
            <Cluster gap="2" :wrap="false" class="k-ecran-op">
              <Icon name="draft" />
              <code>supervision</code>
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
  /* One entry per line, in aligned columns (024-11: the label on the left, the value on
   * the right): a member and its kind, a role and its scope, a permission and its served
   * name; then the actions of the row, in a last column at the end, at the same place on
   * every row. The label takes the room left and wraps; the value does not. */
  .k-ecran-lignes {
    display: grid;
    grid-template-columns: minmax(0, 1fr) max-content max-content;
    gap: var(--k-space-1) var(--k-space-3);
    align-items: center;
  }

  .k-ecran-lignes > li {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
    min-block-size: var(--k-row-height);
    padding-inline: var(--k-space-2);
    border-radius: var(--k-radius-sm);
  }

  /* Read only (an identity, the groups of a role): the entry left, its value right, both
   * wrapping in a narrow panel. */
  .k-ecran-lignes--lecture {
    grid-template-columns: minmax(0, 1fr) minmax(0, auto);
  }

  .k-ecran-lignes > li > * {
    min-inline-size: 0;
  }

  /* A key or a part of a name never breaks at its hyphens; the line breaks before it. */
  .k-ecran-lignes code,
  .k-ecran-insecable {
    white-space: nowrap;
  }

  /* The key of a role under its label, on every row: one height for all (relecture du
   * 2026-10-26), never the key beside a short label and under a long one. */
  .k-ecran-ligne__cle {
    display: block;
  }

  /* A value and its detail (an end date, the granting group) on a second line. */
  .k-ecran-ligne__valeur {
    display: grid;
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
