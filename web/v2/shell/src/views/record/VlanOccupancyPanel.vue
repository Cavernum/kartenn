<script setup lang="ts">
// The occupancy of a group of VLAN (026, REQ-008-20; maquette E12, un groupe): its VIDs
// used, reserved and free in bars, the share taken beside the title, the next free VID,
// from the first of the range or from a VID typed, and the runs of VIDs in the same state.
// Read at the dates of the page; three states.
import { reads, type components } from "@kartenn/sdk";
import {
  BarList,
  ExpandableGrid,
  Skeleton,
  Stack,
  StatusBadge,
  TextField,
  type ChartPoint,
  type GridColumn,
} from "@kartenn/ui";
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import { useProblems } from "../../problems/index.ts";
import ProblemState from "../../problems/ProblemState.vue";
import { useTime } from "../../time/index.ts";
import AttributeList from "../parts/AttributeList.vue";
import ViewPanel from "../parts/ViewPanel.vue";
import { SLICE_STATES } from "../parts/vlan.ts";

type Occupancy = components["schemas"]["VlanOccupancy"];
type NextFree = components["schemas"]["VlanNextFree"];
type Slice = components["schemas"]["VlanSlice"];

const props = defineProps<{ id: string; name: string; range: string | null }>();
const { t, locale } = useI18n();
const time = useTime();
const { sentence } = useProblems();
const dated = () => ({ validAt: time.value.validAt, asOf: time.value.asOf });

const occupancy = useQuery(() =>
  reads.sub<Occupancy>("vlan_group", props.id, "occupancy", null, dated()),
);
const free = useQuery(() =>
  reads.sub<NextFree>("vlan_group", props.id, "next-free", null, dated()),
);

// The next free VID from a VID typed: read once the typing pauses, a whole number only.
const fromText = ref("");
const from = ref<string | null>(null);
const fromError = ref<string | undefined>();
let pause: ReturnType<typeof setTimeout> | undefined;
watch(fromText, (text) => {
  clearTimeout(pause);
  pause = setTimeout(() => {
    const typed = text.trim();
    const valid = /^\d{1,4}$/.test(typed);
    // Typed but not a VID: said on the field (3.3.1), nothing read.
    fromError.value = typed !== "" && !valid ? t("views.vlan.fromInvalid") : undefined;
    from.value = valid ? typed : null;
  }, 300);
});
onBeforeUnmount(() => clearTimeout(pause));
const fromFree = useQuery(() => ({
  ...reads.sub<NextFree>("vlan_group", props.id, "next-free", { from: from.value ?? "" }, dated()),
  enabled: from.value !== null,
}));

const number = (value: number) => new Intl.NumberFormat(locale.value).format(value);
const data = computed(() => occupancy.data.value?.data);
const total = computed(() =>
  data.value ? data.value.used + data.value.reserved + data.value.free : 0,
);
/** The share used or reserved, in whole percent; under one percent but not none, « < 1 % ». */
const share = computed(() => {
  if (!data.value || total.value === 0) return undefined;
  const taken = data.value.used + data.value.reserved;
  const percent = Math.floor((taken * 100) / total.value);
  return t("views.vlan.taken", { share: percent === 0 && taken > 0 ? "< 1 %" : `${percent} %` });
});
const counts = computed<ChartPoint[]>(() =>
  (["used", "reserved", "free"] as const).map((state) => ({
    key: state,
    label: t(`views.vlan.slice.${state}`),
    value: data.value?.[state] ?? 0,
  })),
);
const freeText = (query: typeof free) =>
  query.isPending.value
    ? "…"
    : query.isError.value
      ? sentence(query.error.value)
      : String(query.data.value?.data.vid ?? t("views.vlan.noneFree"));
const items = computed(() => [
  { key: "next", label: t("views.vlan.nextFree"), value: freeText(free), mono: true },
]);
/** The next free VID from the one typed, said by a status region (4.1.3). */
const fromSaid = computed(() =>
  from.value === null || fromFree.isPending.value
    ? ""
    : t("views.vlan.nextFreeFrom", { from: from.value, vid: freeText(fromFree) }),
);

const slices = computed(() =>
  (data.value?.slices ?? []).map((slice: Slice) => ({
    key: slice.first === slice.last ? String(slice.first) : `${slice.first}–${slice.last}`,
    state: slice.state,
    count: slice.last - slice.first + 1,
  })),
);
const sliceColumns = computed<GridColumn[]>(() => [
  { id: "key", label: t("views.field.vid"), width: "sm", mono: true },
  { id: "state", label: t("views.vlan.state"), width: "fill" },
]);
</script>

<template>
  <ViewPanel :title="t('views.record.occupancy')" :detail="share">
    <ProblemState
      v-if="occupancy.isError.value"
      :title="t('views.vlan.occupancyError')"
      :error="occupancy.error.value"
      :retry="occupancy.refetch"
    />
    <Skeleton v-else-if="occupancy.isPending.value" variant="text" :rows="4" />
    <Stack v-else gap="3">
      <BarList
        :label="t('views.vlan.occupancyOf', { name: props.name })"
        :items="counts"
        :max="total"
        :point-label="t('views.vlan.state')"
        :value-label="t('views.vlan.vids')"
        :format="number"
      />
      <AttributeList :items="items" />
      <TextField
        v-model="fromText"
        :label="t('views.vlan.fromLabel')"
        mono
        size="sm"
        :hint="props.range ? t('views.vlan.fromHint', { range: props.range }) : undefined"
        :error="fromError"
      />
      <!-- Present before its sentence, so that the sentence is said. -->
      <p class="k-mono" role="status">{{ fromSaid }}</p>
      <ExpandableGrid
        :label="t('views.vlan.slicesOf', { name: props.name })"
        :columns="sliceColumns"
        :rows="slices"
        row-key="key"
        :visible-rows="6"
      >
        <template #cell="{ row, column, value }">
          <StatusBadge
            v-if="column.id === 'state' && SLICE_STATES[String(value)]"
            :state="SLICE_STATES[String(value)]!"
            :label="
              t(
                `views.vlan.sliceCount.${String(value)}`,
                { n: number(Number(row.count)) },
                Number(row.count),
              )
            "
          />
          <template v-else>{{ value }}</template>
        </template>
      </ExpandableGrid>
    </Stack>
  </ViewPanel>
</template>
