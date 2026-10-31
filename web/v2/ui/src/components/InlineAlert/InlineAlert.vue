<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// An inline message (design « Inventaire », line 26): the violations under a form, a
// warning above a table. `role="alert"` for an error (read at once), `status` otherwise.
// Colour, icon and title together (REQ-020-7); each violation shows its code, copyable
// (REQ-020-22). `icon` replaces the icon of the tone when it says more (a lock for an
// object read only); the colour and the role stay those of the tone.
// It is said when it shows: the region is there, its content hidden (`data-state=
// "waiting"`, `visibility: hidden`, out of the accessibility tree), and its content
// shows a frame later, which the region announces (a region inserted with its text is
// not announced, relecture du 2026-10-26). The box keeps its size meanwhile: no shift.
import { onBeforeUnmount, onMounted, ref } from "vue";
import type { IconName } from "../../icons/index.ts";
import { t } from "../../i18n.ts";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import Icon from "../Icon/Icon.vue";
import { TONE_ICONS, TONE_WORDS, type AlertTone } from "./alert.ts";

const props = withDefaults(
  defineProps<{
    tone?: AlertTone;
    title: string;
    /** Violations: a readable message and the code of each. */
    items?: { message: string; code?: string }[];
    /** An icon that says more than the tone's (e.g. `lock`). */
    icon?: IconName | undefined;
  }>(),
  { tone: "info", items: () => [], icon: undefined },
);

const said = ref(false);
let frame = 0;
onMounted(() => {
  frame = requestAnimationFrame(() => (said.value = true));
});
onBeforeUnmount(() => cancelAnimationFrame(frame));
</script>

<template>
  <div
    class="k-inline-alert"
    :data-tone="props.tone"
    :data-state="said ? undefined : 'waiting'"
    :role="props.tone === 'ko' ? 'alert' : 'status'"
  >
    <Icon :name="props.icon ?? TONE_ICONS[props.tone]" class="k-inline-alert__icon" />
    <div class="k-inline-alert__body">
      <p class="k-inline-alert__title">
        <span class="k-visually-hidden">{{ t(TONE_WORDS[props.tone]) }}{{ t("colon") }} </span
        >{{ props.title }}
      </p>
      <div v-if="$slots.default" class="k-inline-alert__text"><slot /></div>
      <ul v-if="props.items.length" class="k-inline-alert__items">
        <li v-for="(item, i) in props.items" :key="i" class="k-inline-alert__item">
          <span>{{ item.message }}</span>
          <CodeBadge v-if="item.code" :code="item.code" />
        </li>
      </ul>
    </div>
  </div>
</template>

<style>
@layer components {
  /* The tone, the icon, the body and the title: `tone.css`, shared with `Toast`. */
  .k-inline-alert {
    padding: var(--k-space-2) var(--k-space-3);
  }

  .k-inline-alert[data-state="waiting"] > * {
    visibility: hidden;
  }

  .k-inline-alert__items {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-1);
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .k-inline-alert__item {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
  }
}
</style>
