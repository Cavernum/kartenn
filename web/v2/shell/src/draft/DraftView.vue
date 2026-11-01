<script setup lang="ts">
// A draft in its page (`/drafts/:id`; REQ-021-11; design 021 « Écrans » : Brouillons): the
// same operations, simulation and foot as the draft tab of the right panel, for any draft
// of the user; « Reprendre » makes it the active draft of the tab.
import { drafts } from "@kartenn/sdk";
import { Button, injectToasts } from "@kartenn/ui";
import { computed, onScopeDispose, ref, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import ProblemState from "../problems/ProblemState.vue";
import { keepRecordName } from "../views/names.ts";
import ViewPanel from "../views/parts/ViewPanel.vue";
import DraftOperations from "./DraftOperations.vue";
import DraftSubmit from "./DraftSubmit.vue";
import { useDraftQueries } from "./state.ts";

const props = defineProps<{ id: string }>();
const { t } = useI18n();
const toasts = injectToasts();
const { query, draft } = useDraftQueries(() => props.id);
watchEffect(() => {
  if (draft.value) keepRecordName(props.id, draft.value.title);
});

const active = ref<string | null>(drafts.active());
onScopeDispose(drafts.onActive((id) => (active.value = id)));
const isActive = computed(() => active.value === props.id);
function resume(): void {
  drafts.setActive(props.id);
  toasts?.show({ tone: "ok", title: t("drafts.resumed", { title: draft.value?.title ?? "" }) });
}
</script>

<template>
  <h1 class="k-visually-hidden">{{ draft?.title ?? t("app.draft") }}</h1>
  <ProblemState
    v-if="query.isError.value"
    :title="t('drafts.readError')"
    :error="query.error.value"
    :retry="query.refetch"
  >
    {{ t("views.errorText") }}
  </ProblemState>
  <ViewPanel
    v-else
    :title="draft?.title ?? t('app.draft')"
    :detail="isActive ? t('drafts.activeMark') : undefined"
    main
  >
    <template #actions>
      <Button
        icon="undo-2"
        v-bind="isActive ? { reason: t('drafts.why.active') } : {}"
        @click="isActive || resume()"
        >{{ t("drafts.resume") }}</Button
      >
    </template>
    <DraftOperations :id="props.id" page />
    <template #footer>
      <DraftSubmit :id="props.id" />
    </template>
  </ViewPanel>
</template>
