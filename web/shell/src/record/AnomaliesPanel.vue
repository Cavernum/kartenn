<script setup lang="ts">
import { computed, reactive, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Pager, withQuery, write, type Problem } from "../data/api.ts";
import { pageOf } from "../data/collections.ts";
import { problemText } from "../i18n/index.ts";
import { useWrites } from "../time/index.ts";

// Open anomalies and their acknowledgement with a motive (REQ-007-17, REQ-002-36): of a
// domain or an entity, those of one object when `object` is given.

interface Anomaly {
  id: string;
  rule: string;
  message: string;
  opened_at: string;
  object: { id: string; kind: string | null; href: string | null };
  links: { acknowledgement: string };
}

const props = defineProps<{
  filters: Record<string, string | null | undefined>;
  object?: string;
}>();
const i18n = useI18n();
const { t, locale } = i18n;
const writes = useWrites();

const first = computed(() => withQuery("/v1/anomalies", props.filters));
const pager = shallowRef(new Pager<Anomaly>(first.value));
watch(
  first,
  async (url) => {
    pager.value = new Pager<Anomaly>(url);
    // The list has no filter by object: read it whole for a record (one domain's).
    while (!pager.value.done.value && pager.value.items.value.length < 10_000) {
      await pager.value.more();
      if (!props.object) break;
    }
  },
  { immediate: true },
);

const shown = computed(() =>
  props.object
    ? pager.value.items.value.filter((a) => a.object.id === props.object)
    : pager.value.items.value,
);
const motives = reactive<Record<string, string>>({});
const outcome = reactive<Record<string, Problem | "done">>({});

async function acknowledge(anomaly: Anomaly): Promise<void> {
  const answer = await write(anomaly.links.acknowledgement, { motive: motives[anomaly.id] ?? "" });
  outcome[anomaly.id] = answer.ok ? "done" : answer.problem;
}

function when(iso: string): string {
  return new Intl.DateTimeFormat(locale.value, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(iso),
  );
}
</script>

<template>
  <p v-if="pager.problem.value" role="alert" class="k-error">
    {{ problemText(i18n, pager.problem.value) }}
  </p>
  <p v-else-if="pager.done.value && shown.length === 0" class="k-muted">
    {{ t("anomalies.none") }}
  </p>
  <ul v-else class="k-anomalies">
    <li v-for="anomaly in shown" :key="anomaly.id">
      <p>
        <strong>{{ anomaly.rule }}</strong>
        — {{ anomaly.message }}
        <span class="k-muted">
          (<time :datetime="anomaly.opened_at">{{ when(anomaly.opened_at) }}</time
          >)
        </span>
        <RouterLink
          v-if="!object && pageOf(anomaly.object.href)"
          :to="pageOf(anomaly.object.href)!"
        >
          {{ t("anomalies.open") }}
        </RouterLink>
      </p>
      <p v-if="outcome[anomaly.id] === 'done'" role="status">{{ t("anomalies.acknowledged") }}</p>
      <form
        v-else
        class="k-inline-form"
        :aria-label="t('anomalies.acknowledgeOf', { rule: anomaly.rule })"
        @submit.prevent="acknowledge(anomaly)"
      >
        <label>
          {{ t("anomalies.motive") }}
          <input v-model="motives[anomaly.id]" required :disabled="!writes.quickEdit" />
        </label>
        <button type="submit" :disabled="!writes.quickEdit">
          {{ t("anomalies.acknowledge") }}
        </button>
        <p v-if="outcome[anomaly.id]" role="alert" class="k-error">
          {{ problemText(i18n, outcome[anomaly.id] as Problem) }}
        </p>
      </form>
    </li>
  </ul>
  <button
    v-if="!object && !pager.done.value && !pager.problem.value"
    type="button"
    :disabled="pager.loading.value"
    @click="pager.more()"
  >
    {{ t("list.more") }}
  </button>
</template>
