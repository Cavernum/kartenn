<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A typed filter (design « Inventaire », line 6; REQ-020-20): « property operator value »
// as one button that opens the picker (`FilterPicker`), and a button that removes it. A
// filter being written is a draft (dashed); an invalid one says so in its name and why in
// its description, an alert when it turns invalid. A property without a label shows its
// key, in fixed pitch (REQ-020-21).
import { computed, useId } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";

const props = withDefaults(
  defineProps<{
    property: string;
    operator: string;
    value?: string | undefined;
    /** The value is a key, an address, a prefix: fixed pitch. */
    mono?: boolean;
    /** The property is shown by its key (no label in the language): fixed pitch. */
    propertyMono?: boolean;
    state?: "applied" | "draft" | "invalid";
    /** Why the filter is invalid. */
    reason?: string | undefined;
    removeLabel?: string | undefined;
    /** Its picker is open (`aria-expanded` of the button that opens it). */
    expanded?: boolean | undefined;
  }>(),
  {
    value: undefined,
    state: "applied",
    reason: undefined,
    removeLabel: undefined,
    expanded: undefined,
  },
);
const emit = defineEmits<{ edit: []; remove: [] }>();
const reasonId = useId();

const sentence = computed(() =>
  [props.property, props.operator, props.value].filter(Boolean).join(" "),
);
</script>

<template>
  <span class="k-filter-chip">
    <span class="k-filter-chip__pill" :data-state="props.state">
      <button
        type="button"
        class="k-filter-chip__body"
        aria-haspopup="dialog"
        :aria-expanded="props.expanded"
        :aria-describedby="props.state === 'invalid' && props.reason ? reasonId : undefined"
        @click="emit('edit')"
      >
        <Icon v-if="props.state === 'invalid'" name="triangle-alert" class="k-filter-chip__warn" />
        <span class="k-filter-chip__property" :class="{ 'k-mono': props.propertyMono }">{{
          props.property
        }}</span>
        <span class="k-filter-chip__operator">{{ props.operator }}</span>
        <span v-if="props.value" class="k-filter-chip__value" :class="{ 'k-mono': props.mono }">{{
          props.value
        }}</span>
        <span v-if="props.state === 'draft'" class="k-visually-hidden">({{ t("inDraft") }})</span>
        <span v-else-if="props.state === 'invalid'" class="k-visually-hidden"
          >({{ t("invalid") }})</span
        >
      </button>
      <button
        type="button"
        class="k-filter-chip__remove"
        :aria-label="props.removeLabel ?? `${t('removeFilter')} ${sentence}`"
        @click="emit('remove')"
      >
        <Icon name="x" />
      </button>
    </span>
    <span
      v-if="props.state === 'invalid' && props.reason"
      :id="reasonId"
      class="k-filter-chip__reason"
      role="alert"
      >{{ props.reason }}</span
    >
  </span>
</template>

<style>
@layer components {
  .k-filter-chip {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    align-items: center;
    max-inline-size: 100%;
  }

  .k-filter-chip__pill {
    display: inline-flex;
    align-items: stretch;
    min-inline-size: 0;
    min-block-size: var(--k-control-height-sm);
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-md);
  }

  .k-filter-chip__pill[data-state="applied"] {
    background-color: var(--k-accent-soft);
    border-color: var(--k-state-info-border);
  }

  .k-filter-chip__pill[data-state="draft"] {
    border-style: dashed;
  }

  .k-filter-chip__pill[data-state="invalid"] {
    background-color: var(--k-state-ko-bg);
    border-color: var(--k-state-ko-fg);
  }

  .k-filter-chip__body,
  .k-filter-chip__remove {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    padding-inline: var(--k-control-padding-inline-sm);
    font: inherit;
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: inherit;
    cursor: pointer;
    background: none;
    border: 0;
    border-radius: var(--k-radius-md);
  }

  /* A long sentence goes to the line inside its chip, never wider than the row: the
   * property and the operator stay whole, the value wraps (structure before style). */
  .k-filter-chip__body {
    flex-wrap: wrap;
    min-inline-size: 0;
    /* One pixel: a single line keeps the height of the control (24 px, a mono value
     * included); wrapped lines keep off the border. */
    padding-block: var(--k-space-px);
    text-align: start;
  }

  .k-filter-chip__property,
  .k-filter-chip__operator {
    white-space: nowrap;
  }

  /* At least 24 × 24 px inside the border (WCAG 2.5.8): chips sit side by side. */
  .k-filter-chip__remove {
    justify-content: center;
    min-inline-size: var(--k-control-height-sm);
    min-block-size: var(--k-control-height-sm);
    padding-inline: var(--k-space-1);
    color: var(--k-text-muted);
    border-inline-start: var(--k-border-width) solid var(--k-border-strong);
    border-start-start-radius: 0;
    border-end-start-radius: 0;
  }

  .k-filter-chip__body:hover,
  .k-filter-chip__remove:hover {
    background-color: var(--k-hover);
  }

  .k-filter-chip__property {
    color: var(--k-text-muted);
  }

  .k-filter-chip__operator {
    color: var(--k-text-muted);
  }

  .k-filter-chip__value {
    font-weight: var(--k-weight-medium);
  }

  .k-filter-chip__property.k-mono,
  .k-filter-chip__value.k-mono {
    font-size: var(--k-text-sm);
  }

  .k-filter-chip__reason {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-state-ko-fg);
  }

  .k-filter-chip__warn {
    color: var(--k-state-ko-fg);
  }
}
</style>
