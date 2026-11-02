<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The root of an element of the plugin that holds links to its pages (tasks 18cb, 18cc): a
// click on such a link says the next page was reached by a link, and a page (`heading`)
// reached so takes the focus on its heading at its mounting, the link clicked being gone
// with the former page (2.4.3).
import { onMounted, useTemplateRef } from "vue";
import { arrival } from "./dom.ts";

const props = withDefaults(defineProps<{ heading?: boolean }>(), { heading: true });
const root = useTemplateRef<HTMLElement>("root");

/** A click on a link to a page of the plugin: the next page takes the focus on its heading. */
function onClick(event: MouseEvent): void {
  // Only the clicks the shell follows in this tab (design 021 « Liens d'un plugin »).
  if (event.defaultPrevented || event.button !== 0) return;
  if (event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
  const anchor = event.target instanceof Element ? event.target.closest("a[href]") : null;
  if (anchor && !["", "_self"].includes(anchor.getAttribute("target") ?? "")) return;
  if (anchor)
    arrival.byLink = new URL((anchor as HTMLAnchorElement).href).pathname.startsWith(
      "/x/machines/",
    );
}

onMounted(() => {
  if (!props.heading || !arrival.byLink) return;
  arrival.byLink = false;
  root.value?.querySelector<HTMLElement>("h1")?.focus({ preventScroll: true });
});
</script>

<template>
  <!-- A click on a link inside bubbles here before the shell's host follows it. -->
  <!-- eslint-disable-next-line vuejs-accessibility/click-events-have-key-events, vuejs-accessibility/no-static-element-interactions -->
  <div ref="root" @click="onClick"><slot /></div>
</template>
