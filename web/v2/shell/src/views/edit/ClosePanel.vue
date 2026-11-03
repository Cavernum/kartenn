<script setup lang="ts">
// « Clore… » of a record (021/A31, task 11g; REQ-004-25, 26; REQ-021-37): in the Détail tab
// of the right panel, like the edit panel, the closure of an object of the core — to the
// draft, or applied at once with its motive (`?approval=self`) — by the split button;
// Ctrl+Enter runs the chosen action. `expected` is the version served with the value read
// (one snapshot, task 11d), never one read apart. Its simulation alone on the referential
// says beforehand what it would do: the rules that refuse it, with their codes, and, the
// cascade asked, the dependents it closes too, counted by kind. A refusal of the server
// (422, 403, a version changed meanwhile) stays here with its code; « Appliquer » without
// `<sorte>.approval.self` stays visible, refused, and names the permission.
import { addToActiveDraft } from "@kartenn/sdk/host";
import { changesets, reads, type Entry, type Simulated, type WriteOutcome } from "@kartenn/sdk";
import {
  Checkbox,
  Cluster,
  Button,
  InlineAlert,
  Skeleton,
  SplitButton,
  Stack,
  TextField,
  injectToasts,
  type SplitAction,
} from "@kartenn/ui";
import { computed, nextTick, ref, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../../data/query.ts";
import ProblemState from "../../problems/ProblemState.vue";
import { useProblems } from "../../problems/index.ts";
import { useSession } from "../../rights/index.ts";
import { useTime } from "../../time/index.ts";
import { collectionById, nameOf, type Item } from "../collections.ts";
import { useKeyLabel } from "../data.ts";
import {
  CASCADING,
  closeOperation,
  closeVerdict,
  CONFLICT_RULE,
  DEPENDENTS_RULE,
  typeOfKind,
} from "./close.ts";
import { editRight, lackingRight } from "./fields.ts";
import type { Refusal } from "./inline.ts";
import { closeEdit, type CloseRequest } from "./panel.ts";
import RefusalAlert from "./RefusalAlert.vue";
import { useSay } from "./say.ts";

const props = defineProps<{ request: CloseRequest }>();
const { t } = useI18n();
const session = useSession();
const time = useTime();
const keyLabel = useKeyLabel();
const toasts = injectToasts();
const say = useSay((text) => toasts?.show({ tone: "ok", title: text }));
const { lines } = useProblems();

const collection = computed(() => collectionById(props.request.collection)!);
const kind = computed(() => collection.value.kind);
const kindWords = computed(() => keyLabel("kind", collection.value.type) ?? kind.value);

// The object closed, read at the present (the same entry as its record): its version, served
// with its value, is the `expected` of the closure.
const record = useQuery(() => reads.object<Item>(kind.value, props.request.id));
const item = computed(() => (record.data.value as Entry<Item> | undefined)?.data ?? null);
const name = computed(() => (item.value ? nameOf(item.value) : ""));
const version = ref<string | null>(null);
const filled = ref(false);
watch(
  item,
  (read) => {
    if (!read || filled.value) return;
    version.value = typeof read.version === "string" ? read.version : null;
    filled.value = true;
  },
  { immediate: true },
);

// The cascade (REQ-004-26): offered for a kind that holds others, or once the simulation says
// objects of a module still refer to it.
const cascade = ref(false);
const operation = computed(() =>
  closeOperation(kind.value, props.request.id, version.value, cascade.value),
);
const simulation = useQuery(() => ({
  ...changesets.simulate({ motive: "-", operations: [operation.value] }),
  enabled: filled.value && time.value.present,
  retry: false,
  // The cascade ticked: the verdict before stays until the new one, no blank in between.
  placeholderData: (previous: Simulated | undefined) => previous,
}));
const verdict = computed(() =>
  simulation.data.value ? closeVerdict(kind.value, simulation.data.value, cascade.value) : null,
);
const offersCascade = computed(
  () =>
    CASCADING.has(kind.value) ||
    cascade.value ||
    (verdict.value?.errors.some((v) => v.rule === DEPENDENTS_RULE) ?? false),
);
const items = (violations: readonly { detail: string; rule: string }[]) =>
  violations.map((v) => ({ message: v.detail, code: v.rule }));
/** The closures the cascade adds, in words: « Plage · 1, Adresse · 2 ». */
const withIt = computed(() =>
  (verdict.value?.cascaded ?? [])
    .map((c) => `${keyLabel("kind", typeOfKind(c.kind)) ?? c.kind} · ${c.count}`)
    .join(", "),
);
/** A simulation that failed: the problem in words, its code and `Request-Id` copyable. */
const unsimulated = computed(() => (simulation.error.value ? lines(simulation.error.value) : null));
/** What the status of the simulation says: running, or nothing (its verdict is an alert). */
const simulating = computed(() =>
  simulation.isFetching.value && (!verdict.value || simulation.isPlaceholderData.value)
    ? t("edit.close.simulating")
    : "",
);

// The action of the split button: to the draft by default; « Appliquer » with the right.
const action = ref("draft");
const lacking = (id: string) =>
  lackingRight(
    (p) => session.value.rights.has(p),
    [editRight(collection.value.type)],
    id === "apply",
  );
const actions = computed<SplitAction[]>(() =>
  [
    { id: "draft", label: t("edit.close.toDraft"), icon: "draft" as const },
    { id: "apply", label: t("edit.close.apply"), icon: "circle-check" as const },
  ].map((a) => {
    const permission = lacking(a.id);
    return permission ? { ...a, forbidden: permission } : a;
  }),
);
const applying = computed(() => action.value === "apply" && lacking("apply") === null);
// The generic motive, prefilled until the user writes one of their own.
const motive = ref("");
let motiveTyped = false;
watch(
  [name, kindWords],
  () => {
    if (!motiveTyped)
      motive.value = t("edit.close.motive", { kind: kindWords.value, name: name.value });
  },
  { immediate: true },
);
/** The key of a closure applied at once, kept while nothing changes (REQ-006-15). */
let idempotencyKey = crypto.randomUUID();
function setMotive(text: string): void {
  motive.value = text;
  motiveTyped = true;
  idempotencyKey = crypto.randomUUID();
}
watch(cascade, () => (idempotencyKey = crypto.randomUUID()));

const title = computed(() => t("edit.close.title", { kind: kindWords.value, name: name.value }));
const what = computed(() => t("edit.close.what", { kind: kindWords.value, name: name.value }));

const motiveError = ref<string | undefined>();
const refusal = ref<Refusal | null>(null);
const notice = ref<string | null>(null);
const busy = ref(false);

function settle(outcome: WriteOutcome<unknown> | { kind: "saved" }): void {
  busy.value = false;
  if (outcome.kind === "saved") {
    say.status(t("edit.drafted", { what: what.value }));
    closeEdit();
  } else if (outcome.kind === "applied") {
    say.applied(what.value, outcome.changeset);
    closeEdit();
  } else if (outcome.kind === "proposed") {
    say.proposed(what.value, outcome.changeset, outcome.reason);
    closeEdit();
  } else {
    refusal.value = outcome as Refusal;
    if (conflicted.value) void readAgain();
  }
}
/** The object changed since its read: the closure is written again on the version read again. */
const conflicted = computed(() => {
  const r = refusal.value;
  if (r?.kind === "conflict") return true;
  return r?.problem?.violations.some((v) => v.rule === CONFLICT_RULE) ?? false;
});
async function readAgain(): Promise<void> {
  await record.refetch();
  const read = item.value;
  if (read && typeof read.version === "string") version.value = read.version;
}

async function run(id: string = action.value): Promise<void> {
  if (busy.value) return;
  // A refused action, run at the keyboard: its permission said by the status, not silence.
  const missing = lacking(id);
  if (missing !== null) {
    notice.value = t("edit.close.forbidden", { permission: missing });
    return;
  }
  notice.value = null;
  refusal.value = null;
  motiveError.value = undefined;
  // Nothing is written off the present, the draft included.
  if (!time.value.present) {
    notice.value = t("edit.close.past");
    return;
  }
  if (id === "apply" && motive.value.trim() === "") {
    motiveError.value = t("edit.motiveMissing");
    await nextTick();
    root.value?.querySelector<HTMLElement>("[aria-invalid='true']")?.focus();
    return;
  }
  busy.value = true;
  if (id === "draft") {
    settle(
      (await addToActiveDraft([operation.value])) as WriteOutcome<unknown> | { kind: "saved" },
    );
    return;
  }
  const outcome = await changesets
    .submit({ motive: motive.value.trim(), operations: [operation.value] }, idempotencyKey, {
      approvalSelf: true,
    })
    .catch(() => ({ kind: "failed", input: {}, problem: null }) as Refusal);
  settle(outcome);
}

function onKeydown(event: KeyboardEvent): void {
  if (event.defaultPrevented) return;
  if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
    event.preventDefault();
    void run();
  }
}

// The focus, once the panel shows: the cascade if offered, else the action; again at the next
// frame if the side panel, opening, took it elsewhere meanwhile.
const root = useTemplateRef<HTMLFormElement>("root");
function focusStart(): void {
  if (!root.value || root.value.contains(document.activeElement)) return;
  const start =
    root.value.querySelector<HTMLElement>(".k-close-panel__cascade input") ??
    root.value.querySelector<HTMLElement>(".k-split-button__main");
  start?.focus();
}
watch(
  filled,
  async (ready) => {
    if (!ready) return;
    await nextTick();
    focusStart();
    requestAnimationFrame(focusStart);
  },
  { immediate: true },
);
</script>

<template>
  <ProblemState
    v-if="record.isError.value"
    :title="t('edit.close.readError', { kind: kindWords })"
    :error="record.error.value"
    :retry="record.refetch"
  />
  <Skeleton v-else-if="!filled" variant="rows" :rows="3" />
  <form
    v-else
    ref="root"
    class="k-close-panel"
    :aria-label="title"
    novalidate
    @submit.prevent="run()"
    @keydown="onKeydown"
  >
    <Stack gap="3">
      <h2 class="k-close-panel__title">{{ title }}</h2>
      <Checkbox
        v-if="offersCascade"
        v-model="cascade"
        class="k-close-panel__cascade"
        :label="t('edit.close.cascade')"
        :hint="t('edit.close.cascadeHint')"
      />
      <!-- What the closure would do, said before it is sent: its simulation alone. -->
      <!-- Present before its sentence, so that the sentence is said. -->
      <p class="k-close-panel__muted" role="status">{{ simulating }}</p>
      <InlineAlert
        v-if="unsimulated"
        tone="warn"
        :title="t('edit.close.unsimulated')"
        :items="unsimulated"
      />
      <template v-else-if="verdict">
        <InlineAlert
          v-if="!verdict.valid"
          tone="ko"
          :title="t('edit.close.refused')"
          :items="items(verdict.errors)"
        >
          {{ verdict.dependents ? t("edit.close.refusedDependents") : "" }}
        </InlineAlert>
        <InlineAlert
          v-else
          :tone="verdict.cascaded.length > 0 ? 'warn' : 'ok'"
          :title="t('edit.close.valid')"
        >
          {{
            verdict.cascaded.length > 0
              ? t("edit.close.withIt", { list: withIt })
              : t("edit.close.alone")
          }}
        </InlineAlert>
        <InlineAlert
          v-if="verdict.warnings.length > 0"
          tone="warn"
          :title="t('edit.close.warnings')"
          :items="items(verdict.warnings)"
        />
      </template>
      <TextField
        v-if="applying"
        :model-value="motive"
        :label="t('edit.motive')"
        :error="motiveError"
        required
        @update:model-value="setMotive"
      />
      <RefusalAlert
        v-if="refusal"
        :refusal="refusal"
        :title="t('edit.close.failed')"
        :note="conflicted ? t('edit.close.conflict') : null"
      />
      <!-- Present before its sentence, so that the sentence is said. -->
      <p class="k-close-panel__muted" role="status">{{ notice ?? "" }}</p>
      <Cluster justify="end" gap="2">
        <Button variant="ghost" @click="closeEdit()">{{ t("views.cancel") }}</Button>
        <SplitButton
          v-model:action="action"
          :actions="actions"
          :loading="busy"
          :menu-label="t('edit.chooseAction')"
          :shortcut="['Control', 'Enter']"
          @run="run"
        />
      </Cluster>
    </Stack>
  </form>
</template>

<style>
@layer components {
  .k-close-panel__title {
    font-size: var(--k-text-md);
    font-weight: var(--k-weight-semibold);
  }

  .k-close-panel__muted {
    color: var(--k-text-muted);
  }
}
</style>
