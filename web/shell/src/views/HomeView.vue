<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <h1>{{ t("home.title") }}</h1>
  <p class="k-muted">{{ t("home.intro") }}</p>
</template>
