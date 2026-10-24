<script setup lang="ts">
// The generic page of a component (design « Catalogue »): every state its
// `<Nom>.catalogue.ts` declares, one cell each; `?etat=<name>` shows one state alone,
// for its capture.
import { loadCatalogue, type CatalogueEntry } from "@kartenn/ui/catalogue";
import { computed, ref, shallowRef, watch } from "vue";
import { RouterLink, useRoute } from "vue-router";

const props = defineProps<{ component: string }>();
const route = useRoute();
const entry = shallowRef<CatalogueEntry | null>(null);
const loaded = ref(false);

watch(
  () => props.component,
  async (name) => {
    loaded.value = false;
    entry.value = await loadCatalogue(name);
    loaded.value = true;
  },
  { immediate: true },
);

const alone = computed(() => typeof route.query.etat === "string");
const states = computed(() => {
  const only = route.query.etat;
  const all = entry.value?.states ?? [];
  return typeof only === "string" ? all.filter((state) => state.name === only) : all;
});
</script>

<template>
  <section class="k-cat-section" aria-labelledby="ui-component-title">
    <h1 id="ui-component-title" class="k-cat-title">{{ props.component }}</h1>
    <template v-if="entry">
      <p class="k-cat-lead">Motif : {{ entry.pattern }}</p>
      <div class="k-cat-states">
        <figure
          v-for="state in states"
          :key="state.name"
          class="k-cat-panel k-cat-state"
          :data-state-name="state.name"
          :data-interaction="state.interaction"
          :data-target="state.target"
          :data-wide="state.wide ? 'true' : undefined"
          :data-isolated="state.isolated ? 'true' : undefined"
        >
          <div class="k-cat-state__render">
            <RouterLink
              v-if="state.isolated && !alone"
              :to="{ query: { ...route.query, etat: state.name } }"
              >Voir l'état {{ state.name }} seul</RouterLink
            >
            <component :is="state.demo" v-else-if="state.demo" v-bind="state.props" />
            <component :is="entry.component" v-else v-bind="state.props">
              <template v-if="state.slot !== undefined" #default>{{ state.slot }}</template>
            </component>
          </div>
          <figcaption class="k-cat-note">
            <code>{{ state.name }}</code> — {{ state.description }}
          </figcaption>
        </figure>
      </div>
    </template>
    <p v-else-if="loaded" class="k-cat-lead">
      Aucune déclaration <code>{{ props.component }}.catalogue.ts</code>.
    </p>
  </section>
</template>

<style>
@layer components {
  .k-cat-states {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-4);
    align-items: start;
  }

  .k-cat-state {
    display: grid;
    flex: 0 1 auto;
    gap: var(--k-space-3);
    justify-items: start;
    max-inline-size: 100%;
    margin: 0;
  }

  /* An elastic track: a wide table scrolls in its place, never the page. */
  .k-cat-state[data-wide] {
    flex-basis: 100%;
    grid-template-columns: minmax(0, 1fr);
    justify-items: stretch;
  }

  .k-cat-state[data-wide] > .k-cat-state__render {
    display: block;
  }

  .k-cat-state__render {
    display: flex;
    align-items: center;
    min-block-size: var(--k-control-height-lg);
    color: var(--k-text);
  }
}
</style>
