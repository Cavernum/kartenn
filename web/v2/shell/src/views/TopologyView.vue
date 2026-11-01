<script setup lang="ts">
// The topography (REQ-021-9; design 021 « Écrans » : Topographie; maquette E1): the tree of
// the prefixes of a domain in a `TreeGrid` (parents deduced, no move), its first level and
// the children of each node read page by page through the cache at the dates of the page,
// the children of a closed node read on intent (REQ-021-22, `childrenIntent`); beside it,
// the content of the chosen node (`?node=<id>`), its record one link away. The domain is
// chosen in the URL (`?domain=<id>`), the first one by default.
import { queryClient, reads, type components, type TimeArg } from "@kartenn/sdk";
import {
  EmptyState,
  ErrorState,
  Select,
  Split,
  TreeGrid,
  type GridColumn,
  type IconName,
  type ListOption,
  type TreeNode,
  type TreeNodeState,
} from "@kartenn/ui";
import {
  InfiniteQueryObserver,
  type InfiniteData,
  type InfiniteQueryObserverResult,
} from "@tanstack/query-core";
import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowReactive,
  useTemplateRef,
  watch,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { useInfiniteQuery, useQuery } from "../data/query.ts";
import KLink from "../prefetch/KLink.vue";
import { childrenIntent } from "../prefetch/tree.ts";
import { useTime } from "../time/index.ts";
import { pageOfObject } from "./collections.ts";
import { problemOf, rowsOf, useKeyLabel } from "./data.ts";
import ViewPanel from "./parts/ViewPanel.vue";
import ContentTab from "./record/ContentTab.vue";

type Node = components["schemas"]["TreeNode"];
type Pages = InfiniteData<{ data: { items: Node[] } }, string | null>;
type Result = InfiniteQueryObserverResult<Pages>;

const { t, te } = useI18n();
const route = useRoute();
const router = useRouter();
const time = useTime();
const keyLabel = useKeyLabel();
const dated = (): TimeArg => ({ validAt: time.value.validAt, asOf: time.value.asOf });

// The domains, for the selector: the first page of their list (a few, in practice).
const domains = useInfiniteQuery(() => reads.list("/v1/domains", null, dated()));
const domainItems = computed(
  () => rowsOf(domains.data.value) as unknown as { id: string; slug: string }[],
);
const domainOptions = computed<ListOption[]>(() =>
  domainItems.value.map((d) => ({ value: d.id, label: d.slug, mono: true })),
);
const domain = computed(() => {
  const asked = route.query.domain;
  return typeof asked === "string" && domainItems.value.some((d) => d.id === asked)
    ? asked
    : domainItems.value[0]?.id;
});
const domainSlug = computed(() => domainItems.value.find((d) => d.id === domain.value)?.slug ?? "");
function chooseDomain(id: string | undefined): void {
  if (!id || id === domain.value) return;
  void router.replace({ query: { ...route.query, domain: id, node: undefined } });
}

// The pages of the tree: one observer of the cache per level read (the first level, then
// each node opened), its result kept here; all of them start again with the domain or the
// dates.
const results = shallowReactive(new Map<string | null, Result>());
const observers = new Map<
  string | null,
  { observer: InfiniteQueryObserver<Pages>; stop: () => void }
>();
const optionsOf = (parent: string | null) =>
  parent === null
    ? reads.subPages<Node>("domain", domain.value!, "roots", null, dated())
    : reads.subPages<Node>("prefix", parent, "children", null, dated());
function observe(parent: string | null): void {
  if (observers.has(parent)) return;
  const options = queryClient.defaultQueryOptions(optionsOf(parent) as never);
  const observer = new InfiniteQueryObserver<Pages>(queryClient, options as never);
  results.set(parent, observer.getOptimisticResult(options as never) as Result);
  const stop = observer.subscribe((result) => results.set(parent, result as Result));
  observers.set(parent, { observer, stop });
}
function reset(): void {
  for (const { stop } of observers.values()) stop();
  observers.clear();
  results.clear();
  if (domain.value) observe(null);
}
watch([domain, () => time.value.validAt, () => time.value.asOf], reset, { immediate: true });
onBeforeUnmount(() => {
  for (const { stop } of observers.values()) stop();
});
/** The tree asks for the children of a node, or the next page of a level. */
function onLoad(parent: string | null): void {
  const known = observers.get(parent);
  if (!known) observe(parent);
  else if (results.get(parent)?.hasNextPage) void known.observer.fetchNextPage();
}

const ICONS: Record<Node["kind"], IconName> = {
  prefix: "prefix",
  range: "range",
  address: "address",
};
const valueOf = (node: Node) =>
  node.kind === "range" ? `${node.first} – ${node.last}` : node.value;
const detailOf = (node: Node) => {
  const value = node.kind === "prefix" ? node.nature : node.kind === "range" ? node.usage : null;
  if (!value) return "";
  const key = `views.value.${node.kind === "prefix" ? "nature" : "usage"}.${value}`;
  return te(key) ? t(key) : value;
};
const itemsOf = (parent: string | null) =>
  rowsOf(results.get(parent)?.data as never) as unknown as Node[];
function nodesOf(parent: string | null): TreeNode[] {
  return itemsOf(parent).map((item) => {
    const node: TreeNode = {
      id: item.id,
      label: valueOf(item),
      icon: ICONS[item.kind],
      cells: { detail: detailOf(item) },
      detail: keyLabel("kind", `core.${item.kind}`) ?? item.kind,
    };
    if (item.kind !== "prefix") return node;
    const children = results.get(item.id);
    if (children?.data) {
      node.children = nodesOf(item.id);
      if (children.hasNextPage) node.more = true;
    } else node.hasChildren = true;
    return node;
  });
}
const nodes = computed(() => nodesOf(null));
const roots = computed(() => results.get(null));
const nodeState = (node: TreeNode): TreeNodeState | undefined =>
  results.get(node.id)?.isFetching ? "loading-children" : undefined;
const columns = computed<GridColumn[]>(() => [
  { id: "detail", label: t("views.record.nature"), width: "sm" },
]);

/** The open nodes: closed again with the domain or the dates (their pages start again). */
const expanded = ref<string[]>([]);
watch([domain, () => time.value.validAt, () => time.value.asOf], () => (expanded.value = []));

const selected = computed({
  get: () => (typeof route.query.node === "string" ? route.query.node : undefined),
  set: (id: string | undefined) => void router.replace({ query: { ...route.query, node: id } }),
});
/** A node read, by identifier, in any level read so far. */
function nodeById(id: string | undefined): Node | undefined {
  if (id === undefined) return undefined;
  for (const parent of results.keys()) {
    const found = itemsOf(parent).find((item) => item.id === id);
    if (found) return found;
  }
  return undefined;
}
/**
 * A node of the URL not read yet (a link, a reload): read as a prefix — the tree's nodes
 * that hold content are prefixes — until its level is read.
 */
const unread = computed(() =>
  selected.value !== undefined && nodeById(selected.value) === undefined
    ? selected.value
    : undefined,
);
const fallback = useQuery(() => ({
  ...reads.object<{
    id: string;
    value: string;
    nature: "container" | "network";
    links: { self: string };
  }>("prefix", unread.value ?? "", dated()),
  enabled: unread.value !== undefined,
}));
const chosen = computed<Node | undefined>(() => {
  const read = nodeById(selected.value);
  if (read) return read;
  const prefix = fallback.data.value?.data;
  return prefix && prefix.id === selected.value
    ? {
        kind: "prefix",
        id: prefix.id,
        value: prefix.value,
        nature: prefix.nature,
        href: prefix.links.self,
      }
    : undefined;
});
function onActivate(id: string): void {
  const node = nodeById(id);
  const to = node ? pageOfObject(node.kind, node.id) : null;
  if (to) void router.push(to);
}

// The children of a closed node, read on intent.
const tree = useTemplateRef<HTMLElement>("tree");
let stopIntent = () => {};
onMounted(() => {
  if (tree.value)
    stopIntent = childrenIntent(tree.value, (id) => [
      reads.subPages<Node>("prefix", id, "children", null, dated()),
    ]);
});
onBeforeUnmount(() => stopIntent());
</script>

<template>
  <h1 class="k-visually-hidden">{{ t("nav.topology") }}</h1>
  <ErrorState
    v-if="domains.isError.value"
    :title="t('views.topology.domainsError')"
    :problem="problemOf(domains.error.value)"
    :retry-label="t('views.retry')"
    @retry="domains.refetch()"
  />
  <EmptyState
    v-else-if="domains.isSuccess.value && domainItems.length === 0"
    icon="domain"
    :title="t('views.topology.noDomain')"
  >
    {{ t("views.topology.noDomainText") }}
  </EmptyState>
  <Split v-else side-size="lg" gap="3">
    <template #side>
      <ViewPanel :title="t('nav.prefixes')" :detail="domainSlug" mono flush>
        <template #actions>
          <Select
            :model-value="domain"
            :label="t('views.field.domain')"
            :options="domainOptions"
            inline-label
            mono
            size="sm"
            @update:model-value="chooseDomain"
          />
        </template>
        <div ref="tree" class="k-topology__tree">
          <ErrorState
            v-if="roots?.isError && !roots.isFetchNextPageError"
            :title="t('views.topology.treeError')"
            :problem="problemOf(roots.error)"
            :retry-label="t('views.retry')"
            @retry="roots.refetch()"
          />
          <EmptyState
            v-else-if="roots?.isSuccess && nodes.length === 0"
            :title="t('views.topology.empty')"
          />
          <TreeGrid
            v-else
            v-model:expanded="expanded"
            v-model:selected="selected"
            :label="t('views.topology.tree', { name: domainSlug })"
            :tree-label="t('views.topology.prefix')"
            :nodes="nodes"
            :columns="columns"
            :more="roots?.hasNextPage ?? false"
            :more-loading="roots?.isFetchingNextPage ?? false"
            :loading="!roots || roots.isPending"
            :loading-rows="8"
            :node-state="nodeState"
            parent-mode="derived"
            @load="onLoad"
            @activate="onActivate"
          />
        </div>
      </ViewPanel>
    </template>
    <ViewPanel
      v-if="chosen"
      :title="valueOf(chosen)"
      :detail="detailOf(chosen)"
      main
      :flush="chosen.kind === 'prefix'"
    >
      <template #actions>
        <KLink
          v-if="pageOfObject(chosen.kind, chosen.id)"
          :to="pageOfObject(chosen.kind, chosen.id)!"
          >{{ t("views.topology.open") }}</KLink
        >
      </template>
      <ContentTab
        v-if="chosen.kind === 'prefix'"
        :id="chosen.id"
        kind="prefix"
        :name="valueOf(chosen)"
      />
      <p v-else>{{ t("views.topology.leaf") }}</p>
    </ViewPanel>
    <ViewPanel v-else :title="t('views.topology.content')" main>
      <EmptyState icon="folder-tree" :title="t('views.topology.choose')">
        {{ t("views.topology.chooseText") }}
      </EmptyState>
    </ViewPanel>
  </Split>
</template>

<style>
@layer components {
  .k-topology__tree {
    display: grid;
    min-inline-size: 0;
  }
}
</style>
