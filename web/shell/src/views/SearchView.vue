<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { read, withQuery, type Listing } from "../data/api.ts";
import { nameOf, pageOf } from "../data/collections.ts";
import { remember } from "../data/recent.ts";
import { useTime } from "../time/index.ts";

// Global search by value (design « Écrans et routes du client »): an address or a prefix,
// `GET /v1/addresses?value=…` and `/v1/prefixes?value=…`, in every domain; a single
// answer opens its record.

type Item = Record<string, unknown> & { domain?: { slug?: string | null } };

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const time = useTime();

const term = computed(() => (typeof route.query.q === "string" ? route.query.q.trim() : ""));
const found = shallowRef<{ kind: string; item: Item }[] | null>(null);

watch(
  [term, () => time.value.validAt, () => time.value.asOf],
  async () => {
    found.value = null;
    if (!term.value) {
      found.value = [];
      return;
    }
    remember(term.value);
    const params = { value: term.value, valid_at: time.value.validAt, as_of: time.value.asOf };
    const [addresses, prefixes] = await Promise.all([
      read<Listing<Item>>(withQuery("/v1/addresses", params)),
      read<Listing<Item>>(withQuery("/v1/prefixes", params)),
    ]);
    const all = [
      ...(addresses.ok ? addresses.body.items.map((item) => ({ kind: "address", item })) : []),
      ...(prefixes.ok ? prefixes.body.items.map((item) => ({ kind: "prefix", item })) : []),
    ];
    const only = all.length === 1 ? pageOf((all[0]!.item.links as { self?: string }).self) : null;
    if (only) {
      void router.replace({ path: only, query: { ...route.query, q: undefined } });
      return;
    }
    found.value = all;
  },
  { immediate: true },
);
</script>

<template>
  <h1>{{ t("search.title", { term }) }}</h1>
  <p v-if="found === null" class="k-muted">{{ t("app.loading") }}</p>
  <p v-else-if="found.length === 0" class="k-muted">{{ t("search.none") }}</p>
  <ul v-else>
    <li v-for="{ kind, item } in found" :key="String(item.id)">
      <RouterLink :to="pageOf((item.links as { self?: string }).self) ?? '/'">
        {{ t(`object.types.${kind}`) }} {{ nameOf(item) }}
      </RouterLink>
      <span class="k-muted"> — {{ item.domain?.slug ?? "" }}</span>
    </li>
  </ul>
</template>
