<script setup lang="ts">
// A next page of a list refused (REQ-021-20): the rows read stay, and above them an inline
// alert says that the rest could not be read, why (the sentence of the problem's type),
// with its codes copyable, and « Réessayer » for that page alone. A refused cursor is not
// this: the list starts again from its first page (`useFilteredList`, `restarted`).
import { Button, InlineAlert } from "@kartenn/ui";
import { useI18n } from "vue-i18n";
import { useProblems } from "./index.ts";

const props = defineProps<{ error: unknown; retry: () => unknown }>();
const { t } = useI18n();
const { lines } = useProblems();
</script>

<template>
  <InlineAlert tone="ko" :title="t('problem.nextPage')" :items="lines(props.error)">
    <Button size="sm" icon="refresh-cw" @click="props.retry()">{{ t("views.retry") }}</Button>
  </InlineAlert>
</template>
