<script setup lang="ts">
// Index of the catalogue: the sections and the components that declare their states.
import { catalogueNames } from "@kartenn/ui/catalogue";
import { RouterLink, useRoute } from "vue-router";

const route = useRoute();
</script>

<template>
  <section class="k-cat-section" aria-labelledby="ui-index-title">
    <h1 id="ui-index-title" class="k-cat-title">Catalogue du système de design</h1>
    <p class="k-cat-lead">
      Chaque composant de <code>@kartenn/ui</code> dans chacun de ses états ; mode, densité et
      langue se choisissent en tête de page et restent dans l'adresse.
    </p>
    <h2 class="k-cat-subtitle">Fondations</h2>
    <ul class="k-cat-list">
      <li>
        <RouterLink :to="{ path: '/_ui/jetons', query: route.query }">Jetons</RouterLink>
      </li>
      <li>
        <RouterLink :to="{ path: '/_ui/icones', query: route.query }">Icônes</RouterLink>
      </li>
    </ul>
    <h2 class="k-cat-subtitle">Composants</h2>
    <ul class="k-cat-list">
      <li v-for="name in catalogueNames" :key="name">
        <RouterLink :to="{ path: `/_ui/c/${name}`, query: route.query }">{{ name }}</RouterLink>
      </li>
    </ul>
  </section>
</template>
