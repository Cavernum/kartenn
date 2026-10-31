<script setup lang="ts">
// E24 — Échéances (spec 017, task 1; view `expiring` of the plugin `flow`, REQ-017-22, 23;
// Q-017-4; 011/F20): the dated flows and the waivers whose end falls within 30 days
// (`GET /v1/flows?ending_before=`), the nearer of the two ends of each flow, those within
// 7 days marked, sorted by end; from each row, renew the waiver (`POST
// /v1/flows/{id}/waiver`: re-evaluated against today's policy, 200 granted or 202 held by
// the trigger `flow.waiver`) or extend the flow (E22). A waiver beyond 6 months, or granted
// by the requester himself, is refused (`flow.waiver.invalid`), the input kept.
import {
  Button,
  CodeBadge,
  DataGrid,
  DateTimeField,
  Dialog,
  InlineAlert,
  KeyLabel,
  Stack,
  StatusBadge,
  TextField,
  type GridColumn,
  type GridSort,
} from "@kartenn/ui";
import { ref } from "vue";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { RULES, SERVICES } from "./donnees.ts";
import { SPACES_017 } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const invalid = props.vue === "refusee";

type EndKind = "flow" | "waiver";
interface Expiring extends Record<string, unknown> {
  key: string;
  ends: string;
  service: string;
  kind: EndKind;
  end: string;
  days: number;
}
const rows: Expiring[] = [
  {
    key: "flux-1077",
    ends: "srv-web-02 · eth0 → 192.0.2.30",
    service: "postgres",
    kind: "flow",
    end: "2026-01-20",
    days: 5,
  },
  {
    key: "flux-1101",
    ends: "192.0.2.70 → srv-app-01 · eth0",
    service: "api",
    kind: "waiver",
    end: "2026-01-21",
    days: 6,
  },
  {
    key: "flux-1088",
    ends: "srv-sauv-01 · eth0 → 198.51.100.40",
    service: "nfs",
    kind: "flow",
    end: "2026-02-02",
    days: 18,
  },
  {
    key: "flux-1093",
    ends: "srv-app-07 · eth0 → 192.0.2.64/26",
    service: "ssh",
    kind: "waiver",
    end: "2026-02-10",
    days: 26,
  },
  {
    key: "flux-1102",
    ends: "198.51.100.20 → srv-app-01 · eth0",
    service: "https",
    kind: "flow",
    end: "2026-02-13",
    days: 29,
  },
];
const columns: GridColumn[] = [
  { id: "key", label: "Flux", width: "sm", mono: true },
  { id: "ends", label: "Extrémités", width: "lg", mono: true, wrap: true },
  { id: "service", label: "Service", width: "md", priority: 3 },
  { id: "kind", label: "Ce qui finit", width: "md", sortable: true, priority: 2 },
  { id: "end", label: "Fin", width: "md", mono: true, sortable: true },
  { id: "actions", label: "Action", width: "md", actions: true },
];
const sort = ref<GridSort | undefined>({ column: "end", direction: "ascending" });
const reason = ref("Migration des jeux de test reportée à mars");
const risk = ref("Lecture seule, données anonymisées");
const newEnd = ref<string | null>(invalid ? "2026-09-30" : "2026-03-31");
const denyText = RULES.find((r) => r.key === "deny-deve-prod")!.text;
</script>

<template>
  <Cadre
    title="Échéances"
    active="expiring"
    space="flows"
    :spaces="SPACES_017"
    :draft="0"
    :crumbs="[{ label: 'Flux', to: '/x/flow' }, { label: 'Échéances' }]"
  >
    <Panneau title="Échéances" detail="fins sous 30 jours, jusqu'au 2026-02-14" main flush>
      <DataGrid
        v-model:sort="sort"
        label="Flux et dérogations qui finissent sous 30 jours"
        :columns="columns"
        :rows="rows"
        row-key="key"
      >
        <template #cell="{ row, column, value }">
          <KeyLabel
            v-if="column.id === 'service'"
            :value="row.service"
            :label="SERVICES[row.service]?.label ?? null"
            show-key
          />
          <template v-else-if="column.id === 'kind'">{{
            row.kind === "flow" ? "Le flux" : "Sa dérogation"
          }}</template>
          <span v-else-if="column.id === 'end'" class="k-e24-end">
            {{ row.end }}
            <StatusBadge v-if="row.days <= 7" state="warn" :label="`J-${row.days}`" />
            <span v-else class="k-ecran-muted k-num">J-{{ row.days }}</span>
          </span>
          <Button
            v-else-if="column.id === 'actions'"
            size="sm"
            :icon="row.kind === 'flow' ? 'clock' : 'shield'"
            >{{ row.kind === "flow" ? "Prolonger…" : "Recertifier…" }}</Button
          >
          <template v-else>{{ value }}</template>
        </template>
      </DataGrid>
      <template #footer>
        <span class="k-num">5 échéances · 2 sous 7 jours</span>
        <span class="k-ecran-muted"
          >Un flux sans fin n'y figure jamais ; d'un flux dérogé, la plus proche des deux fins</span
        >
      </template>
    </Panneau>

    <Dialog
      v-if="props.vue !== 'echeances'"
      :open="true"
      title="Recertifier la dérogation de flux-1101"
      description="La dérogation est réévaluée contre la politique du jour : le verdict ci-dessous est celui d'aujourd'hui."
    >
      <Stack gap="3">
        <InlineAlert
          v-if="invalid"
          tone="ko"
          title="La dérogation n'a pas été écrite : sa fin dépasse 6 mois."
          :items="[
            {
              message:
                'Une dérogation dure au plus 6 mois depuis son octroi, et pas au-delà de la fin du flux.',
              code: 'flow.waiver.invalid',
            },
          ]"
        >
          Votre saisie est gardée.
        </InlineAlert>
        <p class="k-e24-end">
          <span class="k-ecran-muted">Au 2026-01-15 :</span>
          <StatusBadge state="ko" label="Non conforme" />
          <CodeBadge code="deny-deve-prod" label="Développement vers production" />
        </p>
        <p>{{ denyText }}</p>
        <Attributs
          :items="[
            {
              key: 'current',
              label: 'Dérogation actuelle',
              value: 'jusqu\'au 2026-01-21',
              mono: true,
            },
            { key: 'flow', label: 'Fin du flux', value: 'sans fin' },
          ]"
        />
        <TextField v-model="reason" label="Motif" required />
        <TextField v-model="risk" label="Risque accepté" required />
        <DateTimeField
          v-model="newEnd"
          label="Nouvelle fin"
          granularity="day"
          required
          :error="invalid ? 'Au plus le 2026-07-15, 6 mois après l\'octroi' : undefined"
          hint="Au plus 6 mois après aujourd'hui : le 2026-07-15."
        />
        <p class="k-ecran-muted">
          La politique « Dérogations » (flow.waiver) peut retenir la recertification pour un autre
          approbateur.
        </p>
      </Stack>
      <template #actions>
        <Button variant="ghost">Annuler</Button>
        <Button
          variant="primary"
          icon="shield"
          v-bind="invalid ? { reason: 'La fin dépasse 6 mois.' } : {}"
          >Recertifier</Button
        >
      </template>
    </Dialog>
  </Cadre>
</template>

<style>
@layer components {
  .k-e24-end {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
  }
}
</style>
