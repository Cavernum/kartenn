<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A code (design « Inventaire », line 22; REQ-020-22): fixed pitch, copied by a click or
// the keyboard (APG Button), the outcome announced by a `status` region and shown in its
// bubble (« Copié » for a while; the clipboard refused, the code selected and how to copy
// it by hand). `label` names what the code is (« Request-Id »). The name is the content,
// « Copier » first, hidden on screen: it keeps the visible text (WCAG 2.5.3). A code cut
// by the width shows whole in the bubble, under the pointer and the keyboard focus.
import { computed, ref } from "vue";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";
import { useCopy } from "../internal/copy.ts";
import Tooltip from "../Tooltip/Tooltip.vue";

const props = defineProps<{ code: string; label?: string }>();

const shown = ref<HTMLElement | null>(null);
const isCut = () => !!shown.value && shown.value.scrollWidth > shown.value.clientWidth;
const { tip, outcome, listeners } = useCopy({
  text: () => props.code,
  selection: () => shown.value,
  // Cut, the bubble shows more than the button: Escape closes it alone (1.4.13).
  atRest: isCut,
});

const said = computed(() =>
  outcome.value === "copied" ? t("copied") : outcome.value === "failed" ? t("copyByHand") : "",
);
const icon = computed(() =>
  outcome.value === "copied" ? "check" : outcome.value === "failed" ? "circle-x" : "copy",
);
</script>

<template>
  <span class="k-code-badge">
    <button
      type="button"
      class="k-code-badge__button"
      :data-state="outcome ?? undefined"
      v-on="listeners"
    >
      <span class="k-visually-hidden">{{ `${t("copy")} ` }}</span>
      <span v-if="props.label" class="k-code-badge__label">{{ `${props.label} ` }}</span>
      <code ref="shown" class="k-code-badge__code">{{ props.code }}</code>
      <Icon :name="icon" />
    </button>
    <span role="status" class="k-visually-hidden">{{ said }}</span>
    <Tooltip v-if="tip.open.value && (outcome || isCut())" :control="tip" decorative>
      <code v-if="isCut()" class="k-tooltip__name k-code-badge__whole">{{ props.code }}</code>
      <span v-if="outcome" class="k-tooltip__detail">{{ said }}</span>
    </Tooltip>
  </span>
</template>

<style>
@layer components {
  .k-code-badge {
    display: inline-flex;
    min-inline-size: 0;
    max-inline-size: 100%;
  }

  .k-code-badge__button {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    min-inline-size: 0;
    min-block-size: var(--k-control-height-sm);
    padding-inline: var(--k-control-padding-inline-sm);
    font: inherit;
    color: var(--k-text);
    cursor: copy;
    background-color: var(--k-surface-sunken);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
    transition-property: background-color, border-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-code-badge__button:hover {
    background-color: var(--k-hover);
    border-color: var(--k-border-control);
  }

  .k-code-badge__button .k-icon {
    color: var(--k-text-muted);
  }

  .k-code-badge__button[data-state="copied"] .k-icon {
    color: var(--k-state-ok-fg);
  }

  .k-code-badge__button[data-state="failed"] .k-icon {
    color: var(--k-state-ko-fg);
  }

  .k-code-badge__label {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
  }

  /* The code, selectable for a copy by hand (a button's text is not, by default). */
  .k-code-badge__code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }

  /* Whole in the bubble: a long code breaks anywhere rather than overflow it. */
  .k-code-badge__whole {
    overflow-wrap: anywhere;
  }
}
</style>
