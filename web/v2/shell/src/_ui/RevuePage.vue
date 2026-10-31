<script setup lang="ts">
// The review of the mock-ups (spec 020, tasks 7, 13 to 15): every view of E1 to E10 in
// both modes and densities, the sheet of the business icons, the stand-ins of the
// components still to come and the points to decide.
import { RouterLink } from "vue-router";
import { MOCKUPS } from "./ecrans/registry.ts";

const COMBOS = [
  { mode: "light", densite: "compacte", label: "clair" },
  { mode: "dark", densite: "compacte", label: "sombre" },
  { mode: "light", densite: "confortable", label: "clair, confortable" },
  { mode: "dark", densite: "confortable", label: "sombre, confortable" },
] as const;

/** Components of the P4 replaced in the mock-ups by a marked stand-in. */
const LATER = [
  { component: "PeriodPicker", task: "20", where: "E6 (sélecteur de la pastille de date)" },
  { component: "Toast", task: "22", where: "E9 (accusé « proposé, en revue »)" },
];

/** Choices of the mock-ups to settle with the user (task 15). */
const OPEN = [
  "Rayon plein de la pastille de date (DatePill) : garder ou passer au rayon des contrôles.",
  "Barre latérale sombre en mode clair (direction « Ardoise ») : confirmer.",
  "Motif d'une entrée de menu désactivée sur deux lignes (E2, E4) : confirmer.",
  "Panneau de la surface de travail : composant Panel de @kartenn/ui ou pièce du cadre de la 021 (écrit ici pour les maquettes).",
  "Matrice de politique en DataGrid (une ligne par source) plutôt qu'un composant Matrix dédié (E10).",
  "Historique en DataGrid plutôt qu'une chronologie dessinée (E2, E6).",
];
</script>

<template>
  <section class="k-cat-section" aria-labelledby="ui-review-title">
    <h1 id="ui-review-title" class="k-cat-title">Revue des maquettes</h1>
    <p class="k-cat-lead">
      Les dix écrans de référence (spec 020, design « Maquettes ») construits avec les composants de
      <code>@kartenn/ui</code> sur des données fictives, à valider en clair et en sombre (tâche 15),
      avec la planche des icônes métier (tâche 7). Chaque vue est une adresse : mode et densité
      restent dans l'URL.
    </p>
    <ul class="k-cat-list">
      <li>
        <RouterLink :to="{ path: '/_ui/icones' }">Planche des icônes métier</RouterLink>
        (clair) —
        <RouterLink :to="{ path: '/_ui/icones', query: { mode: 'dark' } }">sombre</RouterLink>
      </li>
    </ul>

    <h2 class="k-cat-subtitle">Maquettes</h2>
    <table class="k-cat-table">
      <caption class="k-cat-note">
        Une ligne par vue ; les liens ouvrent la maquette seule, en pleine page.
      </caption>
      <thead>
        <tr>
          <th scope="col">Maquette</th>
          <th scope="col">Vue</th>
          <th scope="col">Ce qu'elle montre</th>
          <th scope="col">Ouvrir</th>
        </tr>
      </thead>
      <tbody>
        <template v-for="mockup in MOCKUPS" :key="mockup.id">
          <tr v-for="(view, v) in mockup.views" :key="view.id">
            <th v-if="v === 0" scope="rowgroup" :rowspan="mockup.views.length">
              {{ mockup.id }} — {{ mockup.title }}
            </th>
            <td>{{ view.label }}</td>
            <td class="k-cat-value">{{ view.description }}</td>
            <td>
              <span v-for="(combo, c) in COMBOS" :key="combo.label">
                <RouterLink
                  :to="{
                    path: `/_ui/ecrans/${mockup.id}`,
                    query: { vue: view.id, mode: combo.mode, densite: combo.densite },
                  }"
                  :aria-label="`${mockup.id} ${view.label}, ${combo.label}`"
                  >{{ combo.label }}</RouterLink
                ><template v-if="c < COMBOS.length - 1"> · </template>
              </span>
            </td>
          </tr>
        </template>
      </tbody>
    </table>

    <h2 class="k-cat-subtitle">Composants à venir, remplacés par un repère</h2>
    <p class="k-cat-lead">
      Encadrés en tirets et nommés dans la maquette (« À venir : … ») ; le plus proche composant
      existant tient leur place.
    </p>
    <table class="k-cat-table">
      <thead>
        <tr>
          <th scope="col">Composant</th>
          <th scope="col">Tâche (P4)</th>
          <th scope="col">Où</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="item in LATER" :key="item.component">
          <th scope="row">
            <code>{{ item.component }}</code>
          </th>
          <td class="k-num">{{ item.task }}</td>
          <td>{{ item.where }}</td>
        </tr>
      </tbody>
    </table>

    <h2 class="k-cat-subtitle">Points à trancher</h2>
    <ul class="k-cat-list k-cat-lead">
      <li v-for="point in OPEN" :key="point">{{ point }}</li>
    </ul>
  </section>
</template>
