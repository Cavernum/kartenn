<script setup lang="ts">
// The foot of a draft (REQ-021-6, 11, 37; design 021 « Cadre » : pied de l'onglet
// Brouillon; maquette E5, vue « soumission »): the result of the simulation (its badge then
// its sentence, which wraps, never cut), the motive in two fields (`CommitForm`, kept in the
// draft), and the split button: « Appliquer » (`?approval=self`) when the user holds
// `approval.self` on all it touches, « Soumettre en revue », « Enregistrer le brouillon »;
// an impossible one disabled with its reason or its permission. Ctrl+Entrée runs the
// chosen action. « Appliquer » prefills a generic summary. The submission keeps its
// idempotency key until an answer: a retry after a lost answer replays it (REQ-006-15). An
// answer is said by a toast (applied, in review with its reason), a refusal here.
import { drafts, type DraftSubmitOutcome } from "@kartenn/sdk";
import {
  Button,
  Cluster,
  CommitForm,
  Icon,
  InlineAlert,
  SplitButton,
  StatusBadge,
  injectToasts,
} from "@kartenn/ui";
import { computed, ref, shallowRef, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useProblems } from "../problems/index.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import type { Refusal } from "../views/edit/inline.ts";
import { useSay } from "../views/edit/say.ts";
import { useDraftQueries, type DraftAction } from "./state.ts";

const props = defineProps<{ id: string | null }>();
const { t } = useI18n();
const toasts = injectToasts();
const say = useSay();
const { lines } = useProblems();
const { draft, operations, simulated, simulation, simulating, errors, actions, preferred } =
  useDraftQueries(() => props.id);

// The motive, as the draft keeps it; read again when another draft, or another version
// of its reason, comes.
const summary = ref("");
const description = ref("");
watch(
  () => [props.id, draft.value?.reason.summary, draft.value?.reason.description] as const,
  ([, s, d]) => {
    summary.value = s ?? "";
    description.value = d ?? "";
  },
  { immediate: true },
);

const words = (key: string, values?: Record<string, unknown>) => t(key, values ?? {});
const choices = computed(() => actions(words));
const chosen = ref<DraftAction>("apply");
/** The action follows the rights and the simulation until the user chooses one. */
const picked = ref(false);
watch(
  choices,
  (list) => {
    if (!picked.value) chosen.value = preferred(list);
  },
  { immediate: true },
);
watch(
  () => props.id,
  () => (picked.value = false),
);
function choose(id: string): void {
  chosen.value = id as DraftAction;
  picked.value = true;
}
// « Appliquer » prefills a generic summary, editable.
watch(
  chosen,
  (now) => {
    if (now === "apply" && summary.value.trim() === "") summary.value = t("drafts.defaultSummary");
  },
  { immediate: true },
);

const busy = ref(false);
const refusal = shallowRef<Refusal>();
/** The key of the submission in flight, kept until an answer (REQ-006-15). */
let key: string | null = null;
watch(
  () => props.id,
  () => {
    refusal.value = undefined;
    key = null;
  },
);

const reasonChanged = () =>
  (draft.value?.reason.summary ?? "") !== summary.value.trim() ||
  (draft.value?.reason.description ?? "") !== description.value.trim();

/** Keeps the motive in the draft; `false` when refused (said here). */
async function saveReason(): Promise<boolean> {
  if (!props.id || !reasonChanged()) return true;
  const outcome = await drafts.update(props.id, {
    reason: { summary: summary.value.trim(), description: description.value.trim() || null },
  });
  if (outcome.kind === "saved") return true;
  refusal.value = outcome;
  return false;
}

function settle(outcome: DraftSubmitOutcome): void {
  const what = draft.value?.title ?? "";
  if (outcome.kind === "applied") return say.applied(what, outcome.changeset);
  if (outcome.kind === "proposed") return say.proposed(what, outcome.changeset, outcome.reason);
  refusal.value = outcome;
}

async function run(): Promise<void> {
  const id = props.id;
  const action = choices.value.find((a) => a.id === chosen.value);
  if (!id || busy.value || !action || action.disabled) return;
  busy.value = true;
  refusal.value = undefined;
  try {
    if (!(await saveReason())) return;
    if (action.id === "save") {
      toasts?.show({ tone: "ok", title: t("drafts.saved", { title: draft.value?.title ?? "" }) });
      return;
    }
    key ??= crypto.randomUUID();
    const outcome = await drafts.submit(id, {
      approvalSelf: action.id === "apply",
      idempotencyKey: key,
    });
    // Any answer ends the key; a request that got none keeps it for a retry.
    if (!(outcome.kind === "failed" && outcome.problem === null)) key = null;
    settle(outcome);
  } finally {
    busy.value = false;
  }
}

const form = useTemplateRef<InstanceType<typeof CommitForm>>("form");
/** The main button: the summary checked by the form, then the chosen action. */
function onRun(id: string): void {
  chosen.value = id as DraftAction;
  if (chosen.value === "save" || summary.value.trim() !== "") void run();
  else form.value?.submit();
}

/** Why the chosen action cannot run now, for « Réessayer » which runs it again. */
const blockedNow = computed(() => {
  const action = choices.value.find((a) => a.id === chosen.value);
  if (!action?.disabled) return {};
  return action.forbidden ? { forbidden: action.forbidden } : { reason: action.reason ?? "" };
});

const failedSimulation = computed(() =>
  simulation.isError.value ? lines(simulation.error.value) : [],
);
</script>

<template>
  <div v-if="props.id" class="k-draft-submit">
    <!-- Present before the draft is read: its first result is said (4.1.3). -->
    <p class="k-draft-submit__result" role="status">
      <template v-if="!draft"></template>
      <template v-else-if="operations.length === 0">
        <span>{{ t("drafts.empty") }}</span>
      </template>
      <template v-else-if="simulating">
        <Icon name="loader-circle" class="k-spinner" />
        <span>{{ t("drafts.simulating") }}</span>
      </template>
      <template v-else-if="simulation.isError.value">
        <StatusBadge state="ko" :label="t('drafts.simulationFailed')" />
      </template>
      <template v-else-if="errors > 0">
        <StatusBadge state="ko" :label="t('drafts.violations', { n: errors }, errors)" />
        <span>{{ t("drafts.violationsText") }}</span>
      </template>
      <template v-else-if="simulated">
        <StatusBadge state="ok" :label="t('drafts.valid')" />
        <span>{{
          t("drafts.validText", { n: simulated.violations.length }, simulated.violations.length)
        }}</span>
      </template>
    </p>
    <template v-if="draft">
      <InlineAlert
        v-if="failedSimulation.length"
        tone="ko"
        :title="t('drafts.simulationFailed')"
        :items="failedSimulation"
      />
      <RefusalAlert v-if="refusal" :refusal="refusal" />
      <CommitForm
        ref="form"
        v-model:summary="summary"
        v-model:description="description"
        required
        collapsible
        @submit="run"
      />
      <Cluster justify="end" gap="2">
        <Button
          v-if="refusal?.kind === 'failed' && refusal.problem === null"
          variant="ghost"
          icon="refresh-cw"
          :loading="busy"
          v-bind="blockedNow"
          @click="run"
          >{{ t("views.retry") }}</Button
        >
        <SplitButton
          :action="chosen"
          :actions="choices"
          :loading="busy"
          :shortcut="['Control', 'Enter']"
          @update:action="choose"
          @run="onRun"
        />
      </Cluster>
    </template>
  </div>
</template>

<style>
@layer components {
  .k-draft-submit {
    display: grid;
    gap: var(--k-space-2);
    min-inline-size: 0;
  }

  /* The result of the simulation: the badge, then the sentence, which wraps under itself
   * when the panel is narrow, never cut (revue du 2026-10-25). */
  .k-draft-submit__result {
    display: flex;
    gap: var(--k-space-2);
    align-items: baseline;
    min-inline-size: 0;
  }

  .k-draft-submit__result > .k-status-badge,
  .k-draft-submit__result > .k-icon {
    flex: none;
    align-self: center;
  }

  .k-draft-submit__result > span:last-child {
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }
}
</style>
