<script setup lang="ts">
// A component bench (spec 020, design « Budgets »; REQ-020-29): `/_ui/bancs/<Nom>`, the
// host `benches` of `@kartenn/ui/catalogue` declares for it, alone on the page, outside
// the frame of the catalogue, driven by the Playwright project `components-bench`
// (e2e/bench/). The switches of mode, density and language apply, as on every page.
import { benches } from "@kartenn/ui/catalogue";
import { watchEffect } from "vue";
import { useRoute } from "vue-router";
import { applySwitches, readSwitches } from "./switches.ts";

const props = defineProps<{ banc: string }>();
const route = useRoute();

watchEffect(() => applySwitches(document.documentElement, readSwitches(route.query)));
watchEffect(() => (document.title = `Banc ${props.banc}`));
</script>

<template>
  <main class="k-bench">
    <h1 class="k-bench__title">Banc {{ props.banc }}</h1>
    <component :is="benches[props.banc]" v-if="Object.hasOwn(benches, props.banc)" />
    <p v-else>Aucun banc pour « {{ props.banc }} ».</p>
  </main>
</template>

<style>
@layer components {
  .k-bench {
    display: grid;
    gap: var(--k-space-4);
    padding: var(--k-space-6);
  }
  .k-bench__title {
    margin: 0;
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
  }
}
</style>
