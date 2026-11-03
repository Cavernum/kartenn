<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A value absent (task 020-30, 021/A27): never blank, a dash shown and words for the screen
// readers in its place (the dash alone would be read « tiret », or not at all). Said by
// `AttributeList` for a property without value, by `DataGrid` and `TreeGrid` for a cell
// whose value is `null`, and by a view in a cell it renders itself. `label`: the words
// read, « non renseigné » by default; a view whose cell says more than an absent value
// (« Aucun VLAN », « Sans objet ») gives its own.
import { t } from "../../i18n.ts";

const props = defineProps<{ label?: string }>();
</script>

<template>
  <span class="k-no-value"
    ><span aria-hidden="true">—</span
    ><span class="k-visually-hidden">{{ props.label ?? t("valueNone") }}</span></span
  >
</template>

<style>
@layer components {
  .k-no-value {
    color: var(--k-text-muted);
  }
}
</style>
