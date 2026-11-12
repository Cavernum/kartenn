<script setup lang="ts">
// The explanation of a resolved property (REQ-021-9, REQ-003-14, 15; maquette E2): where its
// value comes from — set on the object itself, inherited from an origin through a chain of
// objects walked up to it, or inferred by a rule — read when its popover opens, at the
// dates of the page. A value a module derived (`module:<module>#<motif>`, ADR 0026,
// amendment 14) is said so, not as one typed (task 021-13d). Its three states: a few lines
// of skeleton, the explanation, the error with its codes.
import { reads, type components } from "@kartenn/sdk";
import { CodeBadge, Skeleton } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import { useTime } from "../../time/index.ts";
import ProblemState from "../../problems/ProblemState.vue";
import ObjectName from "./ObjectName.vue";
import { provenanceOf } from "./provenance.ts";

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
/** The provenance, when a module derived the value: said in its place of « direct ». */
const derived = computed(() => {
  const read = provenanceOf(facts.value?.provenance);
  return read?.kind === "label" || read?.kind === "module" ? read : null;
});
</script>

<template>
  <div class="k-explanation">
    <Skeleton v-if="explanation.isPending.value" variant="text" :rows="3" />
    <ProblemState
      v-else-if="explanation.isError.value"
      :title="t('views.record.explanationError')"
      :error="explanation.error.value"
    />
    <template v-else-if="facts">
      <p v-if="facts.masked">{{ t("views.record.explanationMasked") }}</p>
      <template v-else>
        <p v-if="origin !== null && !direct">
          {{ t("views.record.inheritedFrom") }}
          <ObjectName :id="origin.id" :kind="origin.kind" :href="origin.href" with-kind />
        </p>
        <p v-if="derived?.kind === 'label'">
          {{ t("views.record.fromLabel", { module: derived.module }) }}
          <CodeBadge :code="derived.key" />
        </p>
        <p v-else-if="derived">
          {{ t("views.record.byModule", { module: derived.module }) }}
          <CodeBadge :code="derived.motif" />
        </p>
        <p v-else-if="origin === null && facts.provenance">
          {{ t("views.record.inferred") }} <CodeBadge :code="facts.provenance" />
        </p>
        <p v-else-if="origin === null">{{ t("views.record.unset") }}</p>
        <p v-else-if="direct">{{ t("views.record.direct") }}</p>
      </template>
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
