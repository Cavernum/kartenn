<script setup lang="ts">
// The explanation of a resolved property (REQ-021-9, REQ-003-14, 15; maquette E2): where its
// value comes from — set on the object itself, inherited from an origin through a chain of
// objects walked up to it, or inferred by a rule — read when its popover opens, at the
// dates of the page. Its three states: a few lines of skeleton, the explanation, the error
// with its codes.
import { reads, type components } from "@kartenn/sdk";
import { CodeBadge, ErrorState, Skeleton } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import { useTime } from "../../time/index.ts";
import { problemOf } from "../data.ts";
import ObjectName from "./ObjectName.vue";

type Explanation = components["schemas"]["Explanation"];
/** A node of the chain, as the server writes it: its kind, identifier and URL. */
interface ChainNode {
  id: string;
  kind?: string | null;
  href?: string | null;
}

const props = defineProps<{ kind: string; id: string; property: string }>();
const { t } = useI18n();
const time = useTime();

const explanation = useQuery(() =>
  reads.sub<Explanation>(
    props.kind,
    props.id,
    `properties/${encodeURIComponent(props.property)}/explanation`,
    null,
    { validAt: time.value.validAt, asOf: time.value.asOf },
  ),
);
const facts = computed(() => explanation.data.value?.data);
const origin = computed(() => (facts.value?.origin ?? null) as ChainNode | null);
const chain = computed<ChainNode[]>(
  () => (facts.value?.chain ?? []).filter((node) => node !== null) as unknown as ChainNode[],
);
const direct = computed(() => origin.value !== null && origin.value.id === props.id);
</script>

<template>
  <div class="k-explanation">
    <Skeleton v-if="explanation.isPending.value" variant="text" :rows="3" />
    <ErrorState
      v-else-if="explanation.isError.value"
      :title="t('views.record.explanationError')"
      :problem="problemOf(explanation.error.value)"
    />
    <template v-else-if="facts">
      <p v-if="facts.masked">{{ t("views.record.explanationMasked") }}</p>
      <p v-else-if="origin === null && facts.provenance">
        {{ t("views.record.inferred") }} <CodeBadge :code="facts.provenance" />
      </p>
      <p v-else-if="origin === null">{{ t("views.record.unset") }}</p>
      <p v-else-if="direct">{{ t("views.record.direct") }}</p>
      <p v-else>
        {{ t("views.record.inheritedFrom") }}
        <ObjectName :id="origin.id" :kind="origin.kind" :href="origin.href" with-kind />
      </p>
      <template v-if="chain.length > 1 && !direct">
        <p>{{ t("views.record.chain") }}</p>
        <ol class="k-explanation__chain">
          <li v-for="node in chain" :key="node.id">
            <ObjectName :id="node.id" :kind="node.kind" :href="node.href" with-kind />
          </li>
        </ol>
      </template>
    </template>
  </div>
</template>

<style>
@layer components {
  .k-explanation {
    display: grid;
    gap: var(--k-space-2);
  }

  .k-explanation__chain {
    display: grid;
    gap: var(--k-space-1);
    padding-inline-start: var(--k-space-4);
  }
}
</style>
