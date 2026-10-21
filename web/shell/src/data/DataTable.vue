<script setup lang="ts" generic="T extends Record<string, unknown>">
import { useVirtualizer } from "@tanstack/vue-virtual";
import { computed, nextTick, ref, watch } from "vue";
import { useRouter } from "vue-router";

// A long list (REQ-007-31, design « Thème, composants, arbre »): native `<table>`, rows
// virtualised at a fixed height, `aria-rowcount` and `aria-rowindex`, one focusable row at
// a time (arrows, Home, End), Enter opens the record; the next page is asked for as the
// last rows come near.

export interface Column {
  key: string;
  label: string;
}

const props = defineProps<{
  rows: T[];
  columns: readonly Column[];
  caption: string;
  done: boolean;
  loading: boolean;
  /** The page of a row, else `null`. */
  href: (row: T) => string | null;
  /** The text of a cell. */
  cell: (row: T, key: string) => string;
  /** The page a cell links to, else `null` (summaries). */
  link?: (row: T, key: string) => string | null;
}>();
const emit = defineEmits<{ more: [] }>();

/** Rows ahead of the end that ask for the next page. */
const AHEAD = 20;
const ROW = 32;

const router = useRouter();
const scroller = ref<HTMLElement | null>(null);
const active = ref(0);

const virtualizer = useVirtualizer(
  computed(() => ({
    count: props.rows.length,
    getScrollElement: () => scroller.value,
    estimateSize: () => ROW,
    overscan: 8,
  })),
);
const virtual = computed(() => virtualizer.value.getVirtualItems());
const before = computed(() => virtual.value[0]?.start ?? 0);
const after = computed(
  () => virtualizer.value.getTotalSize() - (virtual.value[virtual.value.length - 1]?.end ?? 0),
);

watch(
  [virtual, () => props.rows.length, () => props.done, () => props.loading],
  () => {
    if (props.done || props.loading) return;
    const last = virtual.value[virtual.value.length - 1];
    if (props.rows.length === 0 || (last && last.index >= props.rows.length - AHEAD)) emit("more");
  },
  { immediate: true },
);

async function focusRow(index: number): Promise<void> {
  if (props.rows.length === 0) return;
  active.value = Math.max(0, Math.min(index, props.rows.length - 1));
  virtualizer.value.scrollToIndex(active.value);
  // The row is rendered once the scroll has moved the window: a few frames at most.
  for (let frame = 0; frame < 10; frame += 1) {
    await nextTick();
    const row = scroller.value?.querySelector<HTMLElement>(`tr[data-index="${active.value}"]`);
    if (row) {
      row.focus({ preventScroll: true });
      return;
    }
    await new Promise((resolve) => requestAnimationFrame(resolve));
  }
}

function open(row: T | undefined): void {
  const target = row ? props.href(row) : null;
  if (target) void router.push(target);
}

function onKey(event: KeyboardEvent, index: number): void {
  const moves: Record<string, number> = {
    ArrowDown: index + 1,
    ArrowUp: index - 1,
    Home: 0,
    End: props.rows.length - 1,
    PageDown: index + 10,
    PageUp: index - 10,
  };
  if (event.key in moves) {
    event.preventDefault();
    void focusRow(moves[event.key]!);
  } else if (event.key === "Enter") {
    event.preventDefault();
    open(props.rows[index]);
  }
}
</script>

<template>
  <div ref="scroller" class="k-table-scroll">
    <table class="k-table" :aria-rowcount="done ? rows.length + 1 : -1">
      <caption class="k-visually-hidden">
        {{
          caption
        }}
      </caption>
      <thead>
        <tr aria-rowindex="1">
          <th v-for="column in columns" :key="column.key" scope="col">{{ column.label }}</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-if="before > 0"
          class="k-table-spacer"
          aria-hidden="true"
          :style="{ height: `${before}px` }"
        >
          <td :colspan="columns.length"></td>
        </tr>
        <tr
          v-for="item in virtual"
          :key="item.index"
          :data-index="item.index"
          :aria-rowindex="item.index + 2"
          :tabindex="item.index === active ? 0 : -1"
          class="k-table-row"
          @keydown="onKey($event, item.index)"
          @focus="active = item.index"
          @dblclick="open(rows[item.index])"
        >
          <td v-for="(column, position) in columns" :key="column.key">
            <RouterLink
              v-if="position === 0 && href(rows[item.index]!)"
              :to="href(rows[item.index]!)!"
              tabindex="-1"
            >
              {{ cell(rows[item.index]!, column.key) }}
            </RouterLink>
            <RouterLink
              v-else-if="link?.(rows[item.index]!, column.key)"
              :to="link(rows[item.index]!, column.key)!"
              tabindex="-1"
            >
              {{ cell(rows[item.index]!, column.key) }}
            </RouterLink>
            <template v-else>{{ cell(rows[item.index]!, column.key) }}</template>
          </td>
        </tr>
        <tr
          v-if="after > 0"
          class="k-table-spacer"
          aria-hidden="true"
          :style="{ height: `${after}px` }"
        >
          <td :colspan="columns.length"></td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
