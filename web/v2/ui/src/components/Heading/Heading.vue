<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A heading (spec 021/A27): its level for the outline (`h1` … `h6`), its size apart — that
// of a page (`page`, the size the shell's page titles take) or of a section (`section`),
// by default `page` at level 1 and `section` below. For the screens and the plugins
// written in Vue without a sheet of their own (A23): neither a class nor a value to copy.
// The attributes go to the heading itself (`id` for `aria-labelledby`, `tabindex="-1"` for
// the focus a view places on its title).
const props = defineProps<{ level: 1 | 2 | 3 | 4 | 5 | 6; size?: "page" | "section" }>();
</script>

<template>
  <component
    :is="`h${props.level}`"
    class="k-heading"
    :data-size="props.size ?? (props.level === 1 ? 'page' : 'section')"
  >
    <slot />
  </component>
</template>

<style>
@layer components {
  /* A long name without spaces (an address, an identifier) wraps, never overflows. */
  .k-heading {
    min-inline-size: 0;
    overflow-wrap: anywhere;
    font-weight: var(--k-weight-semibold);
  }

  .k-heading[data-size="page"] {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
  }

  .k-heading[data-size="section"] {
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
  }
}
</style>
