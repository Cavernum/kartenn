<script setup lang="ts">
// The submissions that got no answer and whose draft has left the tab meanwhile (REQ-006-15):
// the server may have applied it and deleted the draft, its event come before « Réessayer ».
// Each says so, and « Réessayer » replays it under its key: the server answers what it did
// (applied, in review), said by a toast; a refusal, here. Under `as_of` nothing is written:
// « Réessayer » is refused with its reason, as in the foot (REQ-021-5).
import type { DraftSubmitOutcome } from "@kartenn/sdk";
import { Button, InlineAlert } from "@kartenn/ui";
import { computed, ref, shallowRef } from "vue";
import { useI18n } from "vue-i18n";
import { useTime, writesOf } from "../time/index.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import type { Refusal } from "../views/edit/inline.ts";
import { useSay } from "../views/edit/say.ts";
import { lostSubmissions, submitDraft, type LostSubmission } from "./state.ts";

const { t } = useI18n();
const say = useSay();
const time = useTime();
const writable = computed(() => writesOf(time.value).draft);
const lost = computed(() => lostSubmissions.all());
const busy = ref<string | null>(null);
const refusal = shallowRef<Refusal>();

async function retry(submission: LostSubmission): Promise<void> {
  if (busy.value || !writable.value) return;
  busy.value = submission.id;
  refusal.value = undefined;
  try {
    const outcome: DraftSubmitOutcome = await submitDraft(
      submission.id,
      submission.approvalSelf,
      submission.title,
    );
    if (outcome.kind === "applied") say.applied(submission.title, outcome.changeset);
    else if (outcome.kind === "proposed") {
      say.proposed(submission.title, outcome.changeset, outcome.reason);
    } else refusal.value = outcome;
  } finally {
    busy.value = null;
  }
}
</script>

<template>
  <RefusalAlert v-if="refusal" :refusal="refusal" :note="null" />
  <InlineAlert
    v-for="submission in lost"
    :key="submission.id"
    tone="warn"
    :title="t('drafts.lost', { title: submission.title })"
  >
    <p>{{ t("drafts.lostText") }}</p>
    <Button
      variant="ghost"
      size="sm"
      icon="refresh-cw"
      :loading="busy === submission.id"
      v-bind="writable ? {} : { reason: t('drafts.why.past') }"
      @click="retry(submission)"
      >{{ t("views.retry") }}</Button
    >
  </InlineAlert>
</template>
