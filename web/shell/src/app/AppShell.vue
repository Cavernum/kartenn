<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { visibleEntries } from "../navigation.ts";
import { useSession } from "../rights/index.ts";
import UserMenu from "./UserMenu.vue";

const { t } = useI18n();
const session = useSession();
const entries = computed(() => visibleEntries(session.value.rights));
</script>

<template>
  <a class="k-skip" href="#k-main">{{ t("app.skip") }}</a>
  <div class="k-layout">
    <header class="k-header">
      <RouterLink class="k-brand" to="/">{{ t("app.name") }}</RouterLink>
      <UserMenu />
    </header>
    <nav class="k-nav" :aria-label="t('app.navigation')">
      <ul>
        <li v-for="entry in entries" :key="entry.to">
          <RouterLink :to="entry.to">{{ t(entry.label) }}</RouterLink>
        </li>
      </ul>
    </nav>
    <main id="k-main" class="k-main" tabindex="-1">
      <RouterView />
    </main>
  </div>
</template>
