<script setup lang="ts">
// The foot of a draft (REQ-021-6, 11, 37; design 021 « Cadre » : pied de l'onglet
// Brouillon; maquette E5, vue « soumission »): the result of the simulation (its badge then
// its sentence, which wraps, never cut), the motive in two fields (`CommitForm`, kept in the
// draft), and the split button: « Appliquer » (`?approval=self`) when the user holds
// `approval.self` on all it touches, « Soumettre en revue », « Enregistrer le brouillon »;
// an impossible one disabled with its reason or its permission. Ctrl+Entrée runs the
// chosen action. « Appliquer » prefills a generic summary. The dates of effect (`valid_from`,
// `valid_to`) stay behind a button until asked for or set — `valid_from` set from the date
// read under `valid_at` (REQ-021-5) —; the simulation and the submission read them, the
// sentence of the result says them. The submission keeps its idempotency key and its period
// until an answer: a retry after a lost answer replays them (REQ-006-15), the motive and the
// dates fixed meanwhile, with their reason, said by the refusal that fixes them; the answer
// come, the focus « Réessayer » held goes to the action beside it (2.4.3), or, the foot gone
// with its draft, to the panel. The dates are kept with the draft, from the date of its first
// addition under `valid_at`, back at the present too (`draftPeriods`). An answer is said
// by a toast (applied, in review with its reason), a refusal here. Under `as_of` nothing is
// written: the fields and the actions are refused with their reason.
import { drafts, type DraftSubmitOutcome } from "@kartenn/sdk";
import {
  Button,
  Cluster,
  CommitForm,
  DateTimeField,
  Icon,
  InlineAlert,
  SplitButton,
  Stack,
  StatusBadge,
  injectToasts,
} from "@kartenn/ui";
import { computed, nextTick, ref, shallowRef, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useChangesetWords } from "../changesets/words.ts";
import { keepMotive, typedMotive } from "./motive.ts";
import { useProblems } from "../problems/index.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import type { Refusal } from "../views/edit/inline.ts";
import { useSay } from "../views/edit/say.ts";
import { draftPeriods, submitDraft, useDraftQueries, type DraftAction } from "./state.ts";

const props = defineProps<{ id: string | null }>();
const { t } = useI18n();
const toasts = injectToasts();
const say = useSay();
const { lines } = useProblems();
const changesetWords = useChangesetWords();
const {
  draft,
  operations,
  writable,
  awaiting,
  period,
  simulated,
  simulation,
  simulating,
  errors,
  actions,
  preferred,
} = useDraftQueries(() => props.id);

// The dates of effect, kept with the draft (`draftPeriods`); fixed while a submission
// without an answer awaits its retry, which replays them as sent.
const periodWhy = computed(() =>
  !writable.value ? t("drafts.why.past") : awaiting.value ? t("drafts.period.frozen") : undefined,
);
const validFrom = computed<string | null>({
  get: () => period.value.valid_from ?? null,
  set: (value) => {
    if (props.id) draftPeriods.set(props.id, { ...period.value, valid_from: value });
  },
});
const validTo = computed<string | null>({
  get: () => period.value.valid_to ?? null,
  set: (value) => {
    if (props.id) draftPeriods.set(props.id, { ...period.value, valid_to: value });
  },
});
/** The fields show once asked for, or when a date is set. */
const periodAsked = ref(false);
watch(
  () => props.id,
  () => (periodAsked.value = false),
);
const periodShown = computed(
  () => periodAsked.value || validFrom.value !== null || validTo.value !== null,
);
const fromField = useTemplateRef<InstanceType<typeof DateTimeField>>("fromField");
const toField = useTemplateRef<InstanceType<typeof DateTimeField>>("toField");
async function askPeriod(): Promise<void> {
  periodAsked.value = true;
  await nextTick();
  fromField.value?.focus();
}
/** An end that does not follow the start, said on the end. */
const orderError = computed(() =>
  validFrom.value && validTo.value && Date.parse(validTo.value) <= Date.parse(validFrom.value)
    ? t("drafts.period.order")
    : undefined,
);
/** The entries complete and in order; otherwise the focus goes to the one at fault. */
function periodChecked(): boolean {
  const from = fromField.value?.check() ?? true;
  const to = toField.value?.check() ?? true;
  if (!from) fromField.value?.focus();
  else if (!to || orderError.value) toField.value?.focus();
  return from && to && !orderError.value;
}
/** What the result says of the period: at the present, from, until, or between. */
const effect = computed(() => {
  const from = changesetWords.when(validFrom.value);
  const to = changesetWords.when(validTo.value);
  if (from && to) return t("drafts.period.between", { from, to });
  if (from) return t("drafts.period.since", { from });
  if (to) return t("drafts.period.until", { to });
  return t("drafts.period.now");
});

// The motive, as the draft keeps it; read again when another draft, or another version
// of its reason, comes. Read only under `as_of`, and while a submission awaits its retry:
// the replay sends what was sent, a motive changed meanwhile would not go with it.
const motiveWhy = computed(() =>
  !writable.value ? t("drafts.why.past") : awaiting.value ? t("drafts.motiveFrozen") : undefined,
);
// The motive: one typed and not sent yet, kept outside the form (`motive.ts`: the panel's
// other tab, the page of the draft show it again), else the one kept with the draft; a
// reason read again never replaces a motive typed.
const summary = ref("");
const description = ref("");
watch(
  () => [props.id, draft.value?.reason.summary, draft.value?.reason.description] as const,
  ([id, s, d]) => {
    const own = id ? typedMotive(id) : undefined;
    summary.value = own?.summary ?? s ?? "";
    description.value = own?.description ?? d ?? "";
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
// A motive typed (other than the one kept with the draft, or the generic one prefilled):
// kept until it is sent or abandoned; the frame's panel asks before it drops it (Q-021-22).
watch([summary, description, () => draft.value?.reason] as const, ([s, d, kept]) => {
  if (!props.id) return;
  const typed = s.trim();
  const own =
    (typed !== (kept?.summary ?? "") && typed !== t("drafts.defaultSummary")) ||
    d.trim() !== (kept?.description ?? "");
  keepMotive(props.id, own ? { summary: s, description: d } : null);
});

const busy = ref(false);
const refusal = shallowRef<Refusal>();
watch(
  () => props.id,
  () => (refusal.value = undefined),
);

/** The refusal is the answer lost (no problem) of the submission that awaits its retry. */
const lostAnswer = computed(
  () =>
    awaiting.value !== undefined &&
    refusal.value?.kind === "failed" &&
    refusal.value.problem === null,
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
  if (action.id !== "save" && !periodChecked()) return;
  busy.value = true;
  refusal.value = undefined;
  try {
    if (!(await saveReason())) return;
    if (action.id === "save") {
      toasts?.show({ tone: "ok", title: t("drafts.saved", { title: draft.value?.title ?? "" }) });
      return;
    }
    settle(await submitDraft(id, action.id === "apply", draft.value?.title ?? "", period.value));
  } finally {
    busy.value = false;
  }
}

/** The submission that got no answer, if one (`awaiting`): « Réessayer » replays it as it was. */
async function retry(): Promise<void> {
  const replay = awaiting.value;
  if (!replay || busy.value || !writable.value) return;
  busy.value = true;
  refusal.value = undefined;
  try {
    settle(await submitDraft(replay.id, replay.approvalSelf, replay.title));
  } finally {
    busy.value = false;
  }
  // Answered, « Réessayer » is gone: the focus it held, lost with it, goes to the action
  // beside it while the foot stays (a refusal, the page); the foot gone with its draft, the
  // panel takes it (`DraftOperations`).
  if (awaiting.value) return;
  await nextTick();
  const active = document.activeElement;
  if (active && active !== document.body && active.isConnected) return;
  (actionsRow.value?.$el as HTMLElement | undefined)?.querySelector<HTMLElement>("button")?.focus();
}
const actionsRow = useTemplateRef<InstanceType<typeof Cluster>>("actionsRow");

const form = useTemplateRef<InstanceType<typeof CommitForm>>("form");
/** The main button: the summary checked by the form, then the chosen action. */
function onRun(id: string): void {
  chosen.value = id as DraftAction;
  if (chosen.value === "save" || summary.value.trim() !== "") void run();
  else form.value?.submit();
}

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
          t(
            "drafts.validText",
            { n: simulated.violations.length, effect },
            simulated.violations.length,
          )
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
      <!-- A lost answer fixes the motive and the dates: its refusal says so (4.1.3), the
           fields only the consequence. -->
      <RefusalAlert
        v-if="refusal"
        :refusal="refusal"
        v-bind="lostAnswer ? { note: t('drafts.frozen') } : {}"
      />
      <CommitForm
        ref="form"
        v-model:summary="summary"
        v-model:description="description"
        required
        collapsible
        :readonly="motiveWhy !== undefined"
        v-bind="motiveWhy ? { reason: motiveWhy } : {}"
        @submit="run"
      />
      <span v-if="!periodShown">
        <Button
          variant="ghost"
          size="sm"
          icon="calendar"
          v-bind="periodWhy ? { reason: periodWhy } : {}"
          @click="!periodWhy && askPeriod()"
          >{{ t("drafts.period.ask") }}</Button
        >
      </span>
      <Stack v-else gap="2">
        <DateTimeField
          ref="fromField"
          v-model="validFrom"
          :label="t('drafts.period.from')"
          :hint="periodWhy ?? t('drafts.period.fromHint')"
          :readonly="periodWhy !== undefined"
          size="sm"
        />
        <DateTimeField
          ref="toField"
          v-model="validTo"
          :label="t('drafts.period.to')"
          :hint="periodWhy ?? t('drafts.period.toHint')"
          :error="orderError"
          :readonly="periodWhy !== undefined"
          size="sm"
        />
      </Stack>
      <Cluster ref="actionsRow" justify="end" gap="2">
        <Button
          v-if="awaiting"
          variant="ghost"
          icon="refresh-cw"
          :loading="busy"
          v-bind="writable ? {} : { reason: t('drafts.why.past') }"
          @click="retry"
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
