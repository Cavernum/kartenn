<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A link through the component the host provides (`provideLink`, the router's), else a
// plain `<a href>`. Classes and ARIA attributes fall through.
import { injectLink } from "../../context.ts";

const props = defineProps<{ to: string }>();
const link = injectLink();
</script>

<template>
  <component :is="link" v-if="link" :to="props.to"><slot /></component>
  <a v-else :href="props.to"><slot /></a>
</template>
