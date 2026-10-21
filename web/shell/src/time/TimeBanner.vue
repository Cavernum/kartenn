<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { toPresent, useTime } from "./index.ts";

// Off the present, a permanent banner says so and brings back to the present in one
// click (REQ-007-22, W9): the confusion of past and present is the main risk of a
// bitemporal interface.

const { t, locale } = useI18n();
const router = useRouter();
const time = useTime();
const timeZone = Intl.DateTimeFormat().resolvedOptions().timeZone;

function shown(iso: string): string {
  return new Intl.DateTimeFormat(locale.value, { dateStyle: "long", timeStyle: "medium" }).format(
    new Date(iso),
  );
}
</script>

<template>
  <div v-if="!time.present" class="k-time-banner" role="status">
    <p>
      <template v-if="time.validAt">
        {{ t("time.validAt") }}
        <time :datetime="time.validAt">{{ shown(time.validAt) }}</time>
      </template>
      <template v-if="time.asOf">
        {{ t("time.asOf") }}
        <time :datetime="time.asOf">{{ shown(time.asOf) }}</time>
      </template>
      <span class="k-muted">({{ timeZone }})</span>
    </p>
    <button type="button" @click="toPresent(router)">{{ t("time.present") }}</button>
  </div>
</template>
