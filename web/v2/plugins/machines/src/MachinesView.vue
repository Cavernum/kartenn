<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The view `machines` under the space `hosts` (spec 021, design « Plugin Machines »; task
// 18cb): `/x/machines/machines`, the list, or `…/<id>`, the record of one machine. The shell
// creates the element again for each path of the view, and keeps it when only the query
// changes (the filters of the list): the page is chosen once, at its creation. Reached by a
// link of the plugin, the focus goes to the heading of the new page (`PageRoot`).
import { pageId } from "./dom.ts";
import MachineList from "./MachineList.vue";
import MachineRecord from "./MachineRecord.vue";
import PageRoot from "./PageRoot.vue";

defineProps<{ object: unknown; row: unknown }>();

const id = pageId("machines");
</script>

<template>
  <PageRoot>
    <MachineRecord v-if="id" :id="id" />
    <MachineList v-else />
  </PageRoot>
</template>
