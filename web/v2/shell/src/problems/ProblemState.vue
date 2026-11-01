<script setup lang="ts">
// The error state of a view of the shell (REQ-021-20): what could not be done (`title`, the
// view's), why in plain words (the translation of the problem's type), then the codes of
// the problem, of its violations and the `Request-Id`, copyable (`ErrorState` of the 020).
// « Réessayer » when the view can read again. The default slot adds a sentence after the why.
import { ErrorState } from "@kartenn/ui";
import { useI18n } from "vue-i18n";
import { problemOf, useProblems } from "./index.ts";

const props = defineProps<{
  title: string;
  error: unknown;
  /** Reads again; without it, no « Réessayer ». */
  retry?: (() => unknown) | undefined;
  /** `false` before any session (the login): the catalogs alone, no dictionary read. */
  dictionary?: boolean;
}>();
const { t } = useI18n();
const { sentence } = useProblems({ dictionary: props.dictionary !== false });
</script>

<template>
  <ErrorState
    :title="props.title"
    :problem="problemOf(props.error)"
    :retry-label="props.retry ? t('views.retry') : undefined"
    @retry="props.retry?.()"
  >
    {{ sentence(props.error) }}
    <slot />
  </ErrorState>
</template>
