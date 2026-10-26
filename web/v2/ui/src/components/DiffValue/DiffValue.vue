<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A value changed by a draft, inline (design « Inventaire », family of `DiffList`; relecture
// du 2026-10-26): `added` and `removed` put their sign (+, −) before the value, attached to
// it — sign and value on one row that never breaks between them, the value wrapping inside
// its own box —, with the word hidden on screen (never the colour alone); `changed` shows
// the value before (slot `before`) then, after an arrow, the value after. The values are
// the host's: text, a badge, a code.
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import { DIFF_SIGNS } from "../DiffList/diff.ts";

const props = defineProps<{ kind: "added" | "removed" | "changed" }>();
</script>

<template>
  <span v-if="props.kind === 'changed'" class="k-diff-value" data-kind="changed">
    <del class="k-diff-value__part"
      ><span class="k-visually-hidden">{{ t("before") }} : </span><slot name="before"
    /></del>
    <Icon name="arrow-right" class="k-diff-value__arrow" />
    <ins class="k-diff-value__part"
      ><span class="k-visually-hidden">{{ t("after") }} : </span><slot
    /></ins>
  </span>
  <component
    :is="props.kind === 'added' ? 'ins' : 'del'"
    v-else
    class="k-diff-value"
    :data-kind="props.kind"
    ><span class="k-diff-value__sign" aria-hidden="true">{{ DIFF_SIGNS[props.kind] }}</span
    ><span class="k-diff-value__value"
      ><span class="k-visually-hidden">{{ t(props.kind) }} : </span><slot /></span
  ></component>
</template>

<style>
@layer components {
  /* One row: the sign stays with the start of its value; the value wraps in its box. */
  .k-diff-value {
    display: inline-flex;
    flex-wrap: nowrap;
    gap: var(--k-space-1);
    align-items: baseline;
    max-inline-size: 100%;
    min-inline-size: 0;
    text-decoration: none;
  }

  /* Before → after: each part whole, the after part on the next line when room runs out. */
  .k-diff-value[data-kind="changed"] {
    flex-wrap: wrap;
    align-items: center;
  }

  .k-diff-value__part {
    display: inline-flex;
    min-inline-size: 0;
    text-decoration: none;
  }

  /* The value before, muted (a badge keeps its own colours). */
  del.k-diff-value__part {
    color: var(--k-text-muted);
  }

  .k-diff-value__sign {
    flex: none;
    font-family: var(--k-font-mono);
    font-weight: var(--k-weight-semibold);
  }

  .k-diff-value[data-kind="added"] > .k-diff-value__sign {
    color: var(--k-diff-added);
  }

  .k-diff-value[data-kind="removed"] > .k-diff-value__sign {
    color: var(--k-diff-removed);
  }

  .k-diff-value__value {
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }

  .k-diff-value__arrow {
    flex: none;
    color: var(--k-text-muted);
  }
}
</style>
