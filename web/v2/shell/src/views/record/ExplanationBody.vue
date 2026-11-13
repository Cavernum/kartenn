<script setup lang="ts">
// The explanation of a resolved property (REQ-021-9, REQ-003-14, 15; maquette E2): where its
// value comes from — set on the object itself, inherited from an origin through a chain of
// objects walked up to it, inherited from an object the caller may not read, the default
// of a source or derived by a module — read when its popover opens, at the dates of the
// page. Classed as the column « Source » classes it (`explanationOriginOf`, task
// 021-11p4): the first sentence says what the cell says, in its words (`ValueSource`); an
// inherited value then says its provenance too when a source or a module gave it (task
// 021-13d), which the cell leaves to the explanation. Its three states: a few lines of
// skeleton, the explanation, the error with its codes.
import { reads, type components } from "@kartenn/sdk";
import { Skeleton } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import { useTime } from "../../time/index.ts";
import ProblemState from "../../problems/ProblemState.vue";
import ObjectName from "./ObjectName.vue";
import { derivedOf, explanationOriginOf } from "./provenance.ts";
import ValueSource from "./ValueSource.vue";

type Explanation = components["schemas"]["Explanation"];
/** A node of the chain, as the server writes it: its kind, identifier and URL. */
type ChainNode = components["schemas"]["NodeRef"];

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
const origin = computed(() => (facts.value ? explanationOriginOf(facts.value, props.id) : null));
const chain = computed<ChainNode[]>(() =>
  (facts.value?.chain ?? []).filter((node): node is ChainNode => node !== null),
);
/** Inherited (named or not): the provenance a source or a module gave, said after it. */
const inheritedDerived = computed(() =>
  origin.value?.kind === "inherited" || origin.value?.kind === "unreadable"
    ? derivedOf(facts.value?.provenance)
    : null,
);
</script>

<template>
  <div class="k-explanation">
    <Skeleton v-if="explanation.isPending.value" variant="text" :rows="3" />
    <ProblemState
      v-else-if="explanation.isError.value"
      :title="t('views.record.explanationError')"
      :error="explanation.error.value"
    />
    <template v-else-if="facts && origin">
      <p v-if="facts.masked">{{ t("views.record.explanationMasked") }}</p>
      <template v-else>
        <p v-if="origin.kind === 'unset'">{{ t("views.record.unset") }}</p>
        <p v-else-if="origin.kind === 'direct'">{{ t("views.record.direct") }}</p>
        <p v-else-if="origin.kind === 'inherited'">
          {{ t("views.record.inheritedFrom") }}
          <ObjectName
            :id="origin.origin.id"
            :kind="origin.origin.kind"
            :href="origin.origin.href"
            with-kind
          />
        </p>
        <p v-else><ValueSource :origin="origin" /></p>
        <p v-if="inheritedDerived">
          <ValueSource :origin="{ kind: 'derived', provenance: inheritedDerived }" />
        </p>
      </template>
      <template
        v-if="chain.length > 1 && (origin.kind === 'inherited' || origin.kind === 'unreadable')"
      >
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
