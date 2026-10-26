<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A button with an icon alone (APG Button; design « Inventaire », line 3): square, of the
// heights of the controls; its name (`label`) is required by its type. The looks are
// those of `Button` (its sheet); `ghost` by default, as in toolbars and rows. Refused
// (`forbidden`, `reason`; 024/M14), as `Button`: focusable, `aria-disabled`, a tooltip.
import { computed } from "vue";
import { t } from "../../i18n.ts";
import type { IconName } from "../../icons/index.ts";
import Icon from "../Icon/Icon.vue";
import BlockedReason from "../internal/BlockedReason.vue";
import Tooltip from "../Tooltip/Tooltip.vue";
import { useTooltip } from "../Tooltip/tooltip.ts";
import { isBlocked, type ButtonBaseProps } from "../Button/button.ts";
// The sheet of `Button` (`.k-button`), which this button wears: loaded with it, so that an
// icon button alone (a page that has no `Button`) is not left with the browser's looks.
import "../Button/button.css";

const props = withDefaults(defineProps<ButtonBaseProps & { icon: IconName; label: string }>(), {
  variant: "ghost",
  size: "md",
  type: "button",
  pressed: undefined,
  forbidden: undefined,
  reason: undefined,
});
const emit = defineEmits<{ click: [event: MouseEvent] }>();

const blocked = computed(() => isBlocked(props));
const tip = useTooltip();
const listeners = tip.listeners(() => blocked.value);

function onClick(event: MouseEvent): void {
  if (props.loading || props.disabled || blocked.value) return;
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
    :aria-label="props.loading ? `${props.label} (${t('loading')})` : props.label"
    :aria-disabled="props.loading || blocked ? 'true' : undefined"
    :aria-busy="props.loading ? 'true' : undefined"
    :aria-pressed="props.pressed"
    :aria-describedby="blocked ? tip.id : undefined"
    :data-blocked="blocked ? 'true' : undefined"
    @click="onClick"
    v-on="listeners"
  >
    <Icon v-if="props.loading" class="k-spinner" name="loader-circle" />
    <Icon v-else :name="props.icon" />
    <Tooltip v-if="blocked" :control="tip">
      <BlockedReason :forbidden="props.forbidden" :reason="props.reason" />
    </Tooltip>
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
