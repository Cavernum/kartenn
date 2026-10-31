<script setup lang="ts">
// The screen of a route of the frame whose view is not written yet (tasks 11 and after): its
// heading, from the last crumb of its breadcrumb, and a sentence. Replaced route by route.
import { EmptyState } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { crumbsOf } from "./breadcrumb.ts";

const props = defineProps<{ missing?: boolean }>();
const { t } = useI18n();
const route = useRoute();
const title = computed(() => crumbsOf(route, t).at(-1)?.label ?? t("app.name"));
</script>

<template>
  <h1 class="k-pending__title">{{ title }}</h1>
  <EmptyState :title="props.missing ? t('app.notFoundTitle') : t('app.pendingTitle')">
    {{ props.missing ? t("app.notFoundText") : t("app.pending") }}
  </EmptyState>
</template>

<style>
@layer components {
  .k-pending__title {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
    font-weight: var(--k-weight-semibold);
  }
}
</style>
