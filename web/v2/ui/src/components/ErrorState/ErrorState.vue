<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The error state of a data view (design « Inventaire », line 25; REQ-020-22, 23):
// `role="alert"`, a sentence, the problem type and the request identifier, both copyable
// (`CodeBadge`), and « Réessayer » when the host can retry.
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import Button from "../Button/Button.vue";
import Icon from "../Icon/Icon.vue";

const props = withDefaults(
  defineProps<{
    title: string;
    /** The problem type (`urn:kartenn:problem:…`) or code. */
    code?: string | undefined;
    requestId?: string | undefined;
    retryLabel?: string | undefined;
  }>(),
  { code: undefined, requestId: undefined, retryLabel: undefined },
);
const emit = defineEmits<{ retry: [] }>();
</script>

<template>
  <div class="k-error-state">
    <Icon name="circle-x" :size="20" class="k-error-state__icon" />
    <p class="k-error-state__title" role="alert">{{ props.title }}</p>
    <p v-if="$slots.default" class="k-error-state__text"><slot /></p>
    <div v-if="props.code || props.requestId" class="k-error-state__codes">
      <CodeBadge v-if="props.code" :code="props.code" />
      <CodeBadge v-if="props.requestId" :code="props.requestId" label="Request-Id" />
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

  .k-error-state__text {
    max-inline-size: var(--k-measure-prose);
    margin: 0;
    color: var(--k-text-muted);
  }

  .k-error-state__codes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    justify-content: center;
  }
}
</style>
