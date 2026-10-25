<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The date pill of the status bar (APG Button; design « Inventaire », line 27; spec 021,
// « Cadre »): « Présent », or the instant read, in the colours of the band, with its
// named time zone. It opens the period picker (`aria-haspopup="dialog"`).
import { computed } from "vue";
import { t } from "../../i18n.ts";
import { formatInstant } from "../../time.ts";
import Icon from "../Icon/Icon.vue";

const props = withDefaults(
  defineProps<{ at?: Date | string | null; timeZone: string; expanded?: boolean }>(),
  { at: null },
);
const emit = defineEmits<{ open: [] }>();

const past = computed(() => props.at !== null);
</script>

<template>
  <button
    type="button"
    class="k-date-pill"
    :data-state="past ? 'past' : 'present'"
    aria-haspopup="dialog"
    :aria-expanded="props.expanded"
    @click="emit('open')"
  >
    <Icon :name="past ? 'rotate-ccw-clock' : 'clock'" />
    <template v-if="props.at !== null">
      <time class="k-date-pill__date" :datetime="new Date(props.at).toISOString()">{{
        formatInstant(props.at, props.timeZone)
      }}</time>
      <span class="k-date-pill__zone">{{ props.timeZone }}</span>
    </template>
    <span v-else>{{ t("present") }}</span>
    <Icon name="chevron-down" class="k-date-pill__chevron" />
  </button>
</template>

<style>
@layer components {
  .k-date-pill {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    min-block-size: var(--k-control-height-md);
    padding-inline: var(--k-control-padding-inline-md) var(--k-control-padding-inline-sm);
    font: inherit;
    font-weight: var(--k-weight-medium);
    color: var(--k-text);
    white-space: nowrap;
    cursor: pointer;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-full);
    transition-property: background-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  /* The chevron turns while the picker is open (design « Mouvement »). */
  .k-date-pill__chevron {
    transition-property: rotate;
    transition-duration: var(--k-duration-short);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-date-pill[aria-expanded="true"] .k-date-pill__chevron {
    rotate: var(--k-turn-half);
  }

  .k-date-pill:hover {
    background-color: var(--k-hover);
  }

  .k-date-pill[data-state="past"] {
    color: var(--k-band-fg);
    background-color: var(--k-band-bg);
    border-color: var(--k-band-fg);
  }

  .k-date-pill__date {
    font-variant-numeric: tabular-nums;
  }

  .k-date-pill__zone {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-regular);
  }

  .k-date-pill[data-state="present"] .k-icon {
    color: var(--k-text-muted);
  }
}
</style>
