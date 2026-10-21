<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { pageOf, TYPES, type Collection } from "../data/collections.ts";
import QuickEdit from "../edit/QuickEdit.vue";
import { useWrites } from "../time/index.ts";

// The creation of an object of the core by its collection (REQ-007-25): on 201 the record
// of the created object, on 202 the pending changeset; `?domain=` preselects a domain.

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const writes = useWrites();

const collection = computed(() => String(route.params.collection) as Collection);
const domain = computed(() => {
  const value = route.query.domain;
  return typeof value === "string" ? value : null;
});
const created = ref(false);

function applied(location: string | null): void {
  const page = pageOf(location);
  if (page) void router.push(page);
  else created.value = true;
}
</script>

<template>
  <h1>{{ t("edit.new", { type: t(`object.types.${TYPES[collection]}`) }) }}</h1>
  <p v-if="!writes.quickEdit" class="k-muted">{{ t("edit.off") }}</p>
  <p v-else-if="created" role="status">{{ t("edit.applied") }}</p>
  <QuickEdit
    v-else
    :collection="collection"
    mode="create"
    :domain="domain"
    @applied="applied"
    @cancel="router.push(`/${collection}`)"
  />
</template>
