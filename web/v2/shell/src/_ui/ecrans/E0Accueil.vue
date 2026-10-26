<script setup lang="ts">
// E0 — Accueil (design « Maquettes », 024/M10; 021, « Écrans » : Accueil): the home page
// (`EcranAccueil`, shared with the background of E8) in the frame.
import Accueil from "./EcranAccueil.vue";
import Cadre from "./EcranCadre.vue";

defineProps<{ vue: string }>();
</script>

<template>
  <Cadre title="Accueil" active="home" space="referential" :crumbs="[{ label: 'Accueil' }]">
    <Accueil />
  </Cadre>
</template>
