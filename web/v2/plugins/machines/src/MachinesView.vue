<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The view `machines` under the space `hosts` (spec 021, design « Plugin Machines »; task
// 18cb): `/x/machines/machines`, the list, or `…/<id>`, the record of one machine. The shell
// creates the element again for each path of the view, and keeps it when only the query
// changes (the filters of the list): the page is chosen once, at its creation. Reached by a
// link of the plugin, the focus goes to the heading of the new page, the link clicked being
// gone with the former one (2.4.3).
import { onMounted, useTemplateRef } from "vue";
import { arrival, pageId } from "./dom.ts";
import MachineList from "./MachineList.vue";
import MachineRecord from "./MachineRecord.vue";

defineProps<{ object: unknown; row: unknown }>();

const id = pageId("machines");
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
  if (!arrival.byLink) return;
  arrival.byLink = false;
  root.value?.querySelector<HTMLElement>("h1")?.focus({ preventScroll: true });
});
</script>

<template>
  <!-- A click on a link inside bubbles here before the shell's host follows it. -->
  <!-- eslint-disable-next-line vuejs-accessibility/click-events-have-key-events, vuejs-accessibility/no-static-element-interactions -->
  <div ref="root" @click="onClick">
    <MachineRecord v-if="id" :id="id" />
    <MachineList v-else />
  </div>
</template>
