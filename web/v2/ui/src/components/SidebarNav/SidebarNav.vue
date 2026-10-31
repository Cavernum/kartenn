<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The sidebar (Disclosure in a `navigation` landmark; design « Inventaire », line 18; spec
// 021, « Cadre », REQ-021-2): the brand, the palette button (Ctrl+K), the spaces as
// disclosures of entries (a space without entries is left out), the active entry
// `aria-current="page"`, the collapse toggle at the bottom (`[`; the shortcuts are the
// shell's, said here by `aria-keyshortcuts`). Collapsed (a model the host keeps), it
// shows icons only: every space open, each list named by its space, each control keeping
// its name (labels hidden on screen only, the counter in the name), and a tooltip after
// it on hover and keyboard focus (APG Tooltip) that repeats its name, its counter and its
// shortcut: decorative, Escape closes it and goes on.
import { computed, nextTick, onMounted, ref, useId, watch } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import LinkTo from "../internal/LinkTo.vue";
import Kbd from "../Kbd/Kbd.vue";
import ShortcutHint from "../ShortcutHint/ShortcutHint.vue";
import type { Shortcut } from "../ShortcutHint/shortcutHint.ts";
import Tooltip from "../Tooltip/Tooltip.vue";
import { useTooltip } from "../Tooltip/tooltip.ts";
import type { SidebarGroup } from "./sidebar.ts";

const props = withDefaults(
  defineProps<{
    brand: string;
    groups: SidebarGroup[];
    active?: string | undefined;
    paletteLabel?: string | undefined;
    /** Spaces closed at first. */
    closed?: string[];
  }>(),
  { active: undefined, paletteLabel: undefined, closed: () => [] },
);
const collapsed = defineModel<boolean>("collapsed", { default: false });
const emit = defineEmits<{ palette: [] }>();

const id = useId();
const closedGroups = ref(new Set(props.closed));
const isOpen = (group: string) => collapsed.value || !closedGroups.value.has(group);
function toggle(group: string): void {
  const next = new Set(closedGroups.value);
  if (next.has(group)) next.delete(group);
  else next.add(group);
  closedGroups.value = next;
}
const state = computed(() => (collapsed.value ? "collapsed" : "expanded"));
const groups = computed(() => props.groups.filter((group) => group.entries.length > 0));

/** The tooltip of the rail: one bubble, after the control under the pointer or the focus. */
interface Tip {
  label: string;
  badge?: number | undefined;
  shortcut?: Shortcut | undefined;
}
const tip = useTooltip();
const tipped = ref<Tip>();
const base = tip.listeners(() => collapsed.value, { pin: () => false, passEscape: () => true });
/** The listeners of a control of the rail, which say what its tooltip shows. */
function tipOf(item: Tip): Record<string, (event: Event) => void> {
  const set = (handler: ((event: Event) => void) | undefined) => (event: Event) => {
    tipped.value = item;
    handler?.(event);
  };
  return { ...base, pointerenter: set(base.pointerenter), focus: set(base.focus) };
}
watch(collapsed, (now) => {
  if (!now) tip.hide();
});
const paletteTip = computed<Tip>(() => ({
  label: props.paletteLabel ?? t("search"),
  shortcut: ["Mod", "k"],
}));
const collapseTip = computed<Tip>(() => ({ label: t("sidebarToggle"), shortcut: ["["] }));

// The current entry is always in view (relecture du 2026-10-26): in a rail every space is
// open, and the entries of the last ones fall under the fold of a short window. The list
// scrolls itself (never the page), the entry centred when it was out of view.
const scroller = ref<HTMLElement | null>(null);
async function revealCurrent(): Promise<void> {
  await nextTick();
  const list = scroller.value;
  const entry = list?.querySelector<HTMLElement>('[aria-current="page"]');
  if (!list || !entry) return;
  const top = entry.getBoundingClientRect().top - list.getBoundingClientRect().top + list.scrollTop;
  const bottom = top + entry.offsetHeight;
  if (top >= list.scrollTop && bottom <= list.scrollTop + list.clientHeight) return;
  list.scrollTop = top - (list.clientHeight - entry.offsetHeight) / 2;
}
onMounted(revealCurrent);
watch(() => [props.active, collapsed.value], revealCurrent);
</script>

<template>
  <nav class="k-sidebar" :data-state="state" :aria-label="t('sidebar')">
    <div class="k-sidebar__brand">
      <Icon name="network" :size="20" class="k-sidebar__logo" />
      <span class="k-sidebar__brand-name">{{ props.brand }}</span>
    </div>
    <button
      type="button"
      class="k-sidebar__palette"
      aria-keyshortcuts="Control+K"
      aria-haspopup="dialog"
      @click="emit('palette')"
      v-on="tipOf(paletteTip)"
    >
      <Icon name="search" />
      <span class="k-sidebar__label">{{ paletteTip.label }}</span>
      <Kbd :keys="['Mod', 'k']" aria-hidden="true" class="k-sidebar__kbd" />
    </button>
    <div ref="scroller" class="k-sidebar__groups">
      <section v-for="group in groups" :key="group.id" class="k-sidebar__group">
        <button
          v-if="!collapsed"
          type="button"
          class="k-sidebar__group-toggle"
          :aria-expanded="isOpen(group.id)"
          :aria-controls="`${id}-${group.id}`"
          @click="toggle(group.id)"
        >
          <Icon
            name="chevron-right"
            class="k-chevron"
            :data-open="isOpen(group.id) ? 'true' : undefined"
          />
          {{ group.label }}
        </button>
        <ul
          v-show="isOpen(group.id)"
          :id="`${id}-${group.id}`"
          class="k-sidebar__entries"
          :aria-label="collapsed ? group.label : undefined"
        >
          <li v-for="entry in group.entries" :key="entry.id">
            <LinkTo
              :to="entry.to"
              class="k-sidebar__entry"
              :aria-current="entry.id === props.active ? 'page' : undefined"
              v-on="tipOf(entry)"
            >
              <Icon :name="entry.icon" class="k-sidebar__icon" />
              <span class="k-sidebar__label">{{ entry.label }}</span>
              <span v-if="entry.badge" class="k-sidebar__badge">{{ entry.badge }}</span>
            </LinkTo>
          </li>
        </ul>
      </section>
    </div>
    <button
      type="button"
      class="k-sidebar__collapse"
      :aria-expanded="!collapsed"
      aria-keyshortcuts="["
      @click="collapsed = !collapsed"
      v-on="tipOf(collapseTip)"
    >
      <Icon name="panel-left" />
      <span class="k-sidebar__label">{{ collapseTip.label }}</span>
      <Kbd :keys="['[']" aria-hidden="true" class="k-sidebar__kbd" />
    </button>
    <!-- Decorative: it repeats the name of its control, mounted only while open; mounted
         again for each control, so that it is placed after the one it names. -->
    <Tooltip
      v-if="collapsed && tip.open.value && tipped"
      :key="tipped.label"
      :control="tip"
      side="inline-end"
      decorative
    >
      <span class="k-tooltip__name"
        >{{ tipped.label
        }}<span v-if="tipped.badge" class="k-sidebar__tip-badge">{{ tipped.badge }}</span
        ><ShortcutHint v-if="tipped.shortcut" :keys="tipped.shortcut"
      /></span>
    </Tooltip>
  </nav>
</template>

<style>
@layer components {
  .k-sidebar {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-2);
    inline-size: var(--k-size-sidebar);
    block-size: 100%;
    padding: var(--k-space-2);
    color: var(--k-sidebar-fg);
    background-color: var(--k-sidebar-bg);
    /* Inside the width of the bar (border-box): the light bar meets the white surface. */
    box-sizing: border-box;
    border-inline-end: var(--k-border-width) solid var(--k-border);
  }

  .k-sidebar[data-state="collapsed"] {
    inline-size: var(--k-size-sidebar-collapsed);
  }

  .k-sidebar__brand {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    min-block-size: var(--k-control-height-lg);
    padding-inline: var(--k-space-1);
    font-weight: var(--k-weight-semibold);
    color: var(--k-sidebar-active-fg);
  }

  .k-sidebar__logo {
    color: var(--k-sidebar-active-icon);
  }

  /* The list scrolls itself; room around its entries for their focus ring (2 px, 1 px
   * out), which the scrolling box would clip, its edges kept where they were. */
  .k-sidebar__groups {
    display: grid;
    gap: var(--k-space-3);
    align-content: start;
    padding: var(--k-space-1);
    margin: calc(var(--k-space-1) * -1);
    overflow-y: auto;
  }

  .k-sidebar__group {
    display: grid;
    gap: var(--k-space-0-5);
  }

  .k-sidebar__palette,
  .k-sidebar__collapse,
  .k-sidebar__group-toggle,
  .k-sidebar__entry {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    min-block-size: var(--k-control-height-md);
    padding-inline: var(--k-space-2);
    font: inherit;
    color: inherit;
    text-align: start;
    text-decoration: none;
    cursor: pointer;
    background: none;
    border: 0;
    border-radius: var(--k-radius-md);
  }

  .k-sidebar__palette {
    border: var(--k-border-width) solid var(--k-border-strong);
  }

  .k-sidebar__group-toggle {
    gap: var(--k-space-1);
    min-block-size: var(--k-control-height-sm);
    padding-inline: var(--k-space-1);
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-sidebar__entries {
    display: grid;
    gap: var(--k-space-0-5);
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .k-sidebar__palette:hover,
  .k-sidebar__collapse:hover,
  .k-sidebar__group-toggle:hover,
  .k-sidebar__entry:hover {
    color: var(--k-sidebar-active-fg);
    background-color: var(--k-sidebar-active-bg);
  }

  .k-sidebar__entry[aria-current="page"] {
    color: var(--k-sidebar-active-fg);
    background-color: var(--k-sidebar-active-bg);
    box-shadow: var(--k-shadow-inset-accent);
  }

  .k-sidebar__entry[aria-current="page"] .k-sidebar__icon {
    color: var(--k-sidebar-active-icon);
  }

  .k-sidebar :focus-visible {
    outline-color: var(--k-sidebar-active-icon);
  }

  .k-sidebar__label {
    flex: 1;
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* A counter: in an entry, and in the tooltip of the rail (in the portal, out of the bar,
   * in the accent as over the corner of its icon). */
  .k-sidebar__badge,
  .k-sidebar__tip-badge {
    min-inline-size: var(--k-space-5);
    padding-inline: var(--k-space-1);
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
    font-variant-numeric: tabular-nums;
    color: var(--k-sidebar-active-fg);
    text-align: center;
    background-color: var(--k-sidebar-active-bg);
    border-radius: var(--k-radius-full);
  }

  .k-sidebar__tip-badge {
    color: var(--k-accent-fg);
    background-color: var(--k-accent);
  }

  .k-sidebar__kbd .k-kbd__key {
    color: var(--k-sidebar-fg);
    background-color: var(--k-sidebar-bg);
    border-color: var(--k-border-strong);
  }

  /* Collapsed: icons only; labels stay for assistive technologies. */
  .k-sidebar[data-state="collapsed"] .k-sidebar__brand-name,
  .k-sidebar[data-state="collapsed"] .k-sidebar__kbd {
    display: none;
  }

  /* Collapsed, a counter stays, over the corner of its icon. */
  .k-sidebar[data-state="collapsed"] .k-sidebar__entry {
    position: relative;
  }

  .k-sidebar[data-state="collapsed"] .k-sidebar__badge {
    position: absolute;
    inset-block-start: 0;
    inset-inline-end: 0;
    min-inline-size: var(--k-space-4);
    padding-inline: var(--k-space-0-5);
    background-color: var(--k-accent);
    color: var(--k-accent-fg);
  }

  .k-sidebar[data-state="collapsed"] .k-sidebar__label {
    position: absolute;
    inline-size: var(--k-space-px);
    block-size: var(--k-space-px);
    overflow: hidden;
    clip-path: inset(50%);
  }

  .k-sidebar[data-state="collapsed"] .k-sidebar__entry,
  .k-sidebar[data-state="collapsed"] .k-sidebar__palette,
  .k-sidebar[data-state="collapsed"] .k-sidebar__collapse {
    justify-content: center;
    padding-inline: 0;
  }
}
</style>
