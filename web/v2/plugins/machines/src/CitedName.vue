<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// An object an element of the plugin cites by its identifier (an owner, a site, a host;
// task 18cb): its name once read through the shared cache (`Names` of the page), a link to
// its record if it has one; while read, a cell skeleton; unreadable, its identifier in fixed
// pitch; none, a dash said « aucun ».
import { Skeleton } from "@kartenn/ui";
import { computed } from "vue";
import { hrefOf } from "./dom.ts";
import { useHosted, useTexts } from "./host.ts";
import type { Names } from "./reads.ts";

const props = defineProps<{
  names: () => Names;
  type: Parameters<Names["of"]>[0];
  id: string | null | undefined;
  /** The page of its record, if it has one. */
  page?: string | undefined;
}>();

const t = useTexts();
const { context } = useHosted();
const name = computed(() => (props.id ? props.names().of(props.type, props.id) : null));
const href = computed(() => {
  // The links carry the dates of the context (W9).
  void context.value.time;
  return props.page ? hrefOf(props.page) : undefined;
});
</script>

<template>
  <span v-if="!name"
    ><span aria-hidden="true">—</span><span class="k-visually-hidden">{{ t("none") }}</span></span
  >
  <Skeleton v-else-if="name.state === 'reading'" variant="cell" />
  <template v-else-if="name.state === 'failed'">
    <a v-if="href" :href="href" class="k-mono">{{ props.id }}</a>
    <span v-else class="k-mono">{{ props.id }}</span>
  </template>
  <a v-else-if="href" :href="href">{{ name.name }}</a>
  <span v-else>{{ name.name }}</span>
</template>
