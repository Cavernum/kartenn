<script setup lang="ts">
// The gaps of one object above its properties (task 021-11p4, REQ-021-9, REQ-021-20;
// maquette E2, « Une propriété attendue manque »): its open deviations from the values a
// source expects, read by `GET /v1/property-gaps?object=<id>` (Q-021-23), each composed
// from its members as the list of the gaps does (`anomalies/GapList.vue`, 021/A36): the
// property by its label and key, the value resolved (or none) and the one expected, the
// source and its revision, the code of the rule. Nothing shows while it loads, nor when
// the object has none; its error is an inline alert with its codes. The gaps are those of
// the present: away from it (`valid_at`, `as_of`), the notice is not read. The code of a
// gap goes to the line when the sentence is long: never under the copiable key of the
// property, a target too close to it (WCAG 2.5.8).
import { reads } from "@kartenn/sdk";
import { Button, Cluster, CodeBadge, InlineAlert, KeyLabel, NoValue, Stack } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useInfiniteQuery } from "../../data/query.ts";
import NextPageAlert from "../../problems/NextPageAlert.vue";
import { useProblems } from "../../problems/index.ts";
import { useTime } from "../../time/index.ts";
import { rowsOf, useDeclarationsOf, useKeyLabel } from "../data.ts";
import { gapDetail as detailOf, noValue as empty, type Gap } from "../anomalies/gap.ts";
import PropertyValue from "./PropertyValue.vue";

const props = defineProps<{ id: string }>();
const { t } = useI18n();
const time = useTime();
const keyLabel = useKeyLabel();
const { lines } = useProblems();

const list = useInfiniteQuery(() => ({
  ...reads.list("/v1/property-gaps", { object: props.id }, null),
  enabled: time.value.present,
}));
const present = computed(() => time.value.present);
const gaps = computed(() => (present.value ? (rowsOf(list.data.value) as Gap[]) : []));
const failed = computed(
  () => present.value && list.isError.value && !list.isFetchNextPageError.value,
);
const nextFailed = computed(() => present.value && list.isFetchNextPageError.value);
// The declaration of each property gapped, read by its key: an heir that does not carry
// it may lack an inherited one (003), which no read by kind would find.
const declared = useDeclarationsOf(() =>
  gaps.value.flatMap((gap) => (gap.property ? [gap.property] : [])),
);
/** What the value of a gap is shown as: its declared type, and the kind a reference targets. */
const shape = (gap: Gap) => {
  const declaration = declared.of(gap.property);
  const target = declaration?.params?.target;
  return {
    property: gap.property ?? "",
    type: declaration?.type,
    target: typeof target === "string" ? target : undefined,
  };
};
</script>

<template>
  <InlineAlert
    v-if="failed"
    tone="ko"
    :title="t('views.anomalies.gapsError')"
    :items="lines(list.error.value)"
  >
    <Button size="sm" icon="refresh-cw" @click="list.refetch()">{{ t("views.retry") }}</Button>
  </InlineAlert>
  <InlineAlert
    v-else-if="gaps.length > 0"
    tone="warn"
    :title="t('views.record.gapsTitle', { n: gaps.length }, gaps.length)"
  >
    <Stack gap="2">
      <Stack as="ul" gap="1">
        <Cluster v-for="gap in gaps" :key="gap.id" as="li" gap="1">
          <span
            ><KeyLabel
              v-if="gap.property"
              :value="gap.property"
              :label="keyLabel('property', gap.property)"
              show-key
            /><NoValue v-else />{{ t("views.record.gapColon") }}
            <template v-if="detailOf(gap) === 'unevaluated'">{{
              t("views.anomalies.unevaluated")
            }}</template>
            <template v-else-if="detailOf(gap) === 'masked'">{{
              t("views.record.gapMasked")
            }}</template>
            <template v-else>
              <template v-if="empty(gap.resolved)">{{ t("views.record.gapNone") }}</template>
              <PropertyValue v-else v-bind="shape(gap)" :value="gap.resolved" />{{
                t("views.record.gapExpected")
              }}<PropertyValue v-bind="shape(gap)" :value="gap.expected" />
            </template>
            <template v-if="gap.source"
              >{{ t("views.record.gapSource") }}<span class="k-mono">{{ gap.source }}</span
              ><template v-if="gap.revision"
                >{{ ` ${t("views.anomalies.revision")} `
                }}<span class="k-mono">{{ gap.revision }}</span></template
              ></template
            ></span
          >
          <CodeBadge :code="gap.rule" />
        </Cluster>
      </Stack>
      <div v-if="list.hasNextPage.value && !list.isFetchNextPageError.value">
        <Button size="sm" :loading="list.isFetchingNextPage.value" @click="list.fetchNextPage()">{{
          t("views.record.gapsMore")
        }}</Button>
      </div>
    </Stack>
  </InlineAlert>
  <NextPageAlert v-if="nextFailed" :error="list.error.value" :retry="list.fetchNextPage" />
</template>
