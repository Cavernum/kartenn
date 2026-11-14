<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A host of `DataGrid` whose slot `cell` is compiled, as a screen's (`ListView`): a slot
// that reads only its own props, which the grid's rows render without the grid (task
// 021-22o-b, `data-grid-rows-mounted.test.ts`). `seen` tells the test each row rendered.
import DataGrid from "../../src/components/DataGrid/DataGrid.vue";

defineProps<{ grid: Record<string, unknown>; seen: (row: string) => string }>();
</script>

<template>
  <DataGrid v-bind="grid as any"
    ><template #cell="{ row, value }"
      ><a :href="`#${seen(String(row.id))}`">{{ value }}</a></template
    ></DataGrid
  >
</template>
