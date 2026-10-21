<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { Problem } from "../data/api.ts";
import { problemText } from "../i18n/index.ts";
import type { Outcome } from "./admin.ts";

// What became of a write of the administration: applied, held for a decision (a link to
// its changeset), or refused with the problem of the API (REQ-007-20).

defineProps<{ outcome: Outcome | null; problem: Problem | null }>();
const i18n = useI18n();
const { t } = i18n;
</script>

<template>
  <p v-if="problem" role="alert" class="k-error">{{ problemText(i18n, problem) }}</p>
  <p v-else-if="outcome" role="status">
    {{ outcome.kind === "pending" ? t("admin.pending") : t("admin.applied") }}
    <RouterLink v-if="outcome.changeset" :to="`/changesets/${outcome.changeset}`">
      {{ t("admin.changeset") }}
    </RouterLink>
  </p>
</template>
