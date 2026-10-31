<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The band shown away from the present (`role="status"`; design « Inventaire », line 27;
// spec 021, « Cadre », REQ-021-5): full width, the band colours, the dated context in
// words with its named zone, and the way back to the present. Two cases (007/W9): a
// validity date alone (`valid_at`) reads « Lecture au <date> — brouillon daté possible »,
// never saying that writes are off (a dated draft stays allowed); a past system date
// (`as_of`, alone or with `valid_at`) reads « État connu au <date> — lecture seule ».
// Nothing is shown at the present (both dates unset): only its status region, empty.
import { computed } from "vue";
import { t } from "../../i18n.ts";
import { formatInstant } from "../../time.ts";
import Button from "../Button/Button.vue";
import Icon from "../Icon/Icon.vue";

const props = withDefaults(
  defineProps<{
    validAt?: Date | string | null;
    asOf?: Date | string | null;
    timeZone: string;
  }>(),
  { validAt: null, asOf: null },
);
const emit = defineEmits<{ present: [] }>();

const readOnly = computed(() => props.asOf !== null);
const present = computed(() => props.validAt === null && props.asOf === null);

/** The sentence: words, and each date a `<time>` (its instant, machine-readable). */
type Part = string | { iso: string; text: string };
const parts = computed<Part[]>(() => {
  const date = (at: Date | string): Part => ({
    iso: new Date(at).toISOString(),
    text: formatInstant(at, props.timeZone),
  });
  const out: Part[] = [];
  if (props.validAt !== null) out.push(`${t("readingAt")} `, date(props.validAt));
  if (props.asOf !== null)
    out.push(
      props.validAt !== null ? `, ${t("knownAt")} ` : `${t("bandKnown")} `,
      date(props.asOf),
    );
  out.push(` (${props.timeZone}) — ${t(readOnly.value ? "bandReadOnly" : "bandDated")}`);
  return out;
});
</script>

<template>
  <!-- At the present, the status region stays, empty and unseen: the band said when the
       read leaves the present (a region inserted with its content is often not read). -->
  <div class="k-time-band" :data-state="present ? 'present' : readOnly ? 'known' : 'valid'">
    <p class="k-time-band__text" role="status">
      <template v-if="!present">
        <Icon :name="readOnly ? 'lock' : 'rotate-ccw-clock'" />
        <span>
          <template v-for="(part, i) in parts" :key="i">
            <time v-if="typeof part !== 'string'" :datetime="part.iso">{{ part.text }}</time>
            <template v-else>{{ part }}</template>
          </template>
        </span>
      </template>
    </p>
    <Button
      v-if="!present"
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
    flex-wrap: wrap;
    gap: var(--k-space-1) var(--k-space-3);
    align-items: center;
    min-block-size: var(--k-control-height-lg);
    padding-inline: var(--k-space-4);
    color: var(--k-band-fg);
    background-color: var(--k-band-bg);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  /* At the present: no band, its empty status region alone (no box). */
  .k-time-band[data-state="present"] {
    display: contents;
  }

  /* A sentence goes to the line in a narrow frame, never cut. */
  .k-time-band__text {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
    margin: 0;
    font-weight: var(--k-weight-medium);
  }

  .k-time-band__text > .k-icon {
    flex: none;
  }

  .k-time-band time {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .k-button.k-time-band__back {
    color: var(--k-band-fg);
    text-decoration: underline;
  }
}
</style>
