<script setup lang="ts">
import { computed, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { read, withQuery, type Listing } from "../data/api.ts";
import { datedSearch, useTime } from "../time/index.ts";
import PrefixTree from "../tree/PrefixTree.vue";

// The landing screen (design « Écrans et routes du client »): the tree of the domain
// chosen in the URL (`?domain=`), the first domain by default.

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const time = useTime();
const select = useId();

const domains = ref<{ id: string; slug: string }[]>([]);
const loaded = ref(false);

watch(
  () => [time.value.validAt, time.value.asOf],
  async () => {
    const answer = await read<Listing<{ id: string; slug: string }>>(
      withQuery(`/v1/domains${datedSearch(time.value)}`, { page_size: "1000" }),
    );
    domains.value = answer.ok ? answer.body.items.map(({ id, slug }) => ({ id, slug })) : [];
    loaded.value = true;
  },
  { immediate: true },
);

const domain = computed(() => {
  const asked = route.query.domain;
  const id = Array.isArray(asked) ? asked[0] : asked;
  return domains.value.find((d) => d.id === id) ?? domains.value[0] ?? null;
});

function choose(event: Event): void {
  const id = (event.target as HTMLSelectElement).value;
  void router.replace({ query: { ...route.query, domain: id } });
}
</script>

<template>
  <h1>{{ t("home.title") }}</h1>
  <p v-if="!loaded" class="k-muted" role="status">{{ t("app.loading") }}</p>
  <p v-else-if="!domain" class="k-muted">{{ t("home.noDomain") }}</p>
  <template v-else>
    <p class="k-field">
      <label :for="select">{{ t("home.domain") }}</label>
      <select :id="select" :value="domain.id" @change="choose">
        <option v-for="d in domains" :key="d.id" :value="d.id">{{ d.slug }}</option>
      </select>
    </p>
    <PrefixTree :domain="domain.id" :label="t('tree.label', { domain: domain.slug })" />
  </template>
</template>
