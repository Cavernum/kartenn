<script setup lang="ts">
// The record of a changeset (REQ-021-10, 37; design 021 « Écrans » : Changesets; maquette
// E4): its state (`StatusBadge`), its review actions by the rights of the reader — each
// refused one visible, its permission or its reason in its tooltip, no box of the rights
// required —, the actions of the plugins on the type `changeset` in « … », its attributes,
// then its differential grouped by object (`DiffList`, read page by page), its operations,
// its violations with their codes and the sections of the modules. The author of a pending
// changeset reads why another must validate it (`changeset.approval.self`). Each decision
// goes under `If-Match` of the state read; a refusal is said above the record (`problems/`).
// Off the present, and under 1024 px wide (the gate of writes, REQ-021-29), nothing is
// decided: each decision refused with its reason, the record read; a dialog left open as
// the window narrows keeps what it holds, its decision refused.
import { queryClient, reads, type Operation } from "@kartenn/sdk";
import { review, type Decision, type DecisionOutcome } from "@kartenn/sdk/host";
import {
  AlertDialog,
  AttributeList,
  Button,
  CodeBadge,
  Dialog,
  DiffList,
  EmptyState,
  Grid,
  InlineAlert,
  KeyLabel,
  Skeleton,
  Stack,
  StatusBadge,
  Tabs,
  TextField,
  Toolbar,
  injectToasts,
  type MenuItem,
  type TabItem,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, nextTick, ref, shallowRef, watch, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { useInfiniteQuery, useQuery } from "../data/query.ts";
import OperationText from "../draft/OperationText.vue";
import PluginAction from "../plugins/PluginAction.vue";
import type { Placed } from "../plugins/registry.ts";
import { actionItems, objectRef, placed, useExtensions } from "../plugins/slots.ts";
import NextPageAlert from "../problems/NextPageAlert.vue";
import ProblemState from "../problems/ProblemState.vue";
import { useSession } from "../rights/index.ts";
import { useTime, useWrites } from "../time/index.ts";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import { useSay } from "../views/edit/say.ts";
import { rowsOf, useKeyLabel } from "../views/data.ts";
import { nameOf, type Item } from "../views/collections.ts";
import { keepRecordName } from "../views/names.ts";
import ViewPanel from "../views/parts/ViewPanel.vue";
import {
  countedGroups,
  diffGroups,
  held,
  reviewActions,
  stateBadge,
  type Changeset,
  type DiffEntry,
  type ReviewAction,
} from "./model.ts";
import { useCopyId } from "./copy.ts";
import { useChangesetWords } from "./words.ts";

const props = defineProps<{ id: string }>();
const { t, locale } = useI18n();
const route = useRoute();
const router = useRouter();
const session = useSession();
const time = useTime();
const writes = useWrites();
const words = useChangesetWords();
const keyLabel = useKeyLabel();
const toasts = injectToasts();
const say = useSay();

const record = useQuery(() => reads.object("changeset", props.id));
const changeset = computed(() => record.data.value?.data as Changeset | undefined);
watchEffect(() => {
  if (changeset.value) keepRecordName(props.id, changeset.value.motive);
});
const title = computed(() => changeset.value?.motive ?? props.id);

// The differential, page by page (`links.next`); its counts and sections from the first.
const diff = useInfiniteQuery(() => reads.subPages<DiffEntry>("changeset", props.id, "diff"));
const entries = computed(() => rowsOf(diff.data.value) as unknown as DiffEntry[]);
const first = computed(
  () =>
    diff.data.value?.pages[0]?.data as
      { sections?: { name: string; count: number }[]; counts?: unknown[] } | undefined,
);
const groups = computed(() =>
  diffGroups(entries.value, {
    property: (key) => keyLabel("property", key),
    masked: t("changesets.masked"),
    object: (id) => cachedName(id) ?? id,
  }),
);
const sections = computed(() => first.value?.sections ?? []);

/**
 * The name of an object of the differential when the cache holds it (its record or a row
 * read before): the differential serves its identifier only (contract 006).
 */
function cachedName(id: string): string | null {
  for (const query of queryClient
    .getQueryCache()
    .findAll({ predicate: (q) => q.queryKey[2] === id })) {
    const data = (query.state.data as { data?: unknown } | undefined)?.data;
    if (data && typeof data === "object" && (data as Item).id === id) {
      const name = nameOf(data as Item);
      if (name !== id) return name;
    }
  }
  return null;
}

// The review actions by the rights of the reader (REQ-021-10, 37), refused ones kept. A
// decision applies at once: off the present it is refused (`valid_at` alone included,
// which leaves only the draft to a write), else by the gate of writes (the width).
const context = computed(() => ({
  me: session.value.me?.identity ?? null,
  rights: session.value.rights,
  refused: time.value.present ? writes.value.refused : ("past" as const),
}));
const decisions = computed<ReviewAction[]>(() =>
  changeset.value ? reviewActions(changeset.value, context.value) : [],
);
/** Why nothing is decided now, if so: said by a dialog left open as the window narrowed. */
const gateWhy = computed(() =>
  context.value.refused ? t(`changesets.why.${context.value.refused}`) : undefined,
);
const ICONS = {
  validation: "check",
  rejection: "x",
  withdrawal: "undo-2",
  undo: "undo-2",
} as const;
const PRIORITY = { validation: 3, rejection: 2, withdrawal: 2, undo: 2 } as const;
const actions = computed<ToolbarAction[]>(() =>
  decisions.value.map((d) => ({
    id: d.id,
    label: t(`changesets.action.${d.id}`),
    icon: ICONS[d.id],
    priority: PRIORITY[d.id],
    ...(d.id === "validation" ? { variant: "primary" as const } : {}),
    ...(d.forbidden ? { forbidden: d.forbidden } : {}),
    ...(d.reason ? { reason: t(`changesets.why.${d.reason}`) } : {}),
  })),
);

// The actions of the plugins on the type `changeset` (REQ-021-13, task 16), in « … »
// with « Copier l'identifiant ».
const tr = (key: string) => t(key);
const pluginActions = useExtensions("action", "changeset");
const more = computed<MenuItem[]>(() => [
  { id: "copy", label: t("changesets.copyId"), icon: "copy" },
  ...actionItems(pluginActions.value, tr),
]);
const object = computed(() =>
  objectRef("changeset", props.id, `/v1/changesets/${encodeURIComponent(props.id)}`),
);
const acting = ref<Placed | null>(null);
const actionOpen = ref(false);

const busy = ref(false);
const refusal =
  shallowRef<Exclude<DecisionOutcome, { kind: "decided" | "applied" | "proposed" }>>();
watch(
  () => props.id,
  () => (refusal.value = undefined),
);

/** The outcome of a decision: a toast once through, a refusal said above the record. */
function settle(outcome: DecisionOutcome, action: Decision): boolean {
  if (outcome.kind === "decided") {
    toasts?.show({ tone: "ok", title: t(`changesets.done.${action}`, { motive: title.value }) });
    return true;
  }
  if (outcome.kind === "applied") {
    if (action === "undo")
      say.applied(t("changesets.undoOf", { motive: title.value }), outcome.changeset);
    else
      toasts?.show({ tone: "ok", title: t("changesets.done.validation", { motive: title.value }) });
    return true;
  }
  if (outcome.kind === "proposed") {
    say.proposed(
      t("changesets.undoOf", { motive: title.value }),
      outcome.changeset,
      outcome.reason,
    );
    return true;
  }
  refusal.value = outcome;
  return false;
}

/**
 * What a refused decision says: nothing typed is kept; a 412, that the changeset was
 * decided or changed meanwhile and read again — the bar offers what its state allows now.
 */
const refusalWords = computed(() =>
  refusal.value?.kind === "conflict"
    ? { title: t("changesets.changed"), note: t("changesets.changedText") }
    : { note: null },
);

/** The bar of the record: the state, then the review actions. */
const bar = ref<HTMLElement | null>(null);
/**
 * A decision changes the actions offered: the button that had the focus may be gone. The
 * focus goes to the first action of the bar that remains, else to « … » (2.4.3), once the
 * changeset is read again.
 */
async function focusBar(): Promise<void> {
  await record.refetch();
  await nextTick();
  const active = document.activeElement;
  if (active instanceof HTMLElement && active !== document.body && active.isConnected) return;
  bar.value?.querySelector<HTMLElement>(".k-toolbar button")?.focus();
}

async function decide(action: Decision, options: { motive?: string } = {}): Promise<boolean> {
  if (busy.value) return false;
  // Refused since its dialog opened (the window narrowed): said, nothing sent; the undo's
  // dialog is closed by its confirmation, a toast then says it (the rejection's, in itself).
  if (gateWhy.value) {
    toasts?.show({ tone: "warn", title: gateWhy.value });
    return false;
  }
  busy.value = true;
  refusal.value = undefined;
  try {
    const done = settle(
      await review.decide(props.id, action, { ...options, approvalSelf: action === "undo" }),
      action,
    );
    if (done) void focusBar();
    return done;
  } finally {
    busy.value = false;
  }
}

/** The identifier copied, said; refused by the browser, how to copy it by hand. */
const copyId = useCopyId();

// The rejection asks for its motive in a dialog (maquette E4, vue « rejet »); the undo is
// confirmed (it writes a new changeset).
const rejecting = ref(false);
const motive = ref("");
const motiveError = ref<string | undefined>();
const rejectForm = ref<HTMLElement | null>(null);
const undoing = ref(false);
/** The refusal of the gate said in the dialog of the rejection; gone once the gate opens. */
const rejectGate = ref<string | undefined>();
watch(gateWhy, (why) => {
  if (!why) rejectGate.value = undefined;
});

async function onAction(id: string): Promise<void> {
  const decision = decisions.value.find((d) => d.id === id);
  if (decision) {
    if (decision.forbidden || decision.reason) return;
    if (id === "rejection") {
      motive.value = "";
      motiveError.value = undefined;
      rejectGate.value = undefined;
      rejecting.value = true;
    } else if (id === "undo") undoing.value = true;
    else await decide(id as Decision);
    return;
  }
  if (id === "copy") {
    await copyId(props.id);
    return;
  }
  acting.value = placed(pluginActions.value, id) ?? null;
  actionOpen.value = acting.value !== null;
}

async function reject(): Promise<void> {
  // Refused by the gate (the window narrowed since the dialog opened): said in the dialog,
  // before the motive is checked (a toast lies under a modal); the motive kept.
  if (gateWhy.value) {
    rejectGate.value = gateWhy.value;
    return;
  }
  if (motive.value.trim() === "") {
    motiveError.value = t("changesets.motiveMissing");
    // The focus to the field in error (3.3.1).
    rejectForm.value?.querySelector<HTMLElement>("input")?.focus();
    return;
  }
  if (await decide("rejection", { motive: motive.value.trim() })) rejecting.value = false;
}
async function undo(): Promise<void> {
  undoing.value = false;
  await nextTick();
  await decide("undo");
}

// The tabs, the one shown in the URL (`?tab=operations`).
const operations = computed<readonly Operation[]>(() => changeset.value?.operations ?? []);
const errors = computed(() =>
  (changeset.value?.violations ?? []).filter((v) => v.severity === "error"),
);
const warnings = computed(() =>
  (changeset.value?.violations ?? []).filter((v) => v.severity !== "error"),
);
/** Every page of the differential is read: its count is the whole one. */
const complete = computed(() => diff.isSuccess.value && !diff.hasNextPage.value);
const tabs = computed<TabItem[]>(() => [
  {
    value: "diff",
    label: t("changesets.tab.diff"),
    ...(complete.value ? { count: groups.value.length } : {}),
  },
  { value: "operations", label: t("changesets.tab.operations"), count: operations.value.length },
  {
    value: "violations",
    label: t("changesets.tab.violations"),
    count: changeset.value?.violations.length ?? 0,
  },
  { value: "sections", label: t("changesets.tab.sections"), count: sections.value.length },
]);
const tab = computed({
  get: () => {
    const asked = route.query.tab;
    return typeof asked === "string" && tabs.value.some((tab) => tab.value === asked)
      ? asked
      : "diff";
  },
  set: (next: string) => {
    void router.replace({ query: { ...route.query, tab: next === "diff" ? undefined : next } });
  },
});

const author = computed(
  () => changeset.value !== undefined && changeset.value.author === context.value.me,
);
/** A person: « vous » in words, anyone else by an identifier in fixed pitch. */
const person = (key: string, label: string, identity: string) => ({
  key,
  label,
  value: words.person(identity),
  mono: identity !== context.value.me,
});
const attributes = computed(() => {
  const c = changeset.value;
  // Retroactive: its period starts before its submission (REQ-004, `retroactive` filter).
  const retroactive =
    c !== undefined && new Date(c.valid_from).getTime() < new Date(c.submitted_at).getTime();
  return {
    who: [
      { key: "kind", label: t("changesets.field.kind") },
      ...(c ? [person("author", t("views.field.author"), c.author)] : []),
      ...(c?.validator ? [person("validator", t("changesets.field.validator"), c.validator)] : []),
    ],
    when: [
      {
        key: "submitted",
        label: t("changesets.field.submitted"),
        value: words.when(c?.submitted_at),
        mono: true,
      },
      {
        key: "effect",
        label: t("changesets.field.effect"),
        value: words.when(c?.valid_from),
        mono: true,
      },
      ...(c?.applied_at
        ? [
            {
              key: "applied",
              label: t("changesets.field.applied"),
              value: words.when(c.applied_at),
              mono: true,
            },
          ]
        : []),
      // The end of the wait for a decision: of a pending changeset only.
      ...(c?.expires_at && c.state === "pending"
        ? [
            {
              key: "expires",
              label: t("changesets.field.expires"),
              value: words.when(c.expires_at),
              mono: true,
            },
          ]
        : []),
    ],
    what: [
      {
        key: "retroactive",
        label: t("views.field.retroactive"),
        value: t(`views.value.boolean.${retroactive}`),
      },
      ...(c?.rejection
        ? [{ key: "rejection", label: t("changesets.field.rejection"), value: c.rejection }]
        : []),
      ...(c?.policy
        ? [{ key: "policy", label: t("changesets.field.policy"), value: c.policy, mono: true }]
        : []),
      { key: "id", label: t("views.field.id") },
    ],
  };
});
const count = computed(() => new Intl.NumberFormat(locale.value));
const detail = computed(() =>
  changeset.value?.state === "applied" && complete.value
    ? t("changesets.objects", { n: count.value.format(groups.value.length) }, groups.value.length)
    : undefined,
);
/**
 * Before its application, the differential has no object yet (`/diff` reads what was
 * written): the counted one of the changeset stands for it, a group per property.
 */
const counted = computed(() =>
  countedGroups(changeset.value?.diff ?? [], {
    property: (key) => keyLabel("property", key),
    masked: t("changesets.masked"),
    objects: (n) => t("changesets.objects", { n: count.value.format(n) }, n),
  }),
);
</script>

<template>
  <h1 class="k-visually-hidden">{{ t("changesets.recordTitle", { motive: title }) }}</h1>
  <ProblemState
    v-if="record.isError.value"
    :title="t('changesets.recordError')"
    :error="record.error.value"
    :retry="record.refetch"
  >
    {{ t("views.errorText") }}
  </ProblemState>
  <ViewPanel v-else :title="title" :detail="detail" main>
    <!-- The state, then the review actions, in one bar (maquette E4). -->
    <template #toolbar>
      <Skeleton v-if="!changeset" variant="text" :rows="1" />
      <div v-else ref="bar" class="k-changeset__bar">
        <StatusBadge
          :state="stateBadge(changeset.state)"
          :label="words.state(changeset.state) ?? changeset.state"
        />
        <Toolbar
          :label="t('changesets.actions')"
          :actions="actions"
          :more="more"
          align="end"
          @action="onAction"
        />
      </div>
    </template>
    <Skeleton v-if="record.isPending.value" variant="rows" :rows="6" />
    <template v-else-if="changeset">
      <RefusalAlert v-if="refusal && !rejecting" :refusal="refusal" v-bind="refusalWords" />
      <!-- Held by a policy, its author does not apply it (REQ-024-12): said once, here. -->
      <InlineAlert
        v-if="author && changeset.state === 'pending' && held(changeset)"
        tone="info"
        :title="t('changesets.authorNotice')"
        :items="[
          { message: t('changesets.authorNoticeText'), code: 'changeset.approval.self' },
          ...(changeset.policy
            ? [{ message: t('changesets.policy'), code: changeset.policy }]
            : []),
        ]"
      />
      <Grid columns="auto" gap="3">
        <AttributeList tile :items="attributes.who">
          <template #kind>
            <KeyLabel :value="changeset.kind" :label="words.kind(changeset.kind)" />
          </template>
        </AttributeList>
        <AttributeList tile :items="attributes.when" />
        <AttributeList tile :items="attributes.what">
          <template #id><CodeBadge :code="props.id" /></template>
        </AttributeList>
      </Grid>
      <Tabs v-model="tab" :tabs="tabs" :label="t('changesets.sections')">
        <template #diff>
          <Stack gap="3">
            <!-- The anomalies the changeset leaves, before its differential (maquette E4). -->
            <InlineAlert
              v-if="warnings.length"
              tone="warn"
              :title="t('changesets.anomalies', warnings.length)"
              :items="warnings.map((v) => ({ message: v.detail, code: v.rule }))"
            />
            <NextPageAlert
              v-if="diff.isFetchNextPageError.value"
              :error="diff.error.value"
              :retry="diff.fetchNextPage"
            />
            <ProblemState
              v-if="diff.isError.value && !diff.isFetchNextPageError.value"
              :title="t('changesets.diffError')"
              :error="diff.error.value"
              :retry="diff.refetch"
            />
            <Skeleton v-else-if="diff.isPending.value" variant="rows" :rows="4" />
            <template v-else-if="groups.length === 0 && counted.length > 0">
              <p class="k-changeset__muted">{{ t("changesets.countedText") }}</p>
              <DiffList :groups="counted" />
            </template>
            <EmptyState
              v-else-if="groups.length === 0 && changeset.state !== 'applied'"
              :title="t('changesets.diffPending')"
            >
              {{ t("changesets.diffPendingText") }}
            </EmptyState>
            <EmptyState v-else-if="groups.length === 0" :title="t('changesets.diffNone')">
              {{ t("changesets.diffNoneText") }}
            </EmptyState>
            <DiffList v-else :groups="groups" />
            <span v-if="diff.hasNextPage.value">
              <Button
                variant="ghost"
                icon="chevron-down"
                :loading="diff.isFetchingNextPage.value"
                @click="diff.fetchNextPage()"
                >{{ t("changesets.diffMore") }}</Button
              >
            </span>
          </Stack>
        </template>
        <template #operations>
          <EmptyState v-if="operations.length === 0" :title="t('changesets.operationsNone')">
            {{ t("changesets.operationsNoneText") }}
          </EmptyState>
          <ol v-else class="k-changeset__operations" :aria-label="t('changesets.tab.operations')">
            <li v-for="(operation, index) in operations" :key="index">
              <OperationText :operation="operation" />
            </li>
          </ol>
        </template>
        <template #violations>
          <EmptyState
            v-if="changeset.violations.length === 0"
            :title="t('changesets.violationsNone')"
          >
            {{ t("changesets.violationsNoneText") }}
          </EmptyState>
          <Stack v-else gap="3">
            <InlineAlert
              v-if="errors.length"
              tone="ko"
              :title="t('changesets.errors', errors.length)"
              :items="errors.map((v) => ({ message: v.detail, code: v.rule }))"
            />
            <InlineAlert
              v-if="warnings.length"
              tone="warn"
              :title="t('changesets.warnings', warnings.length)"
              :items="warnings.map((v) => ({ message: v.detail, code: v.rule }))"
            />
          </Stack>
        </template>
        <template #sections>
          <EmptyState v-if="sections.length === 0" :title="t('changesets.sectionsNone')">
            {{ t("changesets.sectionsNoneText") }}
          </EmptyState>
          <AttributeList
            v-else
            :items="
              sections.map((s) => ({
                key: s.name,
                label: s.name,
                value: t('changesets.rows', { n: count.format(s.count) }, s.count),
              }))
            "
          />
        </template>
      </Tabs>
    </template>
  </ViewPanel>
  <Dialog
    v-model:open="rejecting"
    :title="t('changesets.rejectTitle', { motive: title })"
    :description="t('changesets.rejectText')"
    size="md"
  >
    <form ref="rejectForm" class="k-changeset__reject" novalidate @submit.prevent="reject">
      <TextField
        v-model="motive"
        :label="t('changesets.rejectMotive')"
        :hint="t('changesets.rejectHint')"
        :error="motiveError"
        required
      />
      <InlineAlert v-if="rejectGate" tone="warn" icon="lock" :title="rejectGate" />
      <RefusalAlert v-if="refusal && rejecting" :refusal="refusal" />
    </form>
    <template #actions>
      <Button variant="ghost" @click="rejecting = false">{{ t("views.cancel") }}</Button>
      <Button
        variant="danger"
        icon="x"
        :loading="busy"
        v-bind="gateWhy ? { reason: gateWhy } : {}"
        @click="reject"
        >{{ t("changesets.reject") }}</Button
      >
    </template>
  </Dialog>
  <AlertDialog
    v-model:open="undoing"
    :title="t('changesets.undoTitle', { motive: title })"
    :description="t('changesets.undoText')"
    :confirm="t('changesets.action.undo')"
    @confirm="undo"
  />
  <PluginAction v-model:open="actionOpen" :extension="acting" :object="object" />
</template>

<style>
@layer components {
  /* The state at the start, the actions taking the rest of the line, folded rather than
   * wrapped (024/M15), as the bar of E4. */
  .k-changeset__bar {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-3);
    align-items: center;
    min-inline-size: 0;
  }

  .k-changeset__bar > .k-toolbar {
    flex: 1 1 0;
    min-inline-size: var(--k-toolbar-floor);
    contain: inline-size;
  }

  .k-changeset__muted {
    color: var(--k-text-muted);
  }

  /* The operations as those of a draft: one per line, a hairline between them. */
  .k-changeset__operations {
    display: grid;
    min-inline-size: 0;
  }

  .k-changeset__operations > li {
    padding-block: var(--k-space-1);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  .k-changeset__reject {
    display: grid;
    gap: var(--k-space-3);
  }
}
</style>
