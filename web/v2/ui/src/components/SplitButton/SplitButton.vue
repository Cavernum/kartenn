<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A split button (APG Button + Menu Button; design « Inventaire », line 32; REQ-020-37;
// 024/M9): the chosen action on the left, a chevron on the right that opens the other
// actions (`aria-haspopup="menu"`). Choosing an entry changes the action of the button
// without running it; the button runs it (`run`). An impossible action stays in the menu,
// disabled, its reason in its tooltip; chosen while impossible (the default of the host
// became so), the button itself is refused with that reason.
import { computed } from "vue";
import Button from "../Button/Button.vue";
import type { ButtonVariant } from "../Button/button.ts";
import type { ControlSize } from "../layout.ts";
import Menu from "../Menu/Menu.vue";
import type { MenuItem } from "../Menu/menu.ts";
import { t } from "../../i18n.ts";
import type { SplitAction } from "./split.ts";

const props = withDefaults(
  defineProps<{
    actions: SplitAction[];
    /** The name of the chevron (« Choisir l'action » by default). */
    menuLabel?: string | undefined;
    variant?: ButtonVariant;
    size?: ControlSize;
    loading?: boolean;
    /** The shortcut that runs the chosen action, shown on the button. */
    shortcut?: string[] | undefined;
    defaultOpen?: boolean;
  }>(),
  { menuLabel: undefined, variant: "primary", size: "md", shortcut: undefined },
);
/** The id of the chosen action (the first one when unset). */
const action = defineModel<string>("action", { default: "" });
const emit = defineEmits<{ run: [id: string] }>();

const chosen = computed(() => props.actions.find((a) => a.id === action.value) ?? props.actions[0]);
const items = computed<MenuItem[]>(() =>
  props.actions.map((a) => ({
    id: a.id,
    label: a.label,
    ...(a.icon ? { icon: a.icon } : {}),
    ...(a.disabled ? { disabled: true } : {}),
    ...(a.reason ? { reason: a.reason } : {}),
    ...(a.forbidden ? { forbidden: a.forbidden } : {}),
  })),
);
/** Why the chosen action is impossible, if it is. */
const blocked = computed(() => {
  const a = chosen.value;
  if (!a) return {};
  if (a.forbidden) return { forbidden: a.forbidden };
  if (a.disabled || a.reason) return { reason: a.reason ?? "" };
  return {};
});

function run(): void {
  if (chosen.value) emit("run", chosen.value.id);
}
</script>

<template>
  <div class="k-split-button" :data-size="props.size">
    <Button
      class="k-split-button__main"
      :variant="props.variant"
      :size="props.size"
      :icon="chosen?.icon"
      :loading="props.loading"
      :shortcut="props.shortcut"
      v-bind="blocked"
      @click="run"
      >{{ chosen?.label }}</Button
    >
    <Menu
      :label="props.menuLabel ?? t('chooseAction')"
      icon="chevron-down"
      icon-only
      align="end"
      :variant="props.variant"
      :size="props.size"
      :items="items"
      :default-open="props.defaultOpen"
      @select="action = $event"
    />
  </div>
</template>

<style>
@layer components {
  /* One control: the two halves joined, a hairline between them. */
  .k-split-button {
    display: inline-grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--k-border-width);
    min-inline-size: 0;
  }

  .k-split-button__main {
    min-inline-size: 0;
    border-start-end-radius: 0;
    border-end-end-radius: 0;
  }

  .k-split-button > .k-button:last-child {
    border-start-start-radius: 0;
    border-end-start-radius: 0;
  }
}
</style>
