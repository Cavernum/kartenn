<script setup lang="ts">
// The error state of a view of the shell (REQ-021-20): what could not be done (`title`, the
// view's), why in plain words (the translation of the problem's type), the default slot,
// then « transmettez ces codes » when there are codes — all said by the `alert` of
// `ErrorState` (020) — and the codes of the problem, of its violations and the
// `Request-Id`, copyable. « Réessayer » when the view can read again.
// The why is taken once per error: the dictionary arriving later never rewrites a region
// already said (its labels and the catalogs' are the same sentences, 021-13).
import { ErrorState } from "@kartenn/ui";
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { hasCodes, problemOf, useProblems } from "./index.ts";

const props = withDefaults(
  defineProps<{
    title: string;
    error: unknown;
    /** Reads again; without it, no « Réessayer ». */
    retry?: (() => unknown) | undefined;
    /** `false` before any session (the login): the catalogs alone, no dictionary read. */
    dictionary?: boolean;
  }>(),
  // A boolean prop left out would be `false`: the dictionary is read unless told not to.
  { retry: undefined, dictionary: true },
);
const { t } = useI18n();
const { sentence } = useProblems({ dictionary: props.dictionary });
const why = ref("");
watch(
  () => props.error,
  (error) => (why.value = sentence(error)),
  { immediate: true },
);
</script>

<template>
  <ErrorState
    :title="props.title"
    :problem="problemOf(props.error)"
    :retry-label="props.retry ? t('views.retry') : undefined"
    @retry="props.retry?.()"
  >
    {{ why }}
    <slot />
    <template v-if="hasCodes(props.error)">{{ t("problem.passOn") }}</template>
  </ErrorState>
</template>
