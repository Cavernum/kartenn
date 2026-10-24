<script setup lang="ts">
// Index of the catalogue: the sections, the components that declare their states and the
// views of the mock-ups.
import { catalogueNames } from "@kartenn/ui/catalogue";
import { RouterLink, useRoute } from "vue-router";
import { MOCKUPS } from "./ecrans/registry.ts";

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
    <h2 class="k-cat-subtitle">Maquettes</h2>
    <ul class="k-cat-list">
      <li>
        <RouterLink :to="{ path: '/_ui/revue', query: route.query }"
          >Revue des maquettes</RouterLink
        >
      </li>
      <template v-for="mockup in MOCKUPS" :key="mockup.id">
        <li v-for="view in mockup.views" :key="view.id">
          <RouterLink
            :to="{ path: `/_ui/ecrans/${mockup.id}`, query: { ...route.query, vue: view.id } }"
            >{{ mockup.id }} {{ mockup.title }} — {{ view.label }}</RouterLink
          >
        </li>
      </template>
    </ul>
  </section>
</template>
