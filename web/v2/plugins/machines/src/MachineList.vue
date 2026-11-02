<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The list of the machines (task 18cb; design 021 « Filtres, libellés, codes »): one
// `DataGrid` of `/v1/machines`, read page by page through the shared cache at the dates of
// the context, the next page read as the window nears the end. In the order the server
// gives (by name): the route has no parameter of sort, and a sort of the loaded pages alone
// would lie. Its typed filters are the parameters the route serves (`filterModel` of the
// SDK), one model for the bar and the filters of the columns (020, A7), held in the query of
// the page as the route takes them; the route takes one value of each, a second refused
// before anything is sent. Three states: skeleton rows, an empty list with what to do, an
// error with its codes. Below, the creation of a machine, added to the draft (W10).
import {
  ApiProblem,
  filterModel,
  MAX_PAGES,
  queryClient,
  reads,
  type Entry,
  type Filters,
} from "@kartenn/sdk";
import {
  Button,
  checkFilter,
  DataGrid,
  EmptyState,
  ErrorState,
  FilterPicker,
  filtersFromParams,
  filtersToParams,
  InlineAlert,
  KeyLabel,
  Select,
  Stack,
  TextField,
  type CellRef,
  type Filter,
  type FilterModel,
  type FilterProperty,
  type GridColumn,
  type GridWindow,
  type ListOption,
  type Suggest,
} from "@kartenn/ui";
import { computed, nextTick, onScopeDispose, ref, shallowRef, useTemplateRef, watch } from "vue";
import CitedName from "./CitedName.vue";
import { useList, useNotified } from "./data.ts";
import { arrival, followLink, hrefOf, PAGES } from "./dom.ts";
import DraftForm from "./DraftForm.vue";
import { useHosted, usePresent, useTexts } from "./host.ts";
import { linesOf } from "./problems.ts";
import { KINDS, Names, type Cluster, type Machine } from "./reads.ts";

const PATH = "/v1/machines";
/** The rows left before the end of the window when the next page is read. */
const AHEAD = 20;

const t = useTexts();
const { context, element } = useHosted();
const present = usePresent();
const names = useNotified((changed, live) => new Names(changed, live));

// ------------------------------------------------------------------------------ filters

/** The parameters of the route offered as filters, and the column each one filters. */
const OFFERED = ["name_prefix", "kind", "owner", "site", "host", "cluster"] as const;
const COLUMN_OF: Record<string, string> = {
  name_prefix: "name",
  kind: "kind",
  owner: "owner",
  site: "site",
  host: "host",
};
/** What the contract serves of them (a parameter it dropped is no longer offered). */
const served = new Set(filterModel(PATH).map((field) => field.name));

/** The labels of the objects the suggestions gave, by identifier. */
const labels = new Map<string, string>();

/** The first page of a list for a text, through the cache at the dates of the context. */
async function first<T>(
  path: "/v1/entities" | "/v1/sites" | "/v1/machines" | "/v1/clusters",
  q: string,
) {
  const data = (await queryClient.fetchInfiniteQuery(
    reads.list(path, q ? { q } : {}) as never,
  )) as { pages: Entry<{ items: T[] }>[] };
  return data.pages[0]?.data.items ?? [];
}

/** The suggestions of a collection named by its slug: value its identifier. */
function bySlug(path: "/v1/entities" | "/v1/sites"): Suggest {
  return async (text) =>
    (await first<{ id: string; slug: string }>(path, text.trim())).map((item): ListOption => {
      labels.set(item.id, item.slug);
      return { value: item.id, label: item.slug, mono: true };
    });
}

/** The hosts of a text: the machines by name, then the clusters by slug. */
const hosts =
  (only: "clusters" | "both"): Suggest =>
  async (text) => {
    const q = text.trim();
    const [machines, clusters] = await Promise.all([
      only === "both" ? first<Machine>("/v1/machines", q) : Promise.resolve([]),
      first<Cluster>("/v1/clusters", q),
    ]);
    return [
      ...machines.map((m): ListOption => {
        labels.set(m.id, m.name);
        return { value: m.id, label: m.name, detail: t("move.machine") };
      }),
      ...clusters.map((c): ListOption => {
        labels.set(c.id, c.slug);
        return { value: c.id, label: c.slug, detail: t("move.cluster"), mono: true };
      }),
    ];
  };

/** The name of an object cited by a filter read from the address: a suggestion's, or read. */
function labelOf(type: "core.entity" | "core.site" | "machines.cluster", id: string) {
  const known = labels.get(id);
  if (known) return known;
  const name = names().of(type, id);
  return name.state === "read" ? name.name : undefined;
}
function hostLabel(id: string): string | undefined {
  const known = labels.get(id);
  if (known) return known;
  const machine = names().of("machines.machine", id);
  if (machine.state === "read") return machine.name;
  return machine.state === "failed" ? labelOf("machines.cluster", id) : undefined;
}

const model = computed<FilterModel>(() => {
  const properties: Record<(typeof OFFERED)[number], FilterProperty> = {
    name_prefix: {
      key: "name_prefix",
      label: t("filter.namePrefix"),
      type: "text",
      operators: ["eq"],
    },
    kind: {
      key: "kind",
      label: t("machines.kind"),
      type: "enum",
      operators: ["eq"],
      options: KINDS.map((kind) => ({ value: kind, label: t(`kind.${kind}`) })),
    },
    owner: {
      key: "owner",
      label: t("machines.owner"),
      type: "enum",
      operators: ["eq"],
      suggest: bySlug("/v1/entities"),
      valueLabel: (id) => labelOf("core.entity", id),
    },
    site: {
      key: "site",
      label: t("machines.site"),
      type: "enum",
      operators: ["eq"],
      suggest: bySlug("/v1/sites"),
      valueLabel: (id) => labelOf("core.site", id),
    },
    host: {
      key: "host",
      label: t("machines.host"),
      type: "enum",
      operators: ["eq"],
      suggest: hosts("both"),
      valueLabel: hostLabel,
    },
    cluster: {
      key: "cluster",
      label: t("filter.cluster"),
      type: "enum",
      operators: ["eq"],
      suggest: hosts("clusters"),
      valueLabel: (id) => labelOf("machines.cluster", id),
    },
  };
  return OFFERED.filter((key) => served.has(key)).map((key) => properties[key]);
});

/** The filters of the query of the page. */
const fromPage = () => filtersFromParams(model.value, new URLSearchParams(location.search)).filters;
const filters = ref<Filter[]>(fromPage());
// Back or forward in the history: the element is kept, its filters read again.
const onPop = () => (filters.value = fromPage());
window.addEventListener("popstate", onPop);
onScopeDispose(() => window.removeEventListener("popstate", onPop));

/** The filters applied and well typed: those sent. */
const applied = computed(() =>
  filters.value.filter((f) => !f.draft && checkFilter(model.value, f) === undefined),
);
/** The route takes one value of each parameter: the labels of those given more. */
const repeated = computed(() => {
  const count = new Map<string, number>();
  for (const f of applied.value) count.set(f.key, (count.get(f.key) ?? 0) + f.values.length);
  return model.value.filter((p) => (count.get(p.key) ?? 0) > 1).map((p) => p.label ?? p.key);
});
const sent = computed<Filters>(() =>
  Object.fromEntries(applied.value.map((f) => [f.key, f.values[0] ?? ""])),
);

// The filters in the query of the page, by a link the shell follows with its router: a new
// entry of the history, which the back button returns to; the other parameters (the dates)
// kept as they are.
watch(
  () => filtersToParams(model.value, filters.value).toString(),
  () => {
    const now = new URLSearchParams(location.search);
    const next = filtersToParams(
      model.value,
      filters.value,
      filtersFromParams(model.value, now).rest,
    );
    if (next.toString() === now.toString()) return;
    const query = next.toString();
    followLink(element, `${location.pathname}${query ? `?${query}` : ""}`);
  },
);

const picker = useTemplateRef<InstanceType<typeof FilterPicker>>("picker");
/** The funnel of a header, or Alt+Down on it: the bar's picker, on that column's filter. */
function onColumnFilter(column: string): void {
  const key = Object.keys(COLUMN_OF).find((k) => COLUMN_OF[k] === column);
  const from = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  if (key) picker.value?.openFor(key, from);
}
function clearFilters(): void {
  filters.value = [];
}

// --------------------------------------------------------------------------------- list

const options = () =>
  repeated.value.length ? null : (reads.list(PATH, sent.value as never) as never);
const list = useList<Machine, never>(options);
/** The list read again from the server, its failure gone (« Réessayer »). */
function retry(): void {
  const read = options() as { queryKey: readonly unknown[] } | null;
  if (read) void queryClient.refetchQueries({ queryKey: read.queryKey, exact: true });
}
const state = computed(() => list.state.value);
const rows = computed(() =>
  (state.value.items ?? []).map((m) => ({
    id: m.id,
    name: m.name,
    kind: m.kind,
    owner: m.owner?.id ?? null,
    site: m.site?.id ?? null,
    host: m.host ?? null,
  })),
);
type Row = (typeof rows.value)[number];

const columns = computed<GridColumn[]>(() => {
  const filterable = new Set(model.value.map((p) => COLUMN_OF[p.key]).filter(Boolean));
  const column = (id: string, label: string, more: Partial<GridColumn>): GridColumn => ({
    id,
    label,
    ...more,
    ...(filterable.has(id) ? { filterable: true } : {}),
  });
  return [
    column("name", t("machines.name"), { width: "fill" }),
    column("kind", t("machines.kind"), { width: "sm" }),
    column("owner", t("machines.owner"), { width: "md", priority: 2 }),
    column("site", t("machines.site"), { width: "md", priority: 3 }),
    column("host", t("machines.host"), { width: "md", priority: 2 }),
  ];
});
const filtered = computed(() => [
  ...new Set(applied.value.map((f) => COLUMN_OF[f.key]).filter((c): c is string => !!c)),
]);

/** Every page read already, as many as the cache holds; beyond, said above the grid. */
const capped = computed(() => state.value.more && state.value.pages >= MAX_PAGES);
function onWindow(window: GridWindow): void {
  const s = state.value;
  if (window.end < rows.value.length - AHEAD) return;
  if (s.more && !s.fetching && !s.previous && s.pages < MAX_PAGES) list.next();
}

/** The page of the record of a row, at the dates of the context. */
function recordOf(row: Row): string {
  void context.value.time;
  return hrefOf(PAGES.machine(row.id));
}
/** Enter on a cell: the record of its row, its heading focused. */
function onActivate(cell: CellRef): void {
  const row = rows.value.find((r) => r.id === cell.row);
  if (!row) return;
  arrival.byLink = true;
  followLink(element, recordOf(row));
}
const hostPage = (host: Row["host"]) =>
  host && ("machine" in host ? PAGES.machine(host.machine.id) : PAGES.cluster(host.cluster.id));

const failure = computed(() => (state.value.items === undefined ? state.value.error : null));
const nextFailure = computed(() => (state.value.items !== undefined ? state.value.error : null));
const problemOf = (error: unknown) => (error instanceof ApiProblem ? error : undefined);

// ----------------------------------------------------------------------------- creation

const entities = useList<{ id: string; slug: string }, never>(() =>
  present() ? (reads.list("/v1/entities") as never) : null,
);
/** The entities a machine may be owned by: the first page, by slug. */
const owners = computed(() =>
  [...(entities.state.value.items ?? [])]
    .sort((a, b) => a.slug.localeCompare(b.slug))
    .map((e): ListOption => ({ value: e.id, label: e.slug, mono: true })),
);
const kinds = computed(() =>
  KINDS.map((kind): ListOption => ({ value: kind, label: t(`kind.${kind}`) })),
);
const name = ref("");
const kind = ref<string | undefined>("physical");
const owner = ref<string | undefined>();
/** What is said on each field, by the name of the member of the operation. */
type Errors = Partial<Record<"name" | "kind" | "owner", string | undefined>>;
const errors = shallowRef<Errors>({});
// The first owner taken once they are read, unless one is chosen.
watch(owners, (now) => {
  if (!now.some((o) => o.value === owner.value)) owner.value = now[0]?.value;
});
watch(name, () => (errors.value = { ...errors.value, name: undefined }));
watch(kind, () => (errors.value = { ...errors.value, kind: undefined }));
watch(owner, () => (errors.value = { ...errors.value, owner: undefined }));

// The field of the owner is mounted again as its options come (`SelectValue`, skill
// kartenn-ui « Pièges de Reka UI »): the focus it held goes to the new one.
const ownerField = useTemplateRef<HTMLElement>("ownerField");
watch(
  () => owners.value.length,
  async () => {
    const field = ownerField.value;
    if (!field?.contains(document.activeElement)) return;
    await nextTick();
    field.querySelector<HTMLElement>("[role='combobox']")?.focus();
  },
  { flush: "pre" },
);

function creation() {
  const required = t("form.required");
  const trimmed = name.value.trim();
  errors.value = {
    ...(trimmed ? {} : { name: required }),
    ...(kind.value ? {} : { kind: required }),
    ...(owner.value ? {} : { owner: required }),
  };
  if (!trimmed || !kind.value || !owner.value) return null;
  return {
    op: "machines.create_machine",
    name: trimmed,
    kind: kind.value,
    owner: { id: owner.value },
  } as never;
}
/** The violations of the server on the fields: said there; whether one was. */
function violations(problem: ApiProblem): boolean {
  const next: Errors = {};
  for (const v of problem.violations) {
    const field = v.pointer?.split("/").pop();
    if ((field === "name" || field === "kind" || field === "owner") && !next[field]) {
      next[field] = `${v.detail} (${v.rule})`;
    }
  }
  errors.value = next;
  return Object.keys(next).length > 0;
}
function added(): void {
  // The name cleared for the next machine; the outcome said stays (no event of the user's).
  name.value = "";
  errors.value = {};
}
</script>

<template>
  <Stack gap="6">
    <header>
      <Stack gap="1">
        <h1 tabindex="-1">{{ t("machines.title") }}</h1>
        <p>{{ t("machines.intro") }}</p>
      </Stack>
    </header>
    <Stack gap="4">
      <FilterPicker ref="picker" v-model="filters" :model="model" :label="t('filter.label')" />
      <InlineAlert
        v-if="repeated.length"
        tone="ko"
        :title="t('filter.once', { names: repeated.join(', ') })"
      />
      <ErrorState
        v-else-if="failure"
        :title="t('machines.failed')"
        :problem="problemOf(failure)"
        :retry-label="t('retry')"
        @retry="retry"
      >
        {{ problemOf(failure) ? "" : t("network") }}
      </ErrorState>
      <EmptyState
        v-else-if="state.items?.length === 0 && !state.previous"
        :title="applied.length ? t('machines.noneFiltered') : t('machines.none')"
      >
        {{ applied.length ? t("machines.noneFilteredText") : t("machines.noneText") }}
        <template v-if="applied.length" #action>
          <Button variant="ghost" icon="x" @click="clearFilters">{{ t("filter.clear") }}</Button>
        </template>
      </EmptyState>
      <template v-else>
        <InlineAlert
          v-if="nextFailure"
          tone="ko"
          :title="t('machines.failed')"
          :items="linesOf(nextFailure, t)"
        />
        <InlineAlert v-if="capped" tone="info" :title="t('machines.first', { n: rows.length })" />
        <DataGrid
          :label="t('machines.title')"
          :columns="columns"
          :rows="rows"
          row-key="id"
          :loading="state.items === undefined"
          :loading-rows="8"
          :filtered="filtered"
          :row-count="state.items !== undefined && !state.more ? rows.length : -1"
          @filter="onColumnFilter"
          @activate="onActivate"
          @window="onWindow"
        >
          <template #cell="{ row, column }">
            <a v-if="column.id === 'name'" :href="recordOf(row)">{{ row.name }}</a>
            <KeyLabel
              v-else-if="column.id === 'kind'"
              :value="row.kind"
              :label="t(`kind.${row.kind}`)"
              plain
            />
            <CitedName
              v-else-if="column.id === 'owner'"
              :id="row.owner"
              :names="names"
              type="core.entity"
            />
            <CitedName
              v-else-if="column.id === 'site'"
              :id="row.site"
              :names="names"
              type="core.site"
            />
            <CitedName
              v-else-if="column.id === 'host'"
              :id="row.host && ('machine' in row.host ? row.host.machine.id : row.host.cluster.id)"
              :names="names"
              :type="row.host && 'cluster' in row.host ? 'machines.cluster' : 'machines.machine'"
              :page="hostPage(row.host) ?? undefined"
            />
          </template>
        </DataGrid>
      </template>
    </Stack>
    <section v-if="present()" aria-labelledby="k-machines-create">
      <Stack gap="4">
        <h2 id="k-machines-create">{{ t("machines.create") }}</h2>
        <DraftForm
          permission="machines.edit"
          :operation="creation"
          :violations="violations"
          @added="added"
        >
          <TextField v-model="name" :label="t('machines.name')" :error="errors.name" required />
          <Select
            v-model="kind"
            :label="t('machines.kind')"
            :options="kinds"
            :error="errors.kind"
            required
          />
          <div ref="ownerField">
            <Select
              :key="owners.length"
              v-model="owner"
              :label="t('machines.owner')"
              :options="owners"
              :placeholder="entities.state.value.pending ? t('loading') : undefined"
              :readonly="owners.length === 0"
              :error="errors.owner"
              mono
              required
            />
          </div>
        </DraftForm>
      </Stack>
    </section>
  </Stack>
</template>
