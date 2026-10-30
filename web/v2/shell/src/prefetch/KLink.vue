<script setup lang="ts">
// The link of the shell (design « Préchargement », REQ-021-22): the router's navigation, and
// on intent — hover held 60 ms, or focus — the reads its target declares (`meta.queries`)
// and the modules of the plugin it opens (`meta.modules`). Given to @kartenn/ui with
// `provideLink`, so that every link of the library prefetches. Classes and ARIA attributes
// fall through to the `<a>`.
import { prefetch } from "@kartenn/sdk";
import { onBeforeUnmount, onMounted, useTemplateRef } from "vue";
import { useLink, type RouteLocationRaw } from "vue-router";
import { preloadModulesOf, queriesOf } from "./intent.ts";

const props = defineProps<{ to: RouteLocationRaw; replace?: boolean }>();
const { route, href, isExactActive, navigate } = useLink(props);
const anchor = useTemplateRef<HTMLAnchorElement>("anchor");

let stop = () => {};
onMounted(() => {
  // Called once the intent is held: the plugin's modules go with the reads of the target.
  const onIntent = () => {
    preloadModulesOf(route.value);
    return queriesOf(route.value);
  };
  if (anchor.value) stop = prefetch.intent(anchor.value, onIntent);
});
onBeforeUnmount(() => stop());
</script>

<template>
  <a ref="anchor" :href="href" :aria-current="isExactActive ? 'page' : undefined" @click="navigate"
    ><slot
  /></a>
</template>
