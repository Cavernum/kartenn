<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The sidebar (Disclosure in a `navigation` landmark; design « Inventaire », line 18; spec
// 021, « Cadre »): the brand, the palette button (Ctrl+K), the spaces as disclosures of
// entries, the active entry `aria-current="page"`, the collapse toggle at the bottom (`[`).
// Collapsed, it shows icons only: each entry keeps its name, the labels are hidden on
// screen only (the tooltips come with the complete version, task 23).
import { computed, ref, useId } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import LinkTo from "../internal/LinkTo.vue";
import Kbd from "../Kbd/Kbd.vue";
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
      @click="emit('palette')"
    >
      <Icon name="search" />
      <span class="k-sidebar__label">{{ props.paletteLabel ?? t("search") }}</span>
      <Kbd :keys="['Mod', 'k']" aria-hidden="true" class="k-sidebar__kbd" />
    </button>
    <div class="k-sidebar__groups">
      <section v-for="group in props.groups" :key="group.id" class="k-sidebar__group">
        <button
          v-if="!collapsed"
          type="button"
          class="k-sidebar__group-toggle"
          :aria-expanded="isOpen(group.id)"
          :aria-controls="`${id}-${group.id}`"
          @click="toggle(group.id)"
        >
          <Icon :name="isOpen(group.id) ? 'chevron-down' : 'chevron-right'" />
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
    >
      <Icon name="panel-left" />
      <span class="k-sidebar__label">{{ t("sidebarToggle") }}</span>
      <Kbd :keys="['[']" aria-hidden="true" class="k-sidebar__kbd" />
    </button>
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

  .k-sidebar__groups {
    display: grid;
    gap: var(--k-space-3);
    align-content: start;
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
    border: var(--k-border-width) solid var(--k-sidebar-active-bg);
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

  .k-sidebar__badge {
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

  .k-sidebar__kbd .k-kbd__key {
    color: var(--k-sidebar-fg);
    background-color: var(--k-sidebar-bg);
    border-color: var(--k-sidebar-active-bg);
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
