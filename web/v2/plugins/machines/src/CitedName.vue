<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// An object an element of the plugin cites (an owner, a site, a host, the machine of an
// interface; task 18cb), a link to its record if it has one. By its summary in the body
// (H23, tasks 11p10-d3, d4): its `label`, nothing read, nothing mounted to read it. A summary
// without `label` nor `href` is an object the caller may not read or one absent then
// (REQ-006-42): its identifier in fixed pitch, without a link (its record would not be read
// either), nothing supposed; « (illisible) » after it for the assistive technologies, which
// the fixed pitch does not reach (WCAG 1.3.1, as the cells of `DataGrid` say their word).
// None: said as an absent value (`NoValue`). In a record, an `AttributeList` says the
// absence: the record cites a name only when there is one.
import { NoValue } from "@kartenn/ui";
import { computed } from "vue";
import { hrefOf } from "./dom.ts";
import { useHosted, useTexts } from "./host.ts";
import { readable, type Summary } from "./reads.ts";

const props = defineProps<{
  /** Its summary in the body that cites it. */
  summary: Summary | null | undefined;
  /** The page of its record, if it has one; not linked when the summary was not read. */
  page?: string | undefined;
}>();

const t = useTexts();
const { context } = useHosted();
const read = computed(() => !!props.summary && readable(props.summary));
const href = computed(() => {
  // The links carry the dates of the context (W9).
  void context.value.time;
  return read.value && props.page ? hrefOf(props.page) : undefined;
});
</script>

<template>
  <NoValue v-if="!props.summary" />
  <span v-else-if="!read"
    ><span class="k-mono">{{ props.summary.id }}</span
    ><span class="k-visually-hidden"> ({{ t("cited.unreadable") }})</span></span
  >
  <a v-else-if="href" :href="href" :class="{ 'k-mono': !props.summary.label }">{{
    props.summary.label || props.summary.id
  }}</a>
  <span v-else :class="{ 'k-mono': !props.summary.label }">{{
    props.summary.label || props.summary.id
  }}</span>
</template>
