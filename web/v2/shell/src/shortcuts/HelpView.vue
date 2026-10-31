<script setup lang="ts">
// The page /help/shortcuts (design 021 « Routes »; maquette E8): the help `?` in a page, for
// a link and for whom has turned the single keys off.
import { useI18n } from "vue-i18n";
import { useSettings } from "../frame/settings.ts";
import { useShortcuts } from "./context.ts";
import ShortcutHelp from "./ShortcutHelp.vue";

const { t } = useI18n();
const shortcuts = useShortcuts();
const settings = useSettings();
</script>

<template>
  <div class="k-help">
    <h1 class="k-help__title">{{ t("shortcuts.title") }}</h1>
    <ShortcutHelp :shortcuts="shortcuts" :single-keys="settings.singleKeys.value" />
  </div>
</template>

<style>
@layer components {
  .k-help {
    display: grid;
    gap: var(--k-space-3);
    min-inline-size: 0;
  }

  .k-help__title {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
    font-weight: var(--k-weight-semibold);
  }
}
</style>
