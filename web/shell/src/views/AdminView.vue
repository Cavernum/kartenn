<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { SECTIONS } from "../admin/admin.ts";
import { useSession } from "../rights/index.ts";

// The administration (REQ-007-19): its sections, those the user's rights open, and the
// follow-up of the imports.

const { t } = useI18n();
const session = useSession();
const sections = computed(() => SECTIONS.filter((s) => session.value.rights.hasAny(s.anyOf)));
</script>

<template>
  <h1>{{ t("admin.title") }}</h1>
  <ul>
    <li v-for="section in sections" :key="section.path">
      <RouterLink :to="section.path">{{ t(section.label) }}</RouterLink>
    </li>
    <li>
      <RouterLink to="/imports">{{ t("nav.imports") }}</RouterLink>
    </li>
  </ul>
</template>
