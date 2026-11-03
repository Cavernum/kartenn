<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A value absent in a cell of a grid (task 18g): a dash shown, « non renseigné » read, as an
// `AttributeList` says it in a record. The one place of the plugin that writes it: the cells
// of `DataGrid` say nothing of an absent value by themselves.
import { useTexts } from "./host.ts";

const t = useTexts();
</script>

<template>
  <span
    ><span aria-hidden="true">—</span><span class="k-visually-hidden">{{ t("notSet") }}</span></span
  >
</template>
