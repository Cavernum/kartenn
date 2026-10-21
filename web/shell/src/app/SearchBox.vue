<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { recent } from "../data/recent.ts";

// Global search of the header: an address or a prefix, in every domain.

const { t } = useI18n();
const router = useRouter();
const term = ref("");
const suggestions = ref<string[]>([]);

function refresh(): void {
  suggestions.value = recent();
}

function search(): void {
  const value = term.value.trim();
  if (value) void router.push({ name: "search", query: { q: value } });
}
</script>

<template>
  <form class="k-search" role="search" :aria-label="t('search.label')" @submit.prevent="search">
    <input
      v-model="term"
      type="search"
      name="q"
      list="k-search-recent"
      :aria-label="t('search.label')"
      :placeholder="t('search.placeholder')"
      @focus="refresh"
    />
    <datalist id="k-search-recent">
      <option v-for="value in suggestions" :key="value" :value="value"></option>
    </datalist>
    <button type="submit">{{ t("search.submit") }}</button>
  </form>
</template>
