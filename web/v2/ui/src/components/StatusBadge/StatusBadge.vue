<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A state (design « Inventaire », line 24; REQ-020-7): its colour, its icon and its label,
// never the colour alone (WCAG 1.4.1). The icon is decorative: the label names the state;
// without one, the default word of the state (design « États »). In forced colours, the
// border, the icon and the label stay, in the colours of the system.
import type { IconName } from "../../icons/index.ts";
import { t, type MessageKey } from "../../i18n.ts";
import type { State } from "../../tokens/index.ts";
import Icon from "../Icon/Icon.vue";

const props = defineProps<{ state: State; label?: string | undefined }>();

const WORDS: Record<State, MessageKey> = {
  ok: "stateOk",
  warn: "stateWarn",
  ko: "stateKo",
  review: "stateReview",
  info: "stateInfo",
  neutral: "stateNeutral",
};

const ICONS: Record<State, IconName> = {
  ok: "circle-check",
  warn: "triangle-alert",
  ko: "circle-x",
  review: "eye",
  info: "info",
  neutral: "circle-dashed",
};
</script>

<template>
  <span class="k-status-badge" :data-state="props.state">
    <Icon :name="ICONS[props.state]" />{{ props.label ?? t(WORDS[props.state]) }}
  </span>
</template>

<style>
@layer components {
  .k-status-badge {
    --k-status-fg: var(--k-state-neutral-fg);
    --k-status-bg: var(--k-state-neutral-bg);
    --k-status-border: var(--k-state-neutral-border);

    display: inline-flex;
    flex: none;
    gap: var(--k-space-1);
    align-items: center;
    min-block-size: var(--k-space-5);
    padding-inline: var(--k-space-1) var(--k-space-2);
    font-family: var(--k-font-sans);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-status-fg);
    white-space: nowrap;
    background-color: var(--k-status-bg);
    border: var(--k-border-width) solid var(--k-status-border);
    border-radius: var(--k-radius-full);
  }

  .k-status-badge[data-state="ok"] {
    --k-status-fg: var(--k-state-ok-fg);
    --k-status-bg: var(--k-state-ok-bg);
    --k-status-border: var(--k-state-ok-border);
  }

  .k-status-badge[data-state="warn"] {
    --k-status-fg: var(--k-state-warn-fg);
    --k-status-bg: var(--k-state-warn-bg);
    --k-status-border: var(--k-state-warn-border);
  }

  .k-status-badge[data-state="ko"] {
    --k-status-fg: var(--k-state-ko-fg);
    --k-status-bg: var(--k-state-ko-bg);
    --k-status-border: var(--k-state-ko-border);
  }

  .k-status-badge[data-state="review"] {
    --k-status-fg: var(--k-state-review-fg);
    --k-status-bg: var(--k-state-review-bg);
    --k-status-border: var(--k-state-review-border);
  }

  .k-status-badge[data-state="info"] {
    --k-status-fg: var(--k-state-info-fg);
    --k-status-bg: var(--k-state-info-bg);
    --k-status-border: var(--k-state-info-border);
  }
}
</style>
