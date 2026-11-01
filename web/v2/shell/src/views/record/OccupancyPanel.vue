<script setup lang="ts">
// The occupancy of a prefix and its next free value (REQ-021-9; maquette E2, panneau
// Occupation): the used values out of the total (whole numbers of any size, IPv6 included),
// in percent beside the title, and the next free address (a network) or prefix (a
// container). Three states.
import { reads, type components } from "@kartenn/sdk";
import { ErrorState, Skeleton } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import { useTime } from "../../time/index.ts";
import { problemOf } from "../data.ts";
import AttributeList from "../parts/AttributeList.vue";
import ViewPanel from "../parts/ViewPanel.vue";

type Occupancy = components["schemas"]["Occupancy"];
type NextFree = components["schemas"]["NextFree"];

const props = defineProps<{ id: string }>();
const { t, locale } = useI18n();
const time = useTime();
const dated = () => ({ validAt: time.value.validAt, asOf: time.value.asOf });

const occupancy = useQuery(() =>
  reads.sub<Occupancy>("prefix", props.id, "occupancy", null, dated()),
);
const free = useQuery(() => reads.sub<NextFree>("prefix", props.id, "next-free", null, dated()));

const whole = (value: string) => {
  try {
    return BigInt(value);
  } catch {
    return 0n;
  }
};
const number = (value: string) => new Intl.NumberFormat(locale.value).format(whole(value));
/** The share used, in whole percent; under one percent but not none, « < 1 % ». */
const share = computed(() => {
  const data = occupancy.data.value?.data;
  if (!data) return undefined;
  const total = whole(data.total);
  const used = whole(data.used);
  if (total === 0n) return undefined;
  const percent = (used * 100n) / total;
  return percent === 0n && used > 0n ? "< 1 %" : `${percent} %`;
});
const items = computed(() => {
  const data = occupancy.data.value?.data;
  if (!data) return [];
  return [
    {
      key: "used",
      label: t("views.record.used"),
      value: t("views.record.usedOf", { used: number(data.used), total: number(data.total) }),
    },
    {
      key: "free",
      label: t("views.record.nextFree"),
      value: free.isPending.value
        ? "…"
        : (free.data.value?.data.value ?? t("views.record.noneFree")),
      mono: !!free.data.value?.data.value,
    },
  ];
});
</script>

<template>
  <ViewPanel :title="t('views.record.occupancy')" :detail="share">
    <ErrorState
      v-if="occupancy.isError.value"
      :title="t('views.record.occupancyError')"
      :problem="problemOf(occupancy.error.value)"
    />
    <Skeleton v-else-if="occupancy.isPending.value" variant="text" :rows="2" />
    <AttributeList v-else :items="items" />
  </ViewPanel>
</template>
