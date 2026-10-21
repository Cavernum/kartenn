<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { PAGE_SIZE, Pager, withQuery } from "../data/api.ts";
import { nameOf } from "../data/collections.ts";
import { problemText } from "../i18n/index.ts";
import { useTime } from "../time/index.ts";

// The versions of an object (bitemporal, REQ-007-17): valid period, assertion, changeset.

interface Version {
  valid_from: string;
  valid_to: string | null;
  asserted_from: string;
  asserted_to: string | null;
  changeset: { id: string } | null;
  object: Record<string, unknown>;
}

const props = defineProps<{ href: string }>();
const i18n = useI18n();
const { t, locale } = i18n;
const time = useTime();

const first = computed(() =>
  withQuery(`${props.href}/history`, {
    page_size: String(PAGE_SIZE),
    valid_at: time.value.validAt,
    as_of: time.value.asOf,
  }),
);
const pager = shallowRef(new Pager<Version>(first.value));
watch(
  first,
  (url) => {
    pager.value = new Pager<Version>(url);
    void pager.value.more();
  },
  { immediate: true },
);

function when(iso: string | null): string {
  if (!iso) return t("record.open");
  return new Intl.DateTimeFormat(locale.value, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(iso),
  );
}
</script>

<template>
  <p v-if="pager.problem.value" role="alert" class="k-error">
    {{ problemText(i18n, pager.problem.value) }}
  </p>
  <table v-else class="k-table k-history">
    <caption class="k-visually-hidden">
      {{
        t("record.history")
      }}
    </caption>
    <thead>
      <tr>
        <th scope="col">{{ t("record.version") }}</th>
        <th scope="col">{{ t("record.validFrom") }}</th>
        <th scope="col">{{ t("record.validTo") }}</th>
        <th scope="col">{{ t("record.assertedFrom") }}</th>
        <th scope="col">{{ t("record.changeset") }}</th>
      </tr>
    </thead>
    <tbody>
      <tr v-for="(version, index) in pager.items.value" :key="index">
        <td>{{ nameOf(version.object) }}</td>
        <td>
          <time :datetime="version.valid_from">{{ when(version.valid_from) }}</time>
        </td>
        <td>
          <time v-if="version.valid_to" :datetime="version.valid_to">{{
            when(version.valid_to)
          }}</time>
          <template v-else>{{ when(null) }}</template>
        </td>
        <td>
          <time :datetime="version.asserted_from">{{ when(version.asserted_from) }}</time>
        </td>
        <td class="k-mono">{{ version.changeset?.id ?? "—" }}</td>
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
