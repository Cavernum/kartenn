<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The view `clusters` under the space `hosts` (spec 021, design « Plugin Machines »; task
// 18cd): `/x/machines/clusters`, the tree « Hébergement et clusters », or `…/<id>`, the
// record of one cluster. The shell creates the element again for each path of the view: the
// page is chosen once, at its creation. Reached by a link of the plugin (or Enter on a row of
// the tree), the focus goes to the heading of the new page (`PageRoot`).
import ClusterRecord from "./ClusterRecord.vue";
import { pageId } from "./dom.ts";
import HostingTree from "./HostingTree.vue";
import PageRoot from "./PageRoot.vue";

defineProps<{ object: unknown; row: unknown }>();

const id = pageId("clusters");
</script>

<template>
  <PageRoot>
    <ClusterRecord v-if="id" :id="id" />
    <HostingTree v-else />
  </PageRoot>
</template>
