<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The view `interfaces` under the space `hosts` (spec 021, design « Plugin Machines »; task
// 18cc): `/x/machines/interfaces`, the search by MAC, address and VID, or `…/<id>`, the
// record of one interface. The shell creates the element again for each path of the view,
// and keeps it when only the query changes (the criteria of the search): the page is chosen
// once, at its creation. Reached by a link of the plugin, the focus goes to the heading of
// the new page (`PageRoot`).
import { pageId } from "./dom.ts";
import InterfaceRecord from "./InterfaceRecord.vue";
import InterfaceSearch from "./InterfaceSearch.vue";
import PageRoot from "./PageRoot.vue";

defineProps<{ object: unknown; row: unknown }>();

const id = pageId("interfaces");
</script>

<template>
  <PageRoot>
    <InterfaceRecord v-if="id" :id="id" />
    <InterfaceSearch v-else />
  </PageRoot>
</template>
