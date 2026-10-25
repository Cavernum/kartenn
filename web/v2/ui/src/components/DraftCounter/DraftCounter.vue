<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The draft counter of the status bar (design « Inventaire », line 27; spec 021, « Cadre »;
// 024/M8): « Brouillon (n) », always shown, a toggle of the right panel on its draft tab
// (`aria-expanded`, `aria-controls`); muted at zero. Expanded, its draft icon becomes a
// cross: what the next press does, the panel being open (revue du 2026-10-25). Beside it, when the host lists the
// drafts of the user, a chevron opens them (APG Menu Button); choosing one emits `select`.
import { computed } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import Menu from "../Menu/Menu.vue";
import type { MenuItem } from "../Menu/menu.ts";
import type { DraftSummary } from "./draft.ts";

const props = withDefaults(
  defineProps<{
    count: number;
    expanded?: boolean;
    controls?: string | undefined;
    drafts?: DraftSummary[] | undefined;
    defaultOpen?: boolean;
  }>(),
  { controls: undefined, drafts: undefined },
);
const emit = defineEmits<{ toggle: []; select: [id: string] }>();

const items = computed<MenuItem[]>(() => [
  { kind: "label", label: t("drafts") },
  ...(props.drafts ?? []).map((draft) => ({
    id: draft.id,
    label: `${draft.label} (${draft.count})`,
    icon: draft.current ? ("check" as const) : ("draft" as const),
  })),
]);
</script>

<template>
  <div class="k-draft-counter" :data-state="props.count === 0 ? 'empty' : 'pending'">
    <button
      type="button"
      class="k-draft-counter__toggle"
      :aria-expanded="props.expanded ? 'true' : 'false'"
      :aria-controls="props.controls"
      @click="emit('toggle')"
    >
      <Icon :name="props.expanded ? 'x' : 'draft'" />
      <span class="k-draft-counter__word">{{ t("draft") }}</span>
      <span class="k-draft-counter__count"
        ><span class="k-visually-hidden">(</span>{{ props.count
        }}<span class="k-visually-hidden">)</span></span
      >
    </button>
    <Menu
      v-if="props.drafts"
      :label="t('draftsList')"
      icon="chevron-down"
      icon-only
      variant="secondary"
      align="end"
      :items="items"
      :default-open="props.defaultOpen"
      @select="emit('select', $event)"
    />
  </div>
</template>

<style>
@layer components {
  .k-draft-counter {
    display: inline-flex;
    gap: var(--k-border-width);
    align-items: stretch;
  }

  .k-draft-counter__toggle {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    min-block-size: var(--k-control-height-md);
    padding-inline: var(--k-control-padding-inline-md) var(--k-control-padding-inline-sm);
    font: inherit;
    font-weight: var(--k-weight-medium);
    color: var(--k-text);
    cursor: pointer;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-md);
    transition-property: background-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  /* With the chevron, the two halves join. */
  .k-draft-counter__toggle:not(:only-child) {
    border-start-end-radius: 0;
    border-end-end-radius: 0;
  }

  .k-draft-counter > .k-button {
    border-start-start-radius: 0;
    border-end-start-radius: 0;
  }

  .k-draft-counter__toggle:hover,
  .k-draft-counter__toggle[aria-expanded="true"] {
    background-color: var(--k-hover);
  }

  .k-draft-counter__toggle[aria-expanded="true"] {
    box-shadow: var(--k-shadow-inset-accent);
  }

  /* In the compact frame (its `data-width`), the icon and the count: the word stays in the
   * name, hidden on screen only. */
  [data-width="compact"] .k-draft-counter__word {
    position: absolute;
    inline-size: var(--k-space-px);
    block-size: var(--k-space-px);
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  .k-draft-counter__count {
    min-inline-size: var(--k-space-5);
    padding-inline: var(--k-space-1);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-variant-numeric: tabular-nums;
    color: var(--k-accent-fg);
    text-align: center;
    background-color: var(--k-accent);
    border-radius: var(--k-radius-full);
  }

  .k-draft-counter[data-state="empty"] .k-draft-counter__toggle {
    color: var(--k-text-muted);
  }

  .k-draft-counter[data-state="empty"] .k-draft-counter__count {
    color: var(--k-text-muted);
    background-color: var(--k-surface-sunken);
  }
}
</style>
