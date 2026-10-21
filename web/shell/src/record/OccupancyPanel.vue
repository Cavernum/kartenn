<script setup lang="ts">
import { shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { read } from "../data/api.ts";
import { datedSearch, useTime } from "../time/index.ts";

// Occupancy of a prefix and its next free address or prefix (REQ-007-17, REQ-002).

const props = defineProps<{ href: string }>();
const { t, n } = useI18n();
const time = useTime();

const occupancy = shallowRef<{ used: string; total: string } | null>(null);
const free = shallowRef<{ kind: string; value: string | null } | null>(null);

watch(
  [() => props.href, () => time.value.validAt, () => time.value.asOf],
  async () => {
    const dated = datedSearch(time.value);
    const [used, next] = await Promise.all([
      read<{ used: string; total: string }>(`${props.href}/occupancy${dated}`),
      read<{ kind: string; value: string | null }>(`${props.href}/next-free${dated}`),
    ]);
    occupancy.value = used.ok ? used.body : null;
    free.value = next.ok ? next.body : null;
  },
  { immediate: true },
);

function percent(used: string, total: string): string {
  const ratio = Number(used) / Number(total);
  return Number.isFinite(ratio) ? n(ratio, { style: "percent", maximumFractionDigits: 1 }) : "—";
}
</script>

<template>
  <section class="k-occupancy" :aria-label="t('record.occupancy')">
    <h2>{{ t("record.occupancy") }}</h2>
    <p v-if="occupancy">
      {{ t("record.used", { used: occupancy.used, total: occupancy.total }) }}
      ({{ percent(occupancy.used, occupancy.total) }})
    </p>
    <p v-if="free">
      {{ t("record.nextFree") }}
      <span class="k-mono">{{ free.value ?? t("record.full") }}</span>
    </p>
  </section>
</template>
