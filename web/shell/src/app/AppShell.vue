<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { visibleEntries } from "../navigation.ts";
import { extensionsOf, pluginKey, usePlugins } from "../plugins/registry.ts";
import { useSession } from "../rights/index.ts";
import SessionDialog from "../session/SessionDialog.vue";
import SearchBox from "./SearchBox.vue";
import TimeBanner from "../time/TimeBanner.vue";
import UserMenu from "./UserMenu.vue";

const { t } = useI18n();
const session = useSession();
const plugins = usePlugins();
const entries = computed(() => visibleEntries(session.value.rights));
// The views of plugins, under « Extensions » (design « Chargement des plugins »).
const views = computed(() =>
  extensionsOf(plugins.value, "view", session.value.rights).filter((v) => v.nav),
);
</script>

<template>
  <a class="k-skip" href="#k-main">{{ t("app.skip") }}</a>
  <div class="k-layout">
    <header class="k-header">
      <RouterLink class="k-brand" to="/">{{ t("app.name") }}</RouterLink>
      <SearchBox />
      <UserMenu />
    </header>
    <nav class="k-nav" :aria-label="t('app.navigation')">
      <ul>
        <li v-for="entry in entries" :key="entry.to">
          <RouterLink :to="entry.to">{{ t(entry.label) }}</RouterLink>
        </li>
      </ul>
      <template v-if="views.length > 0">
        <h2 id="k-nav-extensions" class="k-nav-heading">{{ t("nav.extensions") }}</h2>
        <ul aria-labelledby="k-nav-extensions">
          <li v-for="view in views" :key="`${view.plugin}.${view.id}`">
            <RouterLink :to="`/x/${view.plugin}/${view.id}`">
              {{ t(pluginKey(view, view.nav)) }}
            </RouterLink>
          </li>
        </ul>
      </template>
    </nav>
    <main id="k-main" class="k-main" tabindex="-1">
      <TimeBanner />
      <RouterView />
    </main>
  </div>
  <SessionDialog />
</template>
