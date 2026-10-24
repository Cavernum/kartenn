<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A button with an icon alone (APG Button; design « Inventaire », line 3): square, of the
// heights of the controls; its name (`label`) is required by its type. The looks are
// those of `Button` (its sheet); `ghost` by default, as in toolbars and rows.
import type { IconName } from "../../icons/index.ts";
import Icon from "../Icon/Icon.vue";
import type { ButtonBaseProps } from "../Button/button.ts";

const props = withDefaults(defineProps<ButtonBaseProps & { icon: IconName; label: string }>(), {
  variant: "ghost",
  size: "md",
  type: "button",
  pressed: undefined,
});
const emit = defineEmits<{ click: [event: MouseEvent] }>();

function onClick(event: MouseEvent): void {
  if (props.loading || props.disabled) return;
  emit("click", event);
}
</script>

<template>
  <button
    class="k-button k-icon-button"
    :type="props.type"
    :data-variant="props.variant"
    :data-size="props.size"
    :disabled="props.disabled"
    :aria-label="props.label"
    :aria-disabled="props.loading ? 'true' : undefined"
    :aria-busy="props.loading ? 'true' : undefined"
    :aria-pressed="props.pressed"
    @click="onClick"
  >
    <Icon v-if="props.loading" class="k-spinner" name="loader-circle" />
    <Icon v-else :name="props.icon" />
  </button>
</template>

<style>
@layer components {
  .k-button.k-icon-button {
    inline-size: var(--k-control-height-md);
    padding-inline: 0;
  }

  .k-button.k-icon-button[data-size="sm"] {
    inline-size: var(--k-control-height-sm);
    padding-inline: 0;
  }

  .k-button.k-icon-button[data-size="lg"] {
    inline-size: var(--k-control-height-lg);
    padding-inline: 0;
  }
}
</style>
