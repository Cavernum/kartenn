<script setup lang="ts">
// A mock-up of the catalogue (design « Maquettes »): `/_ui/ecrans/<E1…E10>?vue=<id>`, with
// the switches of mode, density and language in the query like every page of the
// catalogue, but without its frame: the mock-up takes the whole page, as the shell will.
import { providePortalTarget } from "@kartenn/ui";
import { computed, defineAsyncComponent, watchEffect, type Component } from "vue";
import { useRoute } from "vue-router";
import { applySwitches, readSwitches } from "../switches.ts";
import { MOCKUPS, viewOf } from "./registry.ts";

const props = defineProps<{ ecran: string }>();
const route = useRoute();

watchEffect(() => applySwitches(document.documentElement, readSwitches(route.query)));
// Floating layers go to the end of the frame, after the work surface.
providePortalTarget("#k-ecran-layers");

const SCREENS: Record<string, Component> = {
  E1: defineAsyncComponent(() => import("./E1Referentiel.vue")),
  E2: defineAsyncComponent(() => import("./E2Fiche.vue")),
  E3: defineAsyncComponent(() => import("./E3Adresses.vue")),
  E4: defineAsyncComponent(() => import("./E4Changeset.vue")),
  E5: defineAsyncComponent(() => import("./E5Brouillon.vue")),
  E6: defineAsyncComponent(() => import("./E6Passe.vue")),
  E7: defineAsyncComponent(() => import("./E7Administration.vue")),
  E8: defineAsyncComponent(() => import("./E8Palette.vue")),
  E9: defineAsyncComponent(() => import("./E9Etats.vue")),
  E10: defineAsyncComponent(() => import("./E10Plugin.vue")),
};

const mockup = computed(() => MOCKUPS.find((m) => m.id === props.ecran));
const view = computed(() => (mockup.value ? viewOf(mockup.value, route.query.vue) : null));
watchEffect(() => {
  if (mockup.value && view.value)
    document.title = `${mockup.value.id} ${mockup.value.title} — ${view.value.label}`;
});
</script>

<template>
  <component :is="SCREENS[mockup.id]" v-if="mockup && view" :key="view.id" :vue="view.id" />
  <main v-else>
    <p>Maquette inconnue : {{ props.ecran }}.</p>
  </main>
</template>
