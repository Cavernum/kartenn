<script setup lang="ts">
// The frame of the catalogue: its sections and the switches of mode, density and language,
// kept in the query (design « Catalogue »).
import { providePortalTarget } from "@kartenn/ui";
import { computed, watchEffect } from "vue";
import { RouterLink, RouterView, useRoute, useRouter } from "vue-router";
import { applySwitches, readSwitches, type Switches } from "./switches.ts";

const route = useRoute();
const router = useRouter();
const switches = computed(() => readSwitches(route.query));

watchEffect(() => applySwitches(document.documentElement, switches.value));
// Floating layers stay inside <main>, where the snapshots of the accessibility tree look.
providePortalTarget("#k-cat-layers");

function set<K extends keyof Switches>(key: K, event: Event): void {
  const value = (event.target as HTMLSelectElement).value;
  void router.replace({ query: { ...route.query, [key]: value } });
}

const sections = [
  { to: "/_ui", label: "Index" },
  { to: "/_ui/jetons", label: "Jetons" },
  { to: "/_ui/icones", label: "Icônes" },
];
</script>

<template>
  <div class="k-cat">
    <header class="k-cat__bar">
      <nav class="k-cat__nav" aria-label="Catalogue">
        <RouterLink
          v-for="section in sections"
          :key="section.to"
          :to="{ path: section.to, query: route.query }"
          class="k-cat__link"
          exact-active-class="k-cat__link--active"
          >{{ section.label }}</RouterLink
        >
      </nav>
      <div class="k-cat__switches">
        <label class="k-cat__switch">
          Mode
          <select :value="switches.mode" @change="set('mode', $event)">
            <option value="light">clair</option>
            <option value="dark">sombre</option>
          </select>
        </label>
        <label class="k-cat__switch">
          Densité
          <select :value="switches.densite" @change="set('densite', $event)">
            <option value="compacte">compacte</option>
            <option value="confortable">confortable</option>
          </select>
        </label>
        <label class="k-cat__switch">
          Langue
          <select :value="switches.langue" @change="set('langue', $event)">
            <option value="fr">français</option>
            <option value="en">anglais</option>
          </select>
        </label>
      </div>
    </header>
    <main class="k-cat__main">
      <div id="k-cat-layers" class="k-cat__layers"></div>
      <RouterView />
    </main>
  </div>
</template>

<style>
@layer components {
  .k-cat__bar {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-4);
    align-items: center;
    justify-content: space-between;
    padding: var(--k-space-2) var(--k-space-6);
    background-color: var(--k-surface);
    border-block-end: var(--k-border-width) solid var(--k-border-strong);
  }

  .k-cat__nav,
  .k-cat__switches {
    display: flex;
    gap: var(--k-space-4);
    align-items: center;
  }

  .k-cat__link {
    font-weight: var(--k-weight-medium);
    text-decoration: none;
  }

  .k-cat__link--active {
    text-decoration: underline;
  }

  .k-cat__switch {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    color: var(--k-text-muted);
    font-size: var(--k-text-sm);
  }

  .k-cat__switch select {
    min-block-size: var(--k-control-height-sm);
    padding-inline: var(--k-control-padding-inline-sm);
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-md);
  }

  /* Floating layers are fixed: their container takes no place in the grid. */
  .k-cat__layers {
    display: contents;
  }

  .k-cat__main {
    display: grid;
    gap: var(--k-space-8);
    padding: var(--k-space-6);
  }

  /* Shared by the pages of the catalogue. */
  .k-cat-section {
    display: grid;
    gap: var(--k-space-3);
    align-content: start;
  }

  .k-cat-title {
    font-size: var(--k-text-xl);
    line-height: var(--k-text-xl-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-cat-subtitle {
    margin-block-start: var(--k-space-4);
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-cat-lead,
  .k-cat-note {
    max-inline-size: var(--k-measure-prose);
    color: var(--k-text-muted);
  }

  .k-cat-list {
    display: grid;
    gap: var(--k-space-1);
  }

  .k-cat-value {
    color: var(--k-text-muted);
  }

  .k-cat-table {
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
  }

  .k-cat-table th,
  .k-cat-table td {
    block-size: var(--k-row-height);
    padding-inline: var(--k-cell-padding-inline);
    text-align: start;
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-cat-table thead th {
    block-size: var(--k-header-height);
    color: var(--k-text-muted);
    font-size: var(--k-text-sm);
    font-weight: var(--k-weight-medium);
  }

  .k-cat-table caption {
    padding-block: var(--k-space-2);
    text-align: start;
  }

  .k-cat-panel {
    padding: var(--k-space-4);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-lg);
  }
}
</style>
