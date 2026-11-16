<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// An object an element of the plugin cites (an owner, a site, a host; task 18cb), a link to
// its record if it has one. By its summary in the body (H23, task 11p10-d3): its `label`,
// nothing read; a summary without one is an object the caller may not read or one absent
// then, its identifier in fixed pitch, nothing supposed. By its identifier alone (the lists,
// until task 11p10-d4): its name once read through the shared cache (`Names` of the page),
// a cell skeleton while read, its identifier in fixed pitch if unreadable. None: said as an
// absent value (`NoValue`). In a record, an `AttributeList` says the absence: the record
// cites a name only when there is one.
import { NoValue, Skeleton } from "@kartenn/ui";
import { computed } from "vue";
import { hrefOf } from "./dom.ts";
import { useHosted } from "./host.ts";
import type { Name, Names, Summary } from "./reads.ts";

const props = defineProps<{
  /** Its summary in the body that cites it: named by its `label`, nothing read. */
  summary?: Summary | null | undefined;
  /** Without a summary, its name read by identifier through the shared cache. */
  names?: (() => Names) | undefined;
  /** Its type; unknown for an object the caller may not read (REQ-006-42): its id alone. */
  type?: Parameters<Names["of"]>[0] | undefined;
  id?: string | null | undefined;
  /** The page of its record, if it has one. */
  page?: string | undefined;
}>();

const { context } = useHosted();
const id = computed(() => (props.summary ? props.summary.id : props.id));
const name = computed<Name | null>(() => {
  if (props.summary) {
    const { label } = props.summary;
    return label ? { state: "read", name: label } : { state: "failed" };
  }
  return props.id && props.type && props.names ? props.names().of(props.type, props.id) : null;
});
const href = computed(() => {
  // The links carry the dates of the context (W9).
  void context.value.time;
  return props.page ? hrefOf(props.page) : undefined;
});
</script>

<template>
  <span v-if="!props.summary && props.id && !props.type" class="k-mono">{{ props.id }}</span>
  <NoValue v-else-if="!name" />
  <Skeleton v-else-if="name.state === 'reading'" variant="cell" />
  <template v-else-if="name.state === 'failed'">
    <a v-if="href" :href="href" class="k-mono">{{ id }}</a>
    <span v-else class="k-mono">{{ id }}</span>
  </template>
  <a v-else-if="href" :href="href">{{ name.name }}</a>
  <span v-else>{{ name.name }}</span>
</template>
