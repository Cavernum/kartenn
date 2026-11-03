<script setup lang="ts">
// The page /help/shortcuts (design 021 « Routes »; maquette E8): the help `?` in a page, for
// a link and for whom has turned the single keys off.
import { Heading } from "@kartenn/ui";
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
    <Heading :level="1">{{ t("shortcuts.title") }}</Heading>
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
}
</style>
