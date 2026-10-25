<script setup lang="ts">
// E4 — Changeset et revue (design « Maquettes »; 021, « Changesets et différentiel »; 024/M7,
// M14): differential grouped by object, attached anomalies, review actions according to the
// rights of the reader (a reviewer, a reviewer who lacks a right, the author held by a
// policy), a refused action greyed with its reason in its tooltip and no box of the
// required rights, the state « en revue », rejection with a motive.
import {
  Button,
  CodeBadge,
  Dialog,
  DiffList,
  Grid,
  InlineAlert,
  KeyLabel,
  Stack,
  StatusBadge,
  Tabs,
  TextField,
  type DiffGroup,
  type TabItem,
  type ToolbarAction,
} from "@kartenn/ui";
import { ref } from "vue";
import Attributs from "./EcranAttributs.vue";
import Barre from "./EcranBarre.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { ID } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const author = props.vue === "auteur";
/** The reader may apply the prefixes, not the VLAN the changeset also touches. */
const lacking = props.vue === "droits";
const tab = ref("diff");
const motive = ref("Le VLAN 120 reste sur le site ouest : renuméroter d'abord le VLAN.");

const tabs: TabItem[] = [
  { value: "diff", label: "Différentiel", count: 5 },
  { value: "operations", label: "Opérations", count: 4 },
  { value: "violations", label: "Violations", count: 1 },
  { value: "sections", label: "Sections de modules", count: 2 },
];

const groups: DiffGroup[] = [
  {
    id: "g1",
    title: "Serveurs",
    objectKey: "192.0.2.0/26",
    kind: "changed",
    changes: [
      { attribute: "Site (core.site)", kind: "changed", before: "site-ouest", after: "site-est" },
    ],
  },
  {
    id: "g2",
    title: "Supervision",
    objectKey: "192.0.2.64/26",
    kind: "changed",
    changes: [
      {
        attribute: "Valeur",
        kind: "changed",
        before: "192.0.2.64/26",
        after: "198.51.100.0/26",
        mono: true,
      },
      { attribute: "Propriétaire (core.owner)", kind: "added", after: "Atelier Nord" },
    ],
  },
  {
    id: "g3",
    title: "Supervision salle B",
    objectKey: "198.51.100.64/26",
    kind: "added",
    changes: [
      { attribute: "Valeur", kind: "added", after: "198.51.100.64/26", mono: true },
      { attribute: "Nature", kind: "added", after: "réseau" },
    ],
  },
  {
    id: "g4",
    title: "Ancienne passerelle",
    objectKey: "192.0.2.65",
    kind: "removed",
    changes: [{ attribute: "Usage", kind: "removed", before: "passerelle" }],
  },
  {
    id: "g5",
    title: "VLAN 120 serveurs",
    objectKey: "vlan:120",
    kind: "anomaly",
    changes: [
      {
        attribute: "Site du VLAN (vlan.site.mismatch)",
        kind: "anomaly",
        before: "site-ouest",
        after: "site-est",
      },
    ],
  },
];

/** The review actions, by the rights of the reader (024/M7, M14): refused ones stay. */
const HELD = "Une politique retient ce changeset : un autre relecteur doit le valider.";
const lacks = lacking ? { forbidden: "vlan.apply" } : {};
const actions: ToolbarAction[] = author
  ? [
      { id: "withdraw", label: "Retirer", icon: "undo-2", priority: 2 },
      {
        id: "approve",
        label: "Valider",
        icon: "check",
        variant: "primary",
        priority: 3,
        reason: HELD,
      },
      { id: "effect", label: "Effet sur la politique", icon: "matrix", priority: 1 },
      { id: "copy", label: "Copier l'identifiant", icon: "copy" },
    ]
  : [
      { id: "reject", label: "Rejeter…", icon: "x", priority: 2, ...lacks },
      { id: "approve", label: "Valider", icon: "check", variant: "primary", priority: 3, ...lacks },
      { id: "effect", label: "Effet sur la politique", icon: "matrix", priority: 1 },
      { id: "copy", label: "Copier l'identifiant", icon: "copy" },
      {
        id: "withdraw",
        label: "Retirer",
        icon: "undo-2",
        reason: "Seul l'auteur retire son changeset.",
      },
    ];
</script>

<template>
  <Cadre
    title="Changeset Renumérotation salle B"
    :active="author ? 'changesets' : 'proposals'"
    space="changes"
    :draft="0"
    :crumbs="[
      { label: 'Changesets', to: '/changesets' },
      ...(author ? [] : [{ label: 'À instruire', to: '/proposals' }]),
      { label: 'Renumérotation salle B' },
    ]"
  >
    <Panneau title="Renumérotation salle B" detail="5 objets" main>
      <template #toolbar>
        <Barre label="Actions de revue" :actions="actions">
          <StatusBadge state="review" label="En revue" />
        </Barre>
      </template>
      <InlineAlert
        v-if="author"
        tone="info"
        title="Vous êtes l'auteur de ce changeset : un autre relecteur doit le valider."
        :items="[
          {
            message: 'Une politique d\'approbation retient les changements de VLAN.',
            code: 'changeset.approval.self',
          },
        ]"
      />
      <Grid columns="auto" gap="3" class="k-ecran-attrs-tiles">
        <Attributs
          :items="[
            { key: 'nature', label: 'Nature' },
            { key: 'author', label: 'Auteur', value: 'Lou Bernard' },
          ]"
        >
          <template #nature><KeyLabel value="approval" label="Avec approbation" /></template>
        </Attributs>
        <Attributs
          :items="[
            { key: 'submitted', label: 'Soumis', value: '2026-01-14 16:02', mono: true },
            { key: 'effect', label: 'Effet au', value: 'présent' },
          ]"
        />
        <Attributs
          :items="[
            { key: 'retro', label: 'Rétroactif', value: 'non' },
            { key: 'id', label: 'Identifiant' },
          ]"
        >
          <template #id><CodeBadge :code="ID.changeset" /></template>
        </Attributs>
      </Grid>
      <Tabs v-model="tab" :tabs="tabs" label="Contenu du changeset">
        <template #diff>
          <Stack gap="3">
            <InlineAlert
              tone="warn"
              title="Une anomalie rattachée"
              :items="[
                {
                  message: 'Le VLAN 120 reste sur le site ouest, ses réseaux passent à l\'est.',
                  code: 'vlan.site.mismatch',
                },
              ]"
            />
            <DiffList :groups="groups" />
          </Stack>
        </template>
      </Tabs>
    </Panneau>
    <Dialog
      v-if="props.vue === 'rejet'"
      :open="true"
      title="Rejeter « Renumérotation salle B »"
      description="Le motif est envoyé à l'auteur et garde la trace du rejet."
    >
      <TextField
        v-model="motive"
        label="Motif du rejet"
        required
        hint="Une phrase : ce qui manque ou ce qu'il faut changer."
      />
      <template #actions>
        <Button variant="ghost">Annuler</Button>
        <Button variant="danger" icon="x">Rejeter</Button>
      </template>
    </Dialog>
  </Cadre>
</template>
