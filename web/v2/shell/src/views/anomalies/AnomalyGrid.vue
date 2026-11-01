<script setup lang="ts">
// Open anomalies in a grid (REQ-021-9; design 021 « Catalogue des écrans » : Anomalies et
// lacunes): the rule (its label, its code always shown), the message (it wraps), the object
// (a link to its record), when it opened, and « Acquitter… » — refused, visible, with its
// reason, out of the present or without `core.anomaly.acknowledge` (024/M14). The page
// around it reads the rows.
import { Button, DataGrid, KeyLabel, type GridColumn, type GridWindow } from "@kartenn/ui";
import { computed, nextTick, ref, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSession } from "../../rights/index.ts";
import { useTime } from "../../time/index.ts";
import { useKeyLabel } from "../data.ts";
import ObjectName from "../record/ObjectName.vue";
import AcknowledgeDialog from "./AcknowledgeDialog.vue";
import type { Anomaly } from "./anomaly.ts";

const props = defineProps<{
  label: string;
  anomalies: Anomaly[];
  loading?: boolean;
  /** The object column: left out on the record of that object. */
  withObject?: boolean;
}>();
const emit = defineEmits<{ window: [window: GridWindow] }>();
const { t, locale } = useI18n();
const time = useTime();
const session = useSession();
const keyLabel = useKeyLabel();

const ACKNOWLEDGE = "core.anomaly.acknowledge";
const refusal = computed(() =>
  !time.value.present
    ? { reason: t("views.anomalies.notPresent") }
    : session.value.rights.has(ACKNOWLEDGE)
      ? {}
      : { forbidden: ACKNOWLEDGE },
);

const columns = computed<GridColumn[]>(() => [
  { id: "rule", label: t("views.anomalies.rule"), width: "md" },
  { id: "message", label: t("views.anomalies.message"), width: "fill", wrap: true },
  ...(props.withObject
    ? [
        {
          id: "object",
          label: t("views.anomalies.object"),
          width: "md" as const,
          priority: 2 as const,
        },
      ]
    : []),
  { id: "opened_at", label: t("views.anomalies.opened"), width: "md", mono: true, priority: 3 },
  { id: "actions", label: t("views.actions"), width: "sm", actions: true },
]);
const when = (iso: string) =>
  new Intl.DateTimeFormat(locale.value, { dateStyle: "short", timeStyle: "short" }).format(
    new Date(iso),
  );

const chosen = ref<Anomaly | null>(null);
const open = ref(false);
function acknowledge(anomaly: Anomaly): void {
  chosen.value = anomaly;
  open.value = true;
}

// An anomaly acknowledged leaves the list once it is read again, and its button with it:
// the focus, if it was lost with the row, goes to the grid's current cell (2.4.3).
const grid = useTemplateRef<{ $el: HTMLElement }>("grid");
const refocus = ref(false);
watch(
  () => props.anomalies,
  async () => {
    if (!refocus.value) return;
    refocus.value = false;
    await nextTick();
    const root = grid.value?.$el;
    if (root && !root.contains(document.activeElement)) {
      root.querySelector<HTMLElement>("[role='grid'] [tabindex='0']")?.focus();
    }
  },
);
</script>

<template>
  <DataGrid
    ref="grid"
    :label="props.label"
    :columns="columns"
    :rows="props.anomalies as unknown as Record<string, unknown>[]"
    row-key="id"
    :loading="props.loading"
    :loading-rows="4"
    @window="emit('window', $event)"
  >
    <template #cell="{ row, column }">
      <KeyLabel
        v-if="column.id === 'rule'"
        :value="String(row.rule)"
        :label="keyLabel('rule', String(row.rule))"
        show-key
      />
      <template v-else-if="column.id === 'message'">{{ row.message }}</template>
      <ObjectName
        v-else-if="column.id === 'object'"
        :id="(row as unknown as Anomaly).object.id"
        :kind="(row as unknown as Anomaly).object.kind"
        :href="(row as unknown as Anomaly).object.href"
      />
      <time v-else-if="column.id === 'opened_at'" :datetime="String(row.opened_at)">{{
        when(String(row.opened_at))
      }}</time>
      <Button
        v-else
        size="sm"
        variant="ghost"
        icon="circle-check"
        v-bind="refusal"
        @click="acknowledge(row as unknown as Anomaly)"
        >{{ t("views.anomalies.acknowledgeOne")
        }}<span class="k-visually-hidden">
          {{ keyLabel("rule", String(row.rule)) ?? row.rule }} : {{ row.message }}</span
        ></Button
      >
    </template>
  </DataGrid>
  <AcknowledgeDialog
    :id="chosen?.id ?? null"
    v-model:open="open"
    :rule="chosen?.rule ?? ''"
    @done="refocus = true"
  />
</template>
