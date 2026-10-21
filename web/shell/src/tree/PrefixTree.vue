<script setup lang="ts">
import { useVirtualizer } from "@tanstack/vue-virtual";
import { computed, nextTick, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { PAGE_SIZE, Pager, withQuery } from "../data/api.ts";
import { datedSearch, useTime } from "../time/index.ts";
import {
  label as nodeLabel,
  moveFocus,
  recordPath,
  ROOT,
  Tree,
  type Fetcher,
  type Node,
} from "./model.ts";

// The tree of a domain (REQ-007-17, 31, 33; design « Thème, composants, arbre »): the
// `treeview` pattern of the APG, focus on the tree and `aria-activedescendant`, rows
// virtualised at a fixed height, the children of a prefix read page by page as their
// `more` row nears the window.

const props = defineProps<{ domain: string; label: string }>();

/** Height of a row, in pixels: the virtualiser lays rows out by it. */
const ROW = 32;

const { t } = useI18n();
const router = useRouter();
const time = useTime();
const prefix = useId();

// One pager per parent (lists of the shell, task 17): its `next` link carries the dates,
// a refused cursor starts again from the first page, whose children already read are not
// repeated by the model.
const pagers = new Map<string, Pager<Node>>();
const fetcher: Fetcher = async (parent) => {
  let pager = pagers.get(parent);
  if (!pager) {
    const path =
      parent === ROOT ? `/v1/domains/${props.domain}/roots` : `/v1/prefixes/${parent}/children`;
    pager = new Pager<Node>(
      withQuery(path + datedSearch(time.value), { page_size: String(PAGE_SIZE) }),
    );
    pagers.set(parent, pager);
  }
  const before = pager.items.value.length;
  await pager.more();
  if (pager.problem.value) {
    // The next attempt starts again from the first page.
    pagers.delete(parent);
    throw new Error(pager.problem.value.title);
  }
  const items = pager.items.value.slice(pager.restarted.value ? 0 : before);
  pager.restarted.value = false;
  return { items, next: pager.done.value ? null : "next" };
};

const tree = new Tree(fetcher);
const version = ref(0);
const rows = computed(() => {
  void version.value;
  return tree.rows();
});
const started = ref(false);
const activeKey = ref<string | null>(null);
const activeIndex = computed(() => {
  const index = rows.value.findIndex((row) => row.key === activeKey.value);
  return index >= 0 ? index : 0;
});

/** Runs a change of the tree, and redraws before and after it. */
async function change(work: Promise<void>): Promise<void> {
  version.value += 1;
  await work;
  version.value += 1;
}

const scroller = ref<HTMLElement | null>(null);
const virtualizer = useVirtualizer(
  computed(() => ({
    count: rows.value.length,
    getScrollElement: () => scroller.value,
    estimateSize: () => ROW,
    overscan: 10,
    getItemKey: (index: number) => rows.value[index]?.key ?? index,
  })),
);
/** The rows in the window, with their offset. */
const visible = computed(() =>
  virtualizer.value.getVirtualItems().flatMap((item) => {
    const row = rows.value[item.index];
    return row ? [{ row, index: item.index, start: item.start }] : [];
  }),
);

// A `more` row that comes into the window reads the next page of its parent.
watch(visible, (shown) => {
  for (const { row } of shown) {
    if (row.kind === "more" && !row.loading && !row.failed) void change(tree.loadMore(row.parent));
  }
});

watch(
  () => [props.domain, time.value.validAt, time.value.asOf],
  async () => {
    tree.reset();
    pagers.clear();
    activeKey.value = null;
    started.value = false;
    await change(tree.loadMore(ROOT));
    started.value = true;
    // The roots drawn: the first useful screen of the browser bench (REQ-007-32).
    await nextTick();
    performance.mark("k:ready");
  },
  { immediate: true },
);

function rowId(key: string): string {
  return `${prefix}-${key}`;
}

function focusRow(index: number): void {
  const row = rows.value[index];
  if (!row) return;
  activeKey.value = row.key;
  virtualizer.value.scrollToIndex(index);
}

function activate(index: number): void {
  const row = rows.value[index];
  if (!row) return;
  if (row.kind === "more") void change(tree.loadMore(row.parent));
  else void router.push(recordPath(row.node));
}

function toggle(index: number): void {
  const row = rows.value[index];
  activeKey.value = row?.key ?? null;
  if (row?.kind !== "node" || row.expanded === null) return;
  if (row.expanded) {
    tree.collapse(row.key);
    version.value += 1;
  } else void change(tree.expand(row.key));
}

function onKey(event: KeyboardEvent): void {
  if (event.key === "Enter") {
    event.preventDefault();
    activate(activeIndex.value);
    return;
  }
  const move = moveFocus(rows.value, activeIndex.value, event.key);
  if (!move) return;
  event.preventDefault();
  if (move.open) void change(tree.expand(move.open));
  if (move.close) {
    tree.collapse(move.close);
    version.value += 1;
  }
  if (move.to !== undefined) focusRow(move.to);
}

/** The position of a row in the sizer, indented by its level. */
function placed(start: number, level: number): Record<string, string> {
  return {
    transform: `translateY(${start}px)`,
    height: `${ROW}px`,
    paddingInlineStart: `calc(${level - 1} * 1.25rem + 0.5rem)`,
  };
}

/** What a node is, for its label: the nature of a prefix, else its kind. */
function kindOf(node: Node): string {
  return node.kind === "prefix" ? node.nature : node.kind;
}

const activeId = computed(() => {
  const row = rows.value[activeIndex.value];
  return row ? rowId(row.key) : undefined;
});
</script>

<template>
  <p v-if="!started" class="k-muted" role="status">{{ t("app.loading") }}</p>
  <p v-else-if="rows.length === 0" class="k-muted">{{ t("tree.empty") }}</p>
  <div
    v-else
    ref="scroller"
    class="k-tree"
    role="tree"
    tabindex="0"
    :aria-label="props.label"
    :aria-activedescendant="activeId"
    @keydown="onKey"
  >
    <div role="none" class="k-tree-sizer" :style="{ height: `${virtualizer.getTotalSize()}px` }">
      <template v-for="{ row, index, start } in visible" :key="row.key">
        <div
          v-if="row.kind === 'node'"
          :id="rowId(row.key)"
          role="treeitem"
          class="k-tree-row"
          :data-active="index === activeIndex || undefined"
          :aria-level="row.level"
          :aria-posinset="row.posinset"
          :aria-setsize="row.setsize"
          :aria-expanded="row.expanded ?? undefined"
          :style="placed(start, row.level)"
          @click="toggle(index)"
        >
          <span class="k-tree-twisty" aria-hidden="true">{{
            row.expanded === null ? "" : row.expanded ? "▾" : "▸"
          }}</span>
          <RouterLink :to="recordPath(row.node)" tabindex="-1" @click.stop>{{
            nodeLabel(row.node)
          }}</RouterLink>
          <span class="k-muted">{{ t(`tree.kind.${kindOf(row.node)}`) }}</span>
        </div>
        <div
          v-else
          :id="rowId(row.key)"
          role="treeitem"
          class="k-tree-row k-muted"
          :data-active="index === activeIndex || undefined"
          :aria-level="row.level"
          :style="placed(start, row.level)"
          @click="activate(index)"
        >
          {{ row.failed ? t("tree.failed") : row.loading ? t("app.loading") : t("tree.more") }}
        </div>
      </template>
    </div>
  </div>
</template>
