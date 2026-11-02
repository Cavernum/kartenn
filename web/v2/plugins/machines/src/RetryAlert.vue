<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A read of the plugin that failed while what it read before stays shown (a next page, the
// hosts of a field; task 18cb): the alert with its codes, then « Réessayer », which reads it
// again — never asked again by itself.
import { Button, InlineAlert, Stack } from "@kartenn/ui";
import { useTexts } from "./host.ts";
import { linesOf } from "./problems.ts";

const props = defineProps<{ title: string; error: unknown }>();
const emit = defineEmits<{ retry: [] }>();
const t = useTexts();
</script>

<template>
  <Stack gap="2" align="start">
    <InlineAlert tone="ko" :title="props.title" :items="linesOf(props.error, t)" />
    <Button icon="refresh-cw" @click="emit('retry')">{{ t("retry") }}</Button>
  </Stack>
</template>
