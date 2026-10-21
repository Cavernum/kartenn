<script setup lang="ts" generic="T extends Record<string, unknown>">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import type { Pager } from "../data/api.ts";
import { problemText } from "../i18n/index.ts";

// A short list of the administration (roles, bindings, accounts…): a plain table, its
// pages read on demand, an `actions` slot per row.

const props = defineProps<{
  pager: Pager<T>;
  columns: readonly { key: string; label: string }[];
  caption: string;
  cell: (row: T, key: string) => string;
  /** The page of a row's first cell, else `null`. */
  href?: (row: T) => string | null;
}>();
defineSlots<{ actions?(props: { row: T }): unknown }>();

const i18n = useI18n();
const { t } = i18n;
onMounted(() => void props.pager.more());
</script>

<template>
  <p v-if="pager.problem.value" role="alert" class="k-error">
    {{ problemText(i18n, pager.problem.value) }}
  </p>
  <table class="k-table">
    <caption class="k-visually-hidden">
      {{
        caption
      }}
    </caption>
    <thead>
      <tr>
        <th v-for="column in columns" :key="column.key" scope="col">{{ column.label }}</th>
        <th v-if="$slots.actions" scope="col">{{ t("admin.actions") }}</th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="(row, index) in pager.items.value" :key="index">
        <td v-for="(column, position) in columns" :key="column.key">
          <RouterLink v-if="position === 0 && href?.(row)" :to="href(row)!">
            {{ cell(row, column.key) }}
          </RouterLink>
          <template v-else>{{ cell(row, column.key) }}</template>
        </td>
        <td v-if="$slots.actions"><slot name="actions" :row="row" /></td>
      </tr>
    </tbody>
  </table>
  <button
    v-if="!pager.done.value && !pager.problem.value"
    type="button"
    :disabled="pager.loading.value"
    @click="pager.more()"
  >
    {{ t("list.more") }}
  </button>
</template>
