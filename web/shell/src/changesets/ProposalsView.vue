<script setup lang="ts">
import { computed, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { PAGE_SIZE, Pager, withQuery } from "../data/api.ts";
import DataTable from "../data/DataTable.vue";
import { problemText } from "../i18n/index.ts";
import { useSession } from "../rights/index.ts";
import { changesetCell, type ProposalRow } from "./changesets.ts";

// The pending proposals the caller may validate, oldest first (REQ-007-18, REQ-004-43),
// with the write permissions their author lacks.

const COLUMNS = ["motive", "author", "required", "submitted_at"] as const;

const i18n = useI18n();
const { t } = i18n;
const session = useSession();
const me = computed(() => session.value.me?.identity ?? null);
const pager = shallowRef(
  new Pager<ProposalRow>(withQuery("/v1/proposals", { page_size: String(PAGE_SIZE) })),
);

const columns = computed(() => COLUMNS.map((key) => ({ key, label: t(`changeset.${key}`) })));
const problem = computed(() => {
  const value = pager.value.problem.value;
  return value ? problemText(i18n, value) : null;
});
const cell = (row: ProposalRow, key: string): string =>
  changesetCell(row, key, me.value, t("changeset.you"));
const href = (row: ProposalRow): string => `/changesets/${row.id}`;
</script>

<template>
  <h1>{{ t("changeset.proposals") }}</h1>
  <p v-if="problem" role="alert" class="k-error">{{ problem }}</p>
  <p
    v-else-if="pager.done.value && pager.items.value.length === 0"
    class="k-muted"
    aria-live="polite"
  >
    {{ t("changeset.noProposal") }}
  </p>
  <DataTable
    :rows="pager.items.value"
    :columns="columns"
    :caption="t('changeset.proposals')"
    :done="pager.done.value"
    :loading="pager.loading.value"
    :href="href"
    :cell="cell"
    @more="pager.more()"
  />
</template>
