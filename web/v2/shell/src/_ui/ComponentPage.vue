<script setup lang="ts">
// The generic page of a component (design « Catalogue »): every state its
// `<Nom>.catalogue.ts` declares, one cell each; `?etat=<name>` shows one state alone,
// for its capture.
import { loadCatalogue, type CatalogueEntry } from "@kartenn/ui/catalogue";
import { computed, ref, shallowRef, watch } from "vue";
import { useRoute } from "vue-router";

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
        >
          <div class="k-cat-state__render">
            <component :is="entry.component" v-bind="state.props" />
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
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(var(--k-space-12), max-content));
    gap: var(--k-space-4);
  }

  .k-cat-state {
    display: grid;
    gap: var(--k-space-3);
    justify-items: start;
    margin: 0;
  }

  .k-cat-state__render {
    display: flex;
    align-items: center;
    min-block-size: var(--k-control-height-lg);
    color: var(--k-text);
  }
}
</style>
