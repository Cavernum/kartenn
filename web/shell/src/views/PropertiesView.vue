<script setup lang="ts">
import { computed, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { PAGE_SIZE, Pager, withQuery } from "../data/api.ts";
import { pageOf } from "../data/collections.ts";
import { problemText } from "../i18n/index.ts";
import { useTime } from "../time/index.ts";

// The declarations of properties and the gaps of their obligations (REQ-007-17, 003).

interface Declaration {
  id: string;
  key: string;
  type: string;
  cardinality: string;
  inherit: string | null;
  carriers: string[];
  sensitive: boolean;
}
interface Gap {
  id: string;
  message: string;
  property: string | null;
  object: { id: string; href: string | null };
}

const i18n = useI18n();
const { t } = i18n;
const time = useTime();

const firstDeclarations = computed(() =>
  withQuery("/v1/property-declarations", {
    page_size: String(PAGE_SIZE),
    valid_at: time.value.validAt,
    as_of: time.value.asOf,
  }),
);
const declarations = shallowRef(new Pager<Declaration>(firstDeclarations.value));
const gaps = shallowRef(new Pager<Gap>("/v1/property-gaps"));
watch(
  firstDeclarations,
  (url) => {
    declarations.value = new Pager<Declaration>(url);
    void declarations.value.more();
  },
  { immediate: true },
);
void gaps.value.more();
</script>

<template>
  <h1>{{ t("nav.properties") }}</h1>
  <section aria-labelledby="k-declarations">
    <h2 id="k-declarations">{{ t("properties.declarations") }}</h2>
    <p v-if="declarations.problem.value" role="alert" class="k-error">
      {{ problemText(i18n, declarations.problem.value) }}
    </p>
    <table v-else class="k-table">
      <thead>
        <tr>
          <th scope="col">{{ t("properties.key") }}</th>
          <th scope="col">{{ t("properties.type") }}</th>
          <th scope="col">{{ t("properties.cardinality") }}</th>
          <th scope="col">{{ t("properties.inherit") }}</th>
          <th scope="col">{{ t("properties.carriers") }}</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="declaration in declarations.items.value" :key="declaration.id">
          <td class="k-mono">{{ declaration.key }}</td>
          <td>{{ declaration.type }}</td>
          <td>{{ declaration.cardinality }}</td>
          <td>{{ declaration.inherit ?? "—" }}</td>
          <td>{{ declaration.carriers.join(", ") }}</td>
        </tr>
      </tbody>
    </table>
    <button
      v-if="!declarations.done.value && !declarations.problem.value"
      type="button"
      @click="declarations.more()"
    >
      {{ t("list.more") }}
    </button>
  </section>
  <section aria-labelledby="k-gaps">
    <h2 id="k-gaps">{{ t("properties.gaps") }}</h2>
    <p v-if="gaps.problem.value" role="alert" class="k-error">
      {{ problemText(i18n, gaps.problem.value) }}
    </p>
    <p v-else-if="gaps.done.value && gaps.items.value.length === 0" class="k-muted">
      {{ t("properties.noGaps") }}
    </p>
    <ul v-else>
      <li v-for="gap in gaps.items.value" :key="gap.id">
        <span class="k-mono">{{ gap.property ?? "—" }}</span> — {{ gap.message }}
        <RouterLink v-if="pageOf(gap.object.href)" :to="pageOf(gap.object.href)!">
          {{ t("anomalies.open") }}
        </RouterLink>
      </li>
    </ul>
    <button v-if="!gaps.done.value && !gaps.problem.value" type="button" @click="gaps.more()">
      {{ t("list.more") }}
    </button>
  </section>
</template>
