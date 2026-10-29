<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A button with an icon alone (APG Button; design « Inventaire », line 3): square, of the
// heights of the controls; its name (`label`) is required by its type. The looks are
// those of `Button` (its sheet); `ghost` by default, as in toolbars and rows. Its
// tooltip comes by itself (APG Tooltip, `useTooltip`): its name, and its shortcut if it
// has one, on hover and keyboard focus; decorative (the name is already `aria-label`),
// mounted only while open. Refused (`forbidden`, `reason`; 024/M14), as `Button`:
// focusable, `aria-disabled`, and the reason under the name in the tooltip, which is then
// the button's description (`aria-describedby` to the reason alone) and opens on a tap.
import { computed } from "vue";
import { t } from "../../i18n.ts";
import type { IconName } from "../../icons/index.ts";
import Icon from "../Icon/Icon.vue";
import BlockedReason from "../internal/BlockedReason.vue";
import ShortcutHint from "../ShortcutHint/ShortcutHint.vue";
import Tooltip from "../Tooltip/Tooltip.vue";
import { useTooltip } from "../Tooltip/tooltip.ts";
import { isBlocked, type ButtonBaseProps } from "../Button/button.ts";
import { ariaKeyshortcuts } from "../Button/shortcut.ts";
// The sheet of `Button` (`.k-button`), which this button wears: loaded with it, so that an
// icon button alone (a page that has no `Button`) is not left with the browser's looks.
import "../Button/button.css";

const props = withDefaults(
  defineProps<
    ButtonBaseProps & {
      icon: IconName;
      label: string;
      /** Its shortcut, one combination as `KeyboardEvent.key` names the keys: in its tooltip. */
      shortcut?: string[] | undefined;
    }
  >(),
  {
    variant: "ghost",
    size: "md",
    type: "button",
    pressed: undefined,
    forbidden: undefined,
    reason: undefined,
    shortcut: undefined,
  },
);
const emit = defineEmits<{ click: [event: MouseEvent] }>();

const blocked = computed(() => isBlocked(props));
const keyshortcuts = computed(() =>
  props.shortcut ? ariaKeyshortcuts(props.shortcut) : undefined,
);
const tip = useTooltip();
const reasonId = `${tip.id}-reason`;
// Its name alone, the tooltip leaves Escape to the focused button's layer or grid.
const listeners = tip.listeners(() => true, {
  pin: () => blocked.value,
  passEscape: () => !blocked.value,
});

function onClick(event: MouseEvent): void {
  // Busy or refused, a submit button submits nothing either.
  if (props.loading || props.disabled || blocked.value) return event.preventDefault();
  emit("click", event);
}
</script>

<template>
  <button
    class="k-button k-icon-button"
    :type="props.type"
    :data-variant="props.variant"
    :data-size="props.size"
    :disabled="props.disabled && !blocked"
    :aria-label="props.loading ? `${props.label} (${t('loading')})` : props.label"
    :aria-disabled="props.loading || blocked ? 'true' : undefined"
    :aria-busy="props.loading ? 'true' : undefined"
    :aria-pressed="props.pressed"
    :aria-keyshortcuts="keyshortcuts"
    :aria-describedby="blocked ? reasonId : undefined"
    :data-blocked="blocked ? 'true' : undefined"
    @click="onClick"
    v-on="listeners"
  >
    <Icon v-if="props.loading" class="k-spinner" name="loader-circle" />
    <Icon v-else :name="props.icon" />
    <Tooltip v-if="blocked || tip.open.value" :control="tip" :decorative="!blocked">
      <span class="k-tooltip__name" :aria-hidden="blocked ? 'true' : undefined"
        >{{ props.label }}<ShortcutHint v-if="props.shortcut" :keys="props.shortcut"
      /></span>
      <span v-if="blocked" :id="reasonId" class="k-tooltip__detail">
        <BlockedReason :forbidden="props.forbidden" :reason="props.reason" />
      </span>
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
