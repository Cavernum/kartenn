<script setup lang="ts">
// E4 — Changeset et revue (design « Maquettes »; 021, « Changesets et différentiel »):
// differential grouped by object, attached anomalies, review actions according to the
// rights of the reader (a reviewer, the author), the state « en revue », rejection with a
// motive.
import {
  Button,
  CodeBadge,
  Dialog,
  DiffList,
  InlineAlert,
  KeyLabel,
  Menu,
  Split,
  Stack,
  StatusBadge,
  Tabs,
  TextField,
  type DiffGroup,
  type MenuItem,
  type TabItem,
} from "@kartenn/ui";
import { ref } from "vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { ID } from "./donnees.ts";

const props = defineProps<{ vue: string }>();
const author = props.vue === "auteur";
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

const more: MenuItem[] = [
  { id: "copy", label: "Copier l'identifiant", icon: "copy" },
  { id: "effect", label: "Effet sur la politique", icon: "matrix" },
  author
    ? { id: "withdraw", label: "Retirer", icon: "undo-2" }
    : {
        id: "withdraw",
        label: "Retirer",
        icon: "undo-2",
        disabled: true,
        reason: "Réservé à l'auteur (changeset.state.not_author)",
      },
  { kind: "separator" },
  { id: "revert", label: "Annuler par un changeset inverse…", icon: "rotate-ccw-clock" },
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
    <Panneau title="Renumérotation salle B" main>
      <template #actions>
        <StatusBadge state="review" label="En revue" />
        <template v-if="author">
          <Button disabled icon="check">Valider</Button>
        </template>
        <template v-else>
          <Button icon="x">Rejeter…</Button>
          <Button variant="primary" icon="check">Valider</Button>
        </template>
        <Menu label="Autres actions" icon="ellipsis" icon-only align="end" :items="more" />
      </template>
      <InlineAlert
        v-if="author"
        tone="info"
        title="Vous êtes l'auteur de ce changeset : un autre relecteur doit le valider."
        :items="[{ message: 'Validation par l\'auteur refusée', code: 'changeset.approval.self' }]"
      />
      <Split side="end" side-size="sm" gap="3">
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
        <template #side>
          <Stack gap="3">
            <Panneau title="Changeset">
              <Attributs
                :items="[
                  { key: 'nature', label: 'Nature' },
                  { key: 'author', label: 'Auteur', value: 'Lou Bernard' },
                  { key: 'submitted', label: 'Soumis', value: '2026-01-14 16:02', mono: true },
                  { key: 'effect', label: 'Effet au', value: 'présent' },
                  { key: 'retro', label: 'Rétroactif', value: 'non' },
                  { key: 'id', label: 'Identifiant' },
                ]"
              >
                <template #nature><KeyLabel value="approval" label="Avec approbation" /></template>
                <template #id><CodeBadge :code="ID.changeset" /></template>
              </Attributs>
            </Panneau>
            <Panneau title="Approbation" detail="renumerotation" mono>
              <p>Droits requis pour valider :</p>
              <Stack gap="2">
                <Stack gap="1">
                  <span>Appliquer les préfixes</span>
                  <CodeBadge code="core.prefix.apply" />
                </Stack>
                <Stack gap="1">
                  <span>Appliquer les VLAN</span>
                  <CodeBadge code="vlan.apply" />
                </Stack>
                <Stack gap="1">
                  <span>Appliquer les propriétés</span>
                  <CodeBadge code="prop.*.apply" />
                </Stack>
              </Stack>
            </Panneau>
          </Stack>
        </template>
      </Split>
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
