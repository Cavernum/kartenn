<script setup lang="ts">
// The topography (REQ-021-9; design 021 « Écrans » : Topographie; maquette E1): the tree of
// the prefixes of a domain in a `TreeGrid` (parents deduced, no move), its first level and
// the children of each node read page by page through the cache at the dates of the page,
// the children of a closed node read on intent (REQ-021-22, `childrenIntent`); beside it,
// the content of the chosen node (`?node=<id>`), its record one link away. The domain is
// chosen in the URL (`?domain=<id>`), the first one by default. The « + » of a prefix, `c`
// and `e` open the edit panel of the kind (task 12). No move (task 24, REQ-021-17): a
// prefix changes place by its value, through the draft, and the tree is read again.
import {
  drafts,
  queryClient,
  reads,
  type components,
  type Draft,
  type Entry,
  type TimeArg,
} from "@kartenn/sdk";
import {
  EmptyState,
  ErrorState,
  InlineAlert,
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
  watchEffect,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { useInfiniteQuery, useQuery } from "../data/query.ts";
import KLink from "../prefetch/KLink.vue";
import { childrenIntent } from "../prefetch/tree.ts";
import { useSession } from "../rights/index.ts";
import { useViewAction } from "../shortcuts/view.ts";
import { useTime } from "../time/index.ts";
import { pageOfObject, type CollectionId } from "./collections.ts";
import { draftedValues, editRight } from "./edit/fields.ts";
import { openEdit } from "./edit/panel.ts";
import { problemOf, rowsOf, useKeyLabel } from "./data.ts";
import ViewPanel from "./parts/ViewPanel.vue";
import { nodesOf } from "./topology.ts";
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

// The domains, for the selector: their first page (a few, in practice); the domain of the
// URL is looked for in the pages after it (ten at most) before the tree falls back to the
// first one, which it then says.
const DOMAIN_PAGES = 10;
const domains = useInfiniteQuery(() => reads.list("/v1/domains", null, dated()));
const domainItems = computed(
  () => rowsOf(domains.data.value) as unknown as { id: string; slug: string }[],
);
const domainOptions = computed<ListOption[]>(() =>
  domainItems.value.map((d) => ({ value: d.id, label: d.slug, mono: true })),
);
const asked = computed(() =>
  typeof route.query.domain === "string" ? route.query.domain : undefined,
);
const found = computed(
  () => asked.value !== undefined && domainItems.value.some((d) => d.id === asked.value),
);
const searching = computed(
  () =>
    asked.value !== undefined &&
    !found.value &&
    domains.hasNextPage.value &&
    !domains.isFetchNextPageError.value &&
    (domains.data.value?.pages.length ?? 0) < DOMAIN_PAGES,
);
watchEffect(() => {
  if (searching.value && !domains.isFetching.value) void domains.fetchNextPage();
});
const domain = computed(() =>
  found.value ? asked.value : searching.value ? undefined : domainItems.value[0]?.id,
);
/** The domain of the URL was not found: the tree shows the first one, and says so. */
const fellBack = computed(
  () => asked.value !== undefined && !found.value && !searching.value && domains.isSuccess.value,
);
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
/** A level read, as the tree takes it: `undefined` until its first page is read. */
const levelOf = (parent: string | null) => {
  const result = results.get(parent);
  return result?.data ? { items: itemsOf(parent), more: result.hasNextPage } : undefined;
};
const nodeOf = (item: Node): TreeNode => ({
  id: item.id,
  label: valueOf(item),
  icon: ICONS[item.kind],
  cells: { detail: detailOf(item) },
  detail: keyLabel("kind", `core.${item.kind}`) ?? item.kind,
});
// Each node placed once, even while its levels are read again (`topology.ts`).
const nodes = computed(() => nodesOf(levelOf, nodeOf, (item) => item.kind === "prefix"));
const roots = computed(() => results.get(null));

// Parents deduced (D34, REQ-021-17): no move is offered; a node whose value the active
// draft changes stays where the present has it, marked « en brouillon » — the tree is never
// retouched by the client, only read again once a change is applied (X24).
const active = ref<string | null>(drafts.active());
const stopActive = drafts.onActive((id) => (active.value = id));
onBeforeUnmount(stopActive);
const activeDraft = useQuery(() => ({
  ...drafts.get(active.value ?? "-"),
  enabled: active.value !== null,
}));
const inDraft = computed(() => {
  const operations = (activeDraft.data.value as Entry<Draft> | undefined)?.data.operations;
  return new Set(
    (["prefixes", "ranges", "addresses"] as const).flatMap((c) => [
      ...draftedValues(operations, c).keys(),
    ]),
  );
});
const nodeState = (node: TreeNode): TreeNodeState | undefined =>
  results.get(node.id)?.isFetching
    ? "loading-children"
    : time.value.present && inDraft.value.has(node.id)
      ? "draft"
      : undefined;
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

// Direct addition (024/M10, REQ-021-36): the « + » of a prefix, and `c`, open the edit
// panel of the kinds it may hold — a network its addresses and ranges, a container its
// prefixes — the domain given; `e` modifies the chosen node. At the present only; the
// panel refuses, visibly, what the rights do not allow.
const session = useSession();
const KINDS: Record<Node["kind"], CollectionId> = {
  prefix: "prefixes",
  range: "ranges",
  address: "addresses",
};
// The « + » only for whom may add (the tree has no refused « + »: Enter on a node would go
// into a button that refuses, rather than open its record).
const addLabel = (node: TreeNode) =>
  time.value.present && mayAdd() && nodeById(node.id)?.kind === "prefix"
    ? t("edit.addIn", { within: node.label })
    : undefined;
function addIn(id: string | undefined): void {
  const node = nodeById(id) ?? chosen.value;
  const inside = node?.kind === "prefix" ? node : undefined;
  const kinds: CollectionId[] =
    inside?.nature === "network" ? ["addresses", "ranges"] : ["prefixes"];
  openEdit({
    mode: "create",
    collection: kinds[0]!,
    ...(inside ? { kinds, within: valueOf(inside) } : {}),
    ...(domain.value ? { preset: { domain: domain.value } } : {}),
  });
}
const mayAdd = () =>
  (["core.prefix", "core.range", "core.address"] as const).some((type) =>
    session.value.rights.has(editRight(type)),
  );
useViewAction("create", {
  label: () =>
    chosen.value?.kind === "prefix"
      ? t("edit.addIn", { within: valueOf(chosen.value) })
      : t("edit.createAction", { kind: t("labels.kind.core.prefix") }),
  allowed: () => mayAdd() && domain.value !== undefined,
  run: () => addIn(chosen.value?.id),
});
useViewAction("edit", {
  label: () =>
    t("edit.editThis", { kind: detailKind.value, name: chosen.value ? valueOf(chosen.value) : "" }),
  allowed: () =>
    chosen.value !== undefined && session.value.rights.has(editRight(`core.${chosen.value.kind}`)),
  run: () => {
    if (chosen.value) {
      openEdit({ mode: "edit", collection: KINDS[chosen.value.kind], id: chosen.value.id });
    }
  },
});
const detailKind = computed(() =>
  chosen.value ? (keyLabel("kind", `core.${chosen.value.kind}`) ?? chosen.value.kind) : "",
);

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
  <InlineAlert v-if="fellBack" tone="warn" :title="t('views.topology.domainMissing')">
    {{ t("views.topology.domainMissingText", { name: domainSlug }) }}
  </InlineAlert>
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
            :add-label="addLabel"
            parent-mode="derived"
            @load="onLoad"
            @activate="onActivate"
            @add="addIn"
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
