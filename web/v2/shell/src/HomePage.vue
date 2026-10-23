<script setup lang="ts">
// Placeholder of the new shell until its screens (spec 021, P2).
</script>

<template>
  <main class="k-home">
    <h1 class="k-home__title">kartenn</h1>
    <p>Nouveau shell en construction (spec 021).</p>
  </main>
</template>

<style>
@layer components {
  .k-home {
    padding: var(--k-space-8);
  }

  .k-home__title {
    font-size: var(--k-text-2xl);
    line-height: var(--k-text-2xl-line);
    font-weight: var(--k-weight-semibold);
  }
}
</style>
