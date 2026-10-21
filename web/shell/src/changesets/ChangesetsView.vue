<script setup lang="ts">
import { computed, reactive, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { PAGE_SIZE, Pager, withQuery } from "../data/api.ts";
import DataTable from "../data/DataTable.vue";
import { problemText } from "../i18n/index.ts";
import { useSession } from "../rights/index.ts";
import { changesetCell, STATES, type ChangesetRow } from "./changesets.ts";

// The changesets, newest first (REQ-007-18): filtered by state, author and period in the
// URL; a row opens its record.

const FILTERS = ["state", "author", "from", "to"] as const;
const COLUMNS = ["motive", "state", "kind", "author", "submitted_at"] as const;

const i18n = useI18n();
const { t } = i18n;
const route = useRoute();
const router = useRouter();
const session = useSession();
const me = computed(() => session.value.me?.identity ?? null);
const draft = reactive<Record<string, string>>({});

function query(name: string): string | null {
  const value = route.query[name];
  return typeof value === "string" && value !== "" ? value : null;
}

const first = computed(() =>
  withQuery("/v1/changesets", {
    page_size: String(PAGE_SIZE),
    ...Object.fromEntries(FILTERS.map((name) => [name, query(name)])),
  }),
);
const pager = shallowRef(new Pager<ChangesetRow>(first.value));
watch(
  first,
  (url) => {
    pager.value = new Pager<ChangesetRow>(url);
    for (const name of FILTERS) draft[name] = query(name) ?? "";
  },
  { immediate: true },
);

const columns = computed(() => COLUMNS.map((key) => ({ key, label: t(`changeset.${key}`) })));
const problem = computed(() => {
  const value = pager.value.problem.value;
  return value ? problemText(i18n, value) : null;
});

function apply(): void {
  const next: Record<string, string> = {};
  for (const name of FILTERS) {
    const value = draft[name]?.trim();
    if (value) next[name] = value;
  }
  void router.push({ path: route.path, query: next });
}

function mine(): void {
  if (me.value) draft.author = me.value;
  apply();
}

const cell = (row: ChangesetRow, key: string): string =>
  key === "state"
    ? t(`changeset.states.${row.state}`)
    : changesetCell(row, key, me.value, t("changeset.you"));
const href = (row: ChangesetRow): string => `/changesets/${row.id}`;
</script>

<template>
  <h1>{{ t("nav.changesets") }}</h1>
  <p>
    <RouterLink to="/proposals">{{ t("changeset.proposals") }}</RouterLink>
  </p>
  <form class="k-filters" role="search" :aria-label="t('list.filters')" @submit.prevent="apply">
    <label>
      {{ t("changeset.state") }}
      <select v-model="draft.state" name="state">
        <option value="">{{ t("changeset.anyState") }}</option>
        <option v-for="state in STATES" :key="state" :value="state">
          {{ t(`changeset.states.${state}`) }}
        </option>
      </select>
    </label>
    <label>
      {{ t("changeset.author") }}
      <input v-model="draft.author" name="author" />
    </label>
    <label>
      {{ t("changeset.from") }}
      <input v-model="draft.from" name="from" placeholder="2026-01-01T00:00:00Z" />
    </label>
    <label>
      {{ t("changeset.to") }}
      <input v-model="draft.to" name="to" placeholder="2026-12-31T23:59:59Z" />
    </label>
    <button type="submit">{{ t("list.apply") }}</button>
    <button v-if="me" type="button" @click="mine">{{ t("changeset.mine") }}</button>
  </form>
  <p v-if="problem" role="alert" class="k-error">{{ problem }}</p>
  <p class="k-muted" aria-live="polite">
    {{
      pager.done.value
        ? t("list.count", { n: pager.items.value.length })
        : t("list.loaded", { n: pager.items.value.length })
    }}
  </p>
  <DataTable
    :rows="pager.items.value"
    :columns="columns"
    :caption="t('nav.changesets')"
    :done="pager.done.value"
    :loading="pager.loading.value"
    :href="href"
    :cell="cell"
    @more="pager.more()"
  />
</template>
