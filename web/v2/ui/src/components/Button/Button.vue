<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A button (APG Button; design « Inventaire », line 2): four variants, three sizes shared
// with the fields (REQ-020-17), an optional leading icon and shortcut. Busy, it keeps the
// focus (`aria-disabled`, `aria-busy`) and ignores activation; disabled, it leaves the
// tab order (native `disabled`). Refused by the rights or a policy (`forbidden`, `reason`;
// 024/M14), it stays visible and focusable, `aria-disabled`, its reason in a tooltip on
// hover and focus and in its description. Its sheet is `button.css`, shared with
// `IconButton`.
import { computed } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import Kbd from "../Kbd/Kbd.vue";
import BlockedReason from "../internal/BlockedReason.vue";
import Tooltip from "../Tooltip/Tooltip.vue";
import { useTooltip } from "../Tooltip/tooltip.ts";
import "./button.css";
import { isBlocked, type ButtonProps } from "./button.ts";
import { ariaKeyshortcuts } from "./shortcut.ts";

const props = withDefaults(defineProps<ButtonProps>(), {
  variant: "secondary",
  size: "md",
  type: "button",
  pressed: undefined,
  forbidden: undefined,
  reason: undefined,
});
const emit = defineEmits<{ click: [event: MouseEvent] }>();

const keyshortcuts = computed(() =>
  props.shortcut ? ariaKeyshortcuts(props.shortcut) : undefined,
);
const blocked = computed(() => isBlocked(props));
const tip = useTooltip();
const listeners = tip.listeners(() => blocked.value);

function onClick(event: MouseEvent): void {
  // Busy or refused, a submit button submits nothing either.
  if (props.loading || props.disabled || blocked.value) return event.preventDefault();
  emit("click", event);
}
</script>

<template>
  <button
    class="k-button"
    :type="props.type"
    :data-variant="props.variant"
    :data-size="props.size"
    :disabled="props.disabled && !blocked"
    :aria-disabled="props.loading || blocked ? 'true' : undefined"
    :aria-busy="props.loading ? 'true' : undefined"
    :aria-pressed="props.pressed"
    :aria-keyshortcuts="keyshortcuts"
    :aria-describedby="blocked ? tip.id : undefined"
    :data-blocked="blocked ? 'true' : undefined"
    @click="onClick"
    v-on="listeners"
  >
    <Icon v-if="props.loading" class="k-spinner" name="loader-circle" />
    <Icon v-else-if="props.icon" :name="props.icon" />
    <span class="k-button__label"
      ><slot /><span v-if="props.loading" class="k-visually-hidden">
        ({{ t("loading") }})</span
      ></span
    >
    <Kbd v-if="props.shortcut" :keys="props.shortcut" aria-hidden="true" />
    <Tooltip v-if="blocked" :control="tip">
      <BlockedReason :forbidden="props.forbidden" :reason="props.reason" />
    </Tooltip>
  </button>
</template>
