<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A host of fixed height for the catalogue, that gives its one child the rest of its height
// and never less than its content (`minmax(auto, 1fr)`), as the shell's surface gives the
// panel of a list screen whose grid is virtualised (`Panel` `list`, 021-22f-h): the child
// fills it, and the host scrolls only when that child is taller than it.
</script>

<template>
  <div class="k-demo-host"><slot /></div>
</template>

<style>
@layer components {
  .k-demo-host {
    display: grid;
    grid-template-rows: minmax(auto, 1fr);
    grid-template-columns: minmax(0, 1fr);
    /* Taller than a list's cap (`--k-size-list-max`): a grid that fills it is uncapped. */
    block-size: var(--k-size-dialog-lg);
    padding: var(--k-space-3);
    overflow: auto;
    isolation: isolate;
    background-color: var(--k-bg);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }
}
</style>
