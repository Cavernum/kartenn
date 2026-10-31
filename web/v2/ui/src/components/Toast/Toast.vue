<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// One toast (design « Inventaire », line 14), in its `ToastRegion`, which says it: the
// outcome of an action, short (a title, a sentence, a code, one action) and its close
// button. Colour, icon and tone in words (hidden, before the title; WCAG 1.3.1). It goes
// after its duration, its time held while `paused` (the pointer over the toasts, the
// focus in them: WCAG 2.2.1); an error, a warning or a toast with an action has none and
// stays until dismissed (`createToasts`).
import { onBeforeUnmount, onMounted, watch } from "vue";
import { t } from "../../i18n.ts";
import Button from "../Button/Button.vue";
import CodeBadge from "../CodeBadge/CodeBadge.vue";
import Icon from "../Icon/Icon.vue";
import IconButton from "../IconButton/IconButton.vue";
import { TONE_ICONS, TONE_WORDS } from "../InlineAlert/alert.ts";
import type { ToastEntry } from "./toasts.ts";

const props = defineProps<{ toast: ToastEntry; paused: boolean }>();
const emit = defineEmits<{ dismiss: [] }>();

let left = props.toast.duration;
let since = 0;
let timer: ReturnType<typeof setTimeout> | undefined;

function start(): void {
  if (timer !== undefined || !Number.isFinite(left)) return;
  since = performance.now();
  timer = setTimeout(() => emit("dismiss"), Math.max(left, 0));
}

function hold(): void {
  if (timer === undefined) return;
  clearTimeout(timer);
  timer = undefined;
  left -= performance.now() - since;
}

onMounted(() => {
  if (!props.paused) start();
});
watch(
  () => props.paused,
  (paused) => (paused ? hold() : start()),
);
onBeforeUnmount(hold);

function run(): void {
  props.toast.action?.run();
  emit("dismiss");
}
</script>

<template>
  <li class="k-toast" :data-tone="props.toast.tone" :data-toast-id="props.toast.id">
    <Icon :name="TONE_ICONS[props.toast.tone]" class="k-toast__icon" />
    <div class="k-toast__body">
      <p class="k-toast__title">
        <span class="k-visually-hidden">{{ t(TONE_WORDS[props.toast.tone]) }}{{ t("colon") }} </span
        >{{ props.toast.title }}
      </p>
      <p v-if="props.toast.description" class="k-toast__text">{{ props.toast.description }}</p>
      <p v-if="props.toast.code || props.toast.requestId" class="k-toast__codes">
        <CodeBadge v-if="props.toast.code" :code="props.toast.code" />
        <CodeBadge v-if="props.toast.requestId" :code="props.toast.requestId" label="Request-Id" />
      </p>
      <p v-if="props.toast.action" class="k-toast__action">
        <Button size="sm" @click="run">{{ props.toast.action.label }}</Button>
      </p>
    </div>
    <IconButton
      class="k-toast__close"
      icon="x"
      :label="t('toastDismiss')"
      size="sm"
      @click="emit('dismiss')"
    />
  </li>
</template>

<style>
@layer components {
  /* The tone, the icon, the body and the title: `InlineAlert/tone.css`. */
  .k-toast {
    padding-block: var(--k-space-2);
    padding-inline: var(--k-space-3) var(--k-space-1);
    box-shadow: var(--k-shadow-overlay);
    pointer-events: auto;
  }

  /* Motion (design « Mouvement »): up 8 px from its place, with a fade; out, a fade (held
   * by its region until the end). Under reduced motion, the fade alone. */
  .k-toast {
    transition-property: opacity, translate;
    transition-duration: max(var(--k-duration-medium), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-standard);
  }

  @starting-style {
    .k-toast {
      opacity: 0;
      translate: 0 var(--k-motion-shift-md);
    }
  }

  .k-toast[data-state="closed"] {
    pointer-events: none;
    animation-name: k-fade-out;
    animation-duration: max(var(--k-duration-short), var(--k-duration-fade));
    animation-timing-function: var(--k-ease-exit);
    animation-fill-mode: forwards;
  }

  .k-toast__text,
  .k-toast__codes,
  .k-toast__action {
    margin: 0;
  }

  .k-toast__codes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    min-inline-size: 0;
  }

  /* Centred on the first line of the title, as the icon of the tone. */
  .k-toast__close {
    flex: none;
    margin-block-start: calc(var(--k-space-0-5) * -1);
  }
}
</style>
