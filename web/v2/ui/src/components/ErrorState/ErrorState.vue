<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The error state of a data view (design « Inventaire », line 25; REQ-020-22, 23): a
// sentence in plain language (the host's), then the code of the problem (its `type`,
// `urn:kartenn:problem:…`), the code of each violation beside its message, and the
// request identifier (`Request-Id`), each copyable (`CodeBadge`); « Réessayer » when the
// host can retry. `problem` takes an `ApiProblem` of the SDK as it is (its `type`,
// `requestId`, `violations`), the library knowing nothing of the SDK.
// The sentence is said: its `alert` region is there, its text hidden (out of the tree),
// shown a frame later, which the region announces (as `InlineAlert`); no shift meanwhile.
import { onBeforeUnmount, onMounted, ref } from "vue";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import Button from "../Button/Button.vue";
import Icon from "../Icon/Icon.vue";
import StatusBadge from "../StatusBadge/StatusBadge.vue";
import type { ProblemFacts } from "./problem.ts";

const props = withDefaults(
  defineProps<{
    title: string;
    problem?: ProblemFacts | undefined;
    retryLabel?: string | undefined;
  }>(),
  { problem: undefined, retryLabel: undefined },
);
const emit = defineEmits<{ retry: [] }>();

const said = ref(false);
let frame = 0;
onMounted(() => {
  frame = requestAnimationFrame(() => (said.value = true));
});
onBeforeUnmount(() => cancelAnimationFrame(frame));
</script>

<template>
  <div class="k-error-state">
    <Icon name="circle-x" :size="20" class="k-error-state__icon" />
    <p class="k-error-state__title" role="alert" :data-state="said ? undefined : 'waiting'">
      <span class="k-error-state__sentence">{{ props.title }}</span>
    </p>
    <p v-if="$slots.default" class="k-error-state__text"><slot /></p>
    <ul v-if="props.problem?.violations?.length" class="k-error-state__violations">
      <li
        v-for="(violation, i) in props.problem.violations"
        :key="i"
        class="k-error-state__violation"
      >
        <StatusBadge :state="violation.severity === 'warning' ? 'warn' : 'ko'" />
        <span>{{ violation.detail }}</span>
        <CodeBadge :code="violation.rule" />
      </li>
    </ul>
    <div v-if="props.problem" class="k-error-state__codes">
      <!-- `about:blank` says no type (RFC 9457 §4.2.1): nothing to pass on. -->
      <CodeBadge
        v-if="props.problem.type !== 'about:blank'"
        :code="props.problem.type"
        label="type"
      />
      <CodeBadge
        v-if="props.problem.requestId"
        :code="props.problem.requestId"
        label="Request-Id"
      />
    </div>
    <Button v-if="props.retryLabel" icon="refresh-cw" @click="emit('retry')">{{
      props.retryLabel
    }}</Button>
  </div>
</template>

<style>
@layer components {
  .k-error-state {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-2);
    justify-items: center;
    padding: var(--k-space-8) var(--k-space-4);
    text-align: center;
  }

  .k-error-state__icon {
    color: var(--k-state-ko-fg);
  }

  .k-error-state__title {
    margin: 0;
    font-weight: var(--k-weight-medium);
  }

  /* Hidden until its region is in the tree, its place kept. */
  .k-error-state__title[data-state="waiting"] .k-error-state__sentence {
    visibility: hidden;
  }

  .k-error-state__text {
    max-inline-size: var(--k-measure-prose);
    margin: 0;
    color: var(--k-text-muted);
  }

  /* A list read down its start edge, the block itself centred. */
  .k-error-state__violations {
    display: grid;
    gap: var(--k-space-1);
    max-inline-size: 100%;
    padding: 0;
    margin: 0;
    text-align: start;
    list-style: none;
  }

  /* The message, then its code beside it; the code goes under it when the room lacks. */
  .k-error-state__violation {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
  }

  /* What to pass on (the type of the problem, the request), apart from what went wrong. */
  .k-error-state__codes {
    margin-block-start: var(--k-space-2);
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    justify-content: center;
    /* A long code (a problem type) stays in a narrow column: the badge truncates. */
    max-inline-size: 100%;
  }
}
</style>
