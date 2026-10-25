<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A code (design « Inventaire », line 22; REQ-020-22): fixed pitch, copied by a click or
// the keyboard (APG Button), the outcome announced by a `status` region. `label` names
// what the code is (« Request-Id »). The name is the content, « Copier » first, hidden on
// screen: it keeps the visible text (WCAG 2.5.3).
import { computed, onBeforeUnmount, ref } from "vue";
import { copyText, type CopyOutcome } from "../../clipboard.ts";
import { t } from "../../i18n.ts";
import Icon from "../Icon/Icon.vue";

const props = defineProps<{ code: string; label?: string }>();

/** How long the outcome stays shown, in milliseconds. */
const OUTCOME_MS = 2000;

const outcome = ref<CopyOutcome | null>(null);
let timer: ReturnType<typeof setTimeout> | undefined;

const announcement = computed(() =>
  outcome.value === "copied" ? t("copied") : outcome.value === "failed" ? t("copyFailed") : "",
);

async function copy(): Promise<void> {
  outcome.value = await copyText(props.code);
  clearTimeout(timer);
  timer = setTimeout(() => (outcome.value = null), OUTCOME_MS);
}

onBeforeUnmount(() => clearTimeout(timer));
</script>

<template>
  <span class="k-code-badge">
    <button
      type="button"
      class="k-code-badge__button"
      :data-state="outcome ?? undefined"
      @click="copy"
    >
      <span class="k-visually-hidden">{{ `${t("copy")} ` }}</span>
      <span v-if="props.label" class="k-code-badge__label">{{ `${props.label} ` }}</span>
      <code class="k-code-badge__code">{{ props.code }}</code>
      <Icon :name="outcome === 'copied' ? 'check' : outcome === 'failed' ? 'circle-x' : 'copy'" />
    </button>
    <span role="status" class="k-visually-hidden">{{ announcement }}</span>
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
    cursor: pointer;
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

  .k-code-badge__code {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}
</style>
