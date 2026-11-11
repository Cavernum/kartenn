<script setup lang="ts">
// What the keys do in the cell of the focus of a grid edited in place, or why it has no
// editor (REQ-021-15, 37; design 021 « Édition en place »): Enter to the draft, Ctrl+Enter
// applied with a motive (or the permission it lacks), Escape, after why the cell is held in
// review when it is; else « Lecture seule » and the permission missing, copyable. At the
// foot of a list, of the tree, of the content.
import { CodeBadge, Kbd } from "@kartenn/ui";
import { useI18n } from "vue-i18n";
import type { NotEditable } from "./fields.ts";

const props = defineProps<{
  /** Why the cell has no editor, `null` when it has one. */
  why: NotEditable | null;
  /** The permission Ctrl+Enter lacks (`X.approval.self`), `null` when held. */
  applyLacks: string | null;
  /** Why the cell is held in review (the reason of its 202), seen as well as read. */
  held?: string | undefined;
}>();
const { t } = useI18n();
</script>

<template>
  <span v-if="props.why === null" class="k-keyboard-hint">
    <template v-if="props.held">{{ props.held }} · </template>
    <Kbd :keys="['Enter']" /> {{ t("edit.hintDraft") }} ·
    <template v-if="props.applyLacks === null">
      <Kbd :keys="['Control', 'Enter']" /> {{ t("edit.hintApply") }}
    </template>
    <template v-else>
      <Kbd :keys="['Control', 'Enter']" /> {{ t("edit.hintApplyForbidden") }}
      <CodeBadge :code="props.applyLacks" />
    </template>
    · <Kbd :keys="['Escape']" /> {{ t("edit.hintCancel") }}
  </span>
  <span v-else-if="props.why.kind !== 'none'" class="k-keyboard-hint">
    <template v-if="props.why.kind === 'forbidden'">
      {{ t("edit.why.forbidden") }} <CodeBadge :code="props.why.permission" />
    </template>
    <template v-else-if="props.why.kind === 'past'">{{ t("edit.why.past") }}</template>
    <template v-else-if="props.why.kind === 'dated'">{{ t("edit.why.inPlaceDated") }}</template>
    <template v-else>{{ t(`edit.why.${props.why.reason}`) }}</template>
  </span>
</template>
