<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The band shown away from the present (`role="status"`; design « Inventaire », line 27;
// spec 021, « Cadre »): full width, the band colours, the instant read with its named
// zone and the way back to the present.
import { t } from "../../i18n.ts";
import { formatInstant } from "../../time.ts";
import Button from "../Button/Button.vue";
import Icon from "../Icon/Icon.vue";

const props = defineProps<{ at: Date | string; timeZone: string }>();
const emit = defineEmits<{ present: [] }>();
</script>

<template>
  <div class="k-time-band">
    <p class="k-time-band__text" role="status">
      <Icon name="rotate-ccw-clock" />
      <span
        >{{ t("readingAt") }}
        <time :datetime="new Date(props.at).toISOString()">{{
          formatInstant(props.at, props.timeZone)
        }}</time>
        ({{ props.timeZone }})</span
      >
    </p>
    <Button
      size="sm"
      variant="ghost"
      icon="arrow-right"
      class="k-time-band__back"
      @click="emit('present')"
    >
      {{ t("backToPresent") }}
    </Button>
  </div>
</template>

<style>
@layer components {
  .k-time-band {
    display: flex;
    gap: var(--k-space-3);
    align-items: center;
    min-block-size: var(--k-control-height-lg);
    padding-inline: var(--k-space-4);
    color: var(--k-band-fg);
    background-color: var(--k-band-bg);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-time-band__text {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    margin: 0;
    font-weight: var(--k-weight-medium);
  }

  .k-time-band time {
    font-variant-numeric: tabular-nums;
  }

  .k-button.k-time-band__back {
    color: var(--k-band-fg);
    text-decoration: underline;
  }
}
</style>
