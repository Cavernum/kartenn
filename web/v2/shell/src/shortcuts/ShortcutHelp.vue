<script setup lang="ts">
// The help of the shortcuts (REQ-021-7; maquette E8): whether the single keys are on, and
// how to turn them off (WCAG 2.1.4); then the table of the registry, one source with the
// palette and the menus — the shell's shortcuts the user is shown, then those of the
// plugin's view — each with its keys (`ShortcutHint`: « g puis c »), what it does and
// where it acts.
// In the dialog `?` and on the page /help/shortcuts.
import { DataGrid, ShortcutHint, Stack, type GridColumn, type Shortcut } from "@kartenn/ui";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { ShellShortcut } from "./registry.ts";

const props = defineProps<{ shortcuts: readonly ShellShortcut[]; singleKeys: boolean }>();
const { t } = useI18n();

interface Row extends Record<string, unknown> {
  id: string;
  keys: Shortcut;
  action: string;
  scope: string;
}
const rows = computed<Row[]>(() =>
  props.shortcuts
    .filter((s) => s.shown?.() !== false)
    .map((s) => ({
      id: s.id,
      keys: s.keys,
      action: s.label(),
      scope: t(`shortcuts.where.${s.where}`),
    })),
);
const columns = computed<GridColumn[]>(() => [
  { id: "keys", label: t("shortcuts.keys"), width: "md" },
  { id: "action", label: t("shortcuts.action"), width: "fill", wrap: true },
  { id: "scope", label: t("shortcuts.scope"), width: "lg", wrap: true, priority: 2 },
]);
</script>

<template>
  <Stack gap="3">
    <!-- The state of the single keys first, in view before the table scrolls (2.1.4). No
         sheet of its own: shared by the frame and the catalogue (E8), it would be split into
         a chunk whose sheet the build links out of the order of the layers. -->
    <p>{{ props.singleKeys ? t("shortcuts.singleOn") : t("shortcuts.singleOff") }}</p>
    <DataGrid :label="t('shortcuts.title')" :columns="columns" :rows="rows" row-key="id">
      <template #cell="{ row, column, value }">
        <span v-if="column.id === 'keys'">
          <span class="k-visually-hidden">{{ t("shortcuts.keysSaid") }}</span>
          <ShortcutHint :keys="(row as Row).keys" />
        </span>
        <template v-else>{{ value }}</template>
      </template>
    </DataGrid>
  </Stack>
</template>
