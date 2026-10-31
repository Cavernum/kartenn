<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A shortcut as the help `?`, a tooltip or a menu shows it (design « Inventaire », line
// 28): one `Kbd` per combination, the steps of a sequence joined by a word of the language
// (« g puis p »), so that a sequence never reads as one combination (g+p). Text, read as
// it shows: the keycaps' names in the language and the word between them.
import { computed } from "vue";
import { t } from "../../i18n.ts";
import Kbd from "../Kbd/Kbd.vue";
import { shortcutSteps, type Shortcut } from "./shortcutHint.ts";

const props = defineProps<{ keys: Shortcut }>();
const steps = computed(() => shortcutSteps(props.keys));
</script>

<template>
  <span class="k-shortcut-hint">
    <template v-for="(step, i) in steps" :key="i">
      <span v-if="i > 0" class="k-shortcut-hint__then">{{ t("then") }}</span>
      <Kbd :keys="step" />
    </template>
  </span>
</template>

<style>
@layer components {
  .k-shortcut-hint {
    display: inline-flex;
    flex-wrap: nowrap;
    gap: var(--k-space-1);
    align-items: center;
    white-space: nowrap;
  }

  .k-shortcut-hint__then {
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
    color: var(--k-text-muted);
  }

  /* Beside a control (hidden from its name, as `Kbd`): gone where the only pointer is a
   * finger and nothing hovers. */
  @media (hover: none) and (pointer: coarse) {
    .k-shortcut-hint[aria-hidden="true"] {
      display: none;
    }
  }
}
</style>
