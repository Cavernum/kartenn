<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A differential grouped by object (Disclosure; design « Inventaire », line 29): each
// object is a disclosure of its changes, attribute by attribute, before and after; each
// change has its sign (+, −, ~, !), its word (hidden on screen) and its colour.
// Virtualisation beyond 200 lines comes with the complete version.
import { ref, useId } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import { DIFF_SIGNS, type DiffGroup, type DiffKind } from "./diff.ts";

const props = withDefaults(defineProps<{ groups: DiffGroup[]; collapsed?: string[] }>(), {
  collapsed: () => [],
});

const id = useId();
const closed = ref(new Set(props.collapsed));
function toggle(group: string): void {
  const next = new Set(closed.value);
  if (next.has(group)) next.delete(group);
  else next.add(group);
  closed.value = next;
}
const word = (kind: DiffKind) => t(kind);
</script>

<template>
  <div class="k-diff-list">
    <section v-for="group in props.groups" :key="group.id" class="k-diff-list__group">
      <h3 class="k-diff-list__heading">
        <button
          type="button"
          class="k-diff-list__toggle"
          :aria-expanded="!closed.has(group.id)"
          :aria-controls="`${id}-${group.id}`"
          @click="toggle(group.id)"
        >
          <Icon :name="closed.has(group.id) ? 'chevron-right' : 'chevron-down'" />
          <span class="k-diff-list__sign" :data-kind="group.kind" aria-hidden="true">{{
            DIFF_SIGNS[group.kind]
          }}</span>
          <span class="k-visually-hidden">{{ word(group.kind) }} :</span>
          <span class="k-diff-list__title">{{ group.title }}</span>
          <code v-if="group.objectKey" class="k-diff-list__key">{{ group.objectKey }}</code>
          <span class="k-diff-list__count">{{ group.changes.length }}</span>
        </button>
      </h3>
      <ul v-show="!closed.has(group.id)" :id="`${id}-${group.id}`" class="k-diff-list__changes">
        <li
          v-for="change in group.changes"
          :key="change.attribute"
          class="k-diff-list__change"
          :data-kind="change.kind"
        >
          <span class="k-diff-list__sign" :data-kind="change.kind" aria-hidden="true">{{
            DIFF_SIGNS[change.kind]
          }}</span>
          <span class="k-visually-hidden">{{ word(change.kind) }} :</span>
          <span class="k-diff-list__attribute">{{ change.attribute }}</span>
          <span class="k-diff-list__values" :class="{ 'k-mono': change.mono }">
            <del v-if="change.before !== undefined" class="k-diff-list__before"
              ><span class="k-visually-hidden">{{ t("before") }} : </span>{{ change.before }}</del
            >
            <Icon
              v-if="change.before !== undefined && change.after !== undefined"
              name="arrow-right"
              class="k-diff-list__arrow"
            />
            <ins v-if="change.after !== undefined" class="k-diff-list__after"
              ><span class="k-visually-hidden">{{ t("after") }} : </span>{{ change.after }}</ins
            >
          </span>
        </li>
      </ul>
    </section>
  </div>
</template>

<style>
@layer components {
  .k-diff-list {
    display: grid;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  .k-diff-list__group + .k-diff-list__group {
    border-block-start: var(--k-border-width) solid var(--k-border);
  }

  .k-diff-list__heading {
    margin: 0;
    font: inherit;
  }

  .k-diff-list__toggle {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    inline-size: 100%;
    min-block-size: var(--k-row-height);
    padding-inline: var(--k-space-2) var(--k-cell-padding-inline);
    font: inherit;
    font-weight: var(--k-weight-medium);
    color: var(--k-text);
    text-align: start;
    cursor: pointer;
    background-color: var(--k-surface-sunken);
    border: 0;
  }

  .k-diff-list__toggle:hover {
    background-color: var(--k-hover);
  }

  .k-diff-list__key {
    color: var(--k-text-muted);
  }

  .k-diff-list__count {
    margin-inline-start: auto;
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
    font-variant-numeric: tabular-nums;
    color: var(--k-text-muted);
  }

  .k-diff-list__sign {
    inline-size: var(--k-space-3);
    font-family: var(--k-font-mono);
    font-weight: var(--k-weight-semibold);
    text-align: center;
  }

  .k-diff-list__sign[data-kind="added"] {
    color: var(--k-diff-added);
  }

  .k-diff-list__sign[data-kind="removed"] {
    color: var(--k-diff-removed);
  }

  .k-diff-list__sign[data-kind="changed"] {
    color: var(--k-diff-changed);
  }

  .k-diff-list__sign[data-kind="anomaly"] {
    color: var(--k-state-review-fg);
  }

  .k-diff-list__changes {
    display: grid;
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .k-diff-list__change {
    display: grid;
    grid-template-columns: var(--k-space-3) var(--k-size-col-md) minmax(0, 1fr);
    gap: var(--k-space-2);
    align-items: center;
    min-block-size: var(--k-row-height);
    padding-inline: var(--k-space-8) var(--k-cell-padding-inline);
    border-block-start: var(--k-border-width) solid var(--k-border);
  }

  .k-diff-list__attribute {
    color: var(--k-text-muted);
  }

  .k-diff-list__values {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
  }

  .k-diff-list__before {
    color: var(--k-text-muted);
    text-decoration: line-through;
  }

  .k-diff-list__after {
    text-decoration: none;
  }

  .k-diff-list__change[data-kind="added"] .k-diff-list__after {
    color: var(--k-diff-added);
  }

  .k-diff-list__change[data-kind="removed"] .k-diff-list__before {
    color: var(--k-diff-removed);
  }

  .k-diff-list__arrow {
    color: var(--k-text-muted);
  }
}
</style>
