<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t } = useI18n();
// The screens of each section come with the administration (spec 007, task 22).
const sections = ["roles", "bindings", "issuers", "serviceAccounts", "identities", "audit"];
</script>

<template>
  <h1>{{ t("admin.title") }}</h1>
  <ul>
    <li v-for="section in sections" :key="section">{{ t(`admin.${section}`) }}</li>
  </ul>
</template>
