<script setup lang="ts">
// The operations of a draft (REQ-021-6, 11; design 021 « Cadre » : onglet Brouillon,
// « Écrans » : Brouillons; maquette E5): in the right panel for the active draft of the tab,
// on the page `/drafts/:id` for any of the user's. Each operation with its button to remove
// it and, after the simulation, its violations in plain words with their codes; the
// violations of the result, then what the simulation counts (differential, cascades). The
// deletion of an inactive draft is announced (`expires_at`). A removal refused is said here
// (`problems/`), the focus kept in the list. Under `as_of` the draft is read only, said at
// its head (design 021 « Écrans » : lecture seule annoncée en tête), each removal refused.
// « Modifier » of a line (task 14l, Q-021-17) opens the edit panel of its kind prefilled
// from the operation, which replaces it at its place; refused, with its reason, for an
// operation no panel writes (a closure, a move…) and under `as_of`.
import { drafts, type Draft, type Entry } from "@kartenn/sdk";
import { Cluster, DiffValue, Icon, IconButton, InlineAlert, Skeleton, Stack } from "@kartenn/ui";
import { computed, nextTick, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useChangesetWords } from "../changesets/words.ts";
import KLink from "../prefetch/KLink.vue";
import ProblemState from "../problems/ProblemState.vue";
import RefusalAlert from "../views/edit/RefusalAlert.vue";
import { editOfOperation } from "../views/edit/fields.ts";
import type { Refusal } from "../views/edit/inline.ts";
import { lineToReturn, openEdit } from "../views/edit/panel.ts";
import { useKeyLabel } from "../views/data.ts";
import LostSubmissions from "./LostSubmissions.vue";
import OperationText from "./OperationText.vue";
import { listedOperations, operationLine, withoutOperation } from "./operations.ts";
import { useDraftQueries } from "./state.ts";

const props = withDefaults(defineProps<{ id: string | null; page?: boolean }>(), {
  page: false,
});
const { t } = useI18n();
const words = useChangesetWords();
const keyLabel = useKeyLabel();
const { query, draft, operations, writable, simulated, violations } = useDraftQueries(
  () => props.id,
);

const general = computed(() => violations.value.get(-1) ?? []);
const counts = computed(() => simulated.value?.diff ?? []);
const cascaded = computed(() => simulated.value?.cascaded.length ?? 0);

// The operations listed, guards left out: each guard goes with the operation of its object,
// its violations said there, removed with it (Q-021-21).
const listed = computed(() => listedOperations(operations.value));
const violationsOf = (item: (typeof listed.value)[number]) =>
  [item.index, ...item.guards].flatMap((at) => violations.value.get(at) ?? []);

/** What a remove button says: the operation and what it names. */
function removeLabel(position: number): string {
  const line = operationLine(listed.value[position]!.operation);
  return t("drafts.remove", { n: position + 1, op: line.op, subject: line.subject ?? "" });
}
/** What a modify button says: the same words as its line, at its place in the list. */
function modifyLabel(position: number): string {
  const line = operationLine(listed.value[position]!.operation);
  return t("drafts.modify", { n: position + 1, op: line.op, subject: line.subject ?? "" });
}
/** The form of each operation, `null` for one no edit panel writes. */
const edits = computed(() => operations.value.map((operation) => editOfOperation(operation)));
/** Why « Modifier » is refused on a line: under `as_of`, or no panel for its operation. */
function modifyReason(index: number): string | undefined {
  if (!writable.value) return t("drafts.why.past");
  return edits.value[index] ? undefined : t("drafts.why.noPanel");
}
/**
 * « Modifier »: the edit panel of the kind, prefilled from the operation, which it replaces
 * at its place in the draft as read now (its version, for `If-Match`).
 */
function modify(index: number): void {
  const edit = edits.value[index];
  const read = query.data.value as Entry<Draft> | undefined;
  if (!edit || !read || read.data.id !== props.id || modifyReason(index)) return;
  const replace = { draft: read, index };
  openEdit(
    edit.mode === "create"
      ? { mode: "create", collection: edit.collection, replace }
      : { mode: "edit", collection: edit.collection, id: edit.id!, replace },
  );
}

const list = ref<HTMLElement | null>(null);
const empty = ref<HTMLElement | null>(null);
const none = ref<HTMLElement | null>(null);
const refusal = shallowRef<Refusal>();
const removing = ref<number | null>(null);
// The refusal of a removal stays said when the draft is gone with it (404), until another
// draft is shown.
watch(
  () => props.id,
  (id) => {
    if (id !== null) refusal.value = undefined;
  },
);
/**
 * The draft left the tab with the foot that held the focus (submitted, its retry
 * answered), or a retry of the notices was answered: the focus, lost with its button, goes
 * to what says the tab has no draft (2.4.3) — after a notice, to the « Réessayer » of the
 * next one still waiting first. A focus elsewhere stays.
 */
function focusNone(next = false): void {
  const active = document.activeElement;
  if (active && active !== document.body && active.isConnected) return;
  const retry = next
    ? none.value?.parentElement?.querySelector<HTMLElement>("[data-k-retry]")
    : null;
  (retry ?? none.value)?.focus();
}
watch(
  () => props.id,
  (id, before) => {
    if (id === null && before !== null) focusNone();
  },
  { flush: "post" },
);
/**
 * The edit of a line closed while the Détail tab had taken this one's place: shown again,
 * the focus to the « Modifier » of that line (2.4.3), else the list, else what says it is
 * empty or gone.
 */
const returning = lineToReturn();
/** The frames waited for the tab to show again (the panel keeps it mounted, hidden). */
let waited = 0;
function giveBack(): void {
  const line = returning.value;
  // Waited while the draft is read; gone or unread, the focus to what says so, and cleared.
  if (!line || (props.id !== null && !draft.value && !query.isError.value)) return;
  // The tab is being shown again: its panel loses `hidden` a frame later or so.
  const shown = list.value ?? empty.value ?? none.value;
  if (shown?.closest("[hidden]") && waited < 10) {
    waited++;
    requestAnimationFrame(giveBack);
    return;
  }
  waited = 0;
  returning.value = null;
  const buttons = list.value?.querySelectorAll<HTMLElement>("[data-k-modify]") ?? [];
  // The line's place in the list, its guards left out of it (Q-021-21).
  const position = listed.value.findIndex((item) => item.index === line.index);
  const own = line.draft === props.id && position >= 0 ? buttons[position] : undefined;
  (own ?? list.value ?? empty.value ?? none.value)?.focus();
}
watch([returning, draft, query.isError], giveBack, { flush: "post" });
onMounted(giveBack);
/** The draft is no longer the server's (deleted, submitted or taken over elsewhere). */
const refusedGone = computed(
  () => refusal.value?.kind === "failed" && refusal.value.problem?.status === 404,
);
/** What the refusal of a removal says: nothing typed is kept there. */
const refusalWords = computed(() =>
  refusedGone.value
    ? { title: t("drafts.goneRefused"), note: t("drafts.goneRefusedText") }
    : refusal.value?.kind === "conflict"
      ? { title: t("drafts.changed"), note: t("drafts.changedText") }
      : { note: null },
);
/**
 * Removes an operation (`PATCH /v1/drafts/{id}`, the others kept in order, under
 * `If-Match`); the focus goes to the button of the operation that takes its place, else the
 * one before, else the list.
 */
async function remove(index: number): Promise<void> {
  const at = listed.value[index]?.index;
  if (!props.id || at === undefined || removing.value !== null || !writable.value) return;
  removing.value = index;
  refusal.value = undefined;
  const kept = withoutOperation(operations.value, at);
  const outcome = await drafts.update(props.id, { operations: kept });
  removing.value = null;
  if (outcome.kind !== "saved") {
    refusal.value = outcome;
    // Gone (404): the tab has no draft now, its button with it; the focus to what says so.
    if (refusedGone.value) {
      await nextTick();
      none.value?.focus();
    }
    return;
  }
  await nextTick();
  const buttons = list.value?.querySelectorAll<HTMLElement>("[data-k-remove]") ?? [];
  (buttons[Math.min(index, buttons.length - 1)] ?? list.value ?? empty.value)?.focus();
}

const value = (text: string | null | undefined) =>
  text === null || text === undefined ? undefined : text;
</script>

<template>
  <Stack gap="3" class="k-draft-ops">
    <RefusalAlert v-if="refusal" :refusal="refusal" v-bind="refusalWords" />
    <template v-if="props.id === null">
      <LostSubmissions @answered="focusNone(true)" />
      <p ref="none" class="k-draft-ops__muted" tabindex="-1">{{ t("drafts.none") }}</p>
      <p>
        <KLink to="/drafts">{{ t("drafts.page") }}</KLink>
      </p>
    </template>
    <ProblemState
      v-else-if="query.isError.value"
      :title="t('drafts.readError')"
      :error="query.error.value"
      :retry="query.refetch"
    />
    <Skeleton v-else-if="!draft" variant="rows" :rows="3" />
    <template v-else>
      <InlineAlert v-if="!writable" tone="warn" icon="lock" :title="t('drafts.readOnly')" />
      <InlineAlert
        v-if="draft.expires_at"
        tone="warn"
        :title="t('drafts.expires', { when: words.when(draft.expires_at) })"
      >
        {{ t("drafts.expiresText") }}
      </InlineAlert>
      <p v-if="listed.length === 0" ref="empty" class="k-draft-ops__muted" tabindex="-1">
        {{ t("drafts.empty") }}
      </p>
      <ol
        v-else
        ref="list"
        class="k-draft-ops__list"
        tabindex="-1"
        :aria-label="t('drafts.operations', { title: draft.title })"
      >
        <li v-for="(item, index) in listed" :key="item.index" class="k-draft-ops__item">
          <div class="k-draft-ops__line">
            <span class="k-draft-ops__mark"><Icon name="draft" /></span>
            <OperationText :operation="item.operation" />
            <Cluster gap="1" :wrap="false">
              <IconButton
                data-k-modify
                icon="pencil"
                size="sm"
                variant="ghost"
                :label="modifyLabel(index)"
                v-bind="modifyReason(item.index) ? { reason: modifyReason(item.index) } : {}"
                @click="modify(item.index)"
              />
              <IconButton
                data-k-remove
                icon="x"
                size="sm"
                variant="ghost"
                :label="removeLabel(index)"
                :loading="removing === index"
                v-bind="writable ? {} : { reason: t('drafts.why.past') }"
                @click="remove(index)"
              />
            </Cluster>
          </div>
          <InlineAlert
            v-if="violationsOf(item).length"
            tone="ko"
            :title="t('drafts.refusedBySimulation', { n: index + 1 })"
            :items="violationsOf(item).map((v) => ({ message: v.detail, code: v.rule }))"
          />
        </li>
      </ol>
      <InlineAlert
        v-if="general.length"
        :tone="general.some((v) => v.severity === 'error') ? 'ko' : 'warn'"
        :title="t('drafts.resultViolations', general.length)"
        :items="general.map((v) => ({ message: v.detail, code: v.rule }))"
      />
      <section v-if="counts.length || cascaded" class="k-draft-ops__effect">
        <h3 class="k-draft-ops__subtitle">{{ t("drafts.effect") }}</h3>
        <ul class="k-draft-ops__counts">
          <li v-for="(count, index) in counts" :key="index">
            <span>{{ keyLabel("property", count.property) ?? count.property }}</span>
            <DiffValue v-if="count.before && count.after" kind="changed">
              <template #before>{{ count.before }}</template>
              {{ count.after }}
            </DiffValue>
            <DiffValue v-else-if="count.after" kind="added">{{ value(count.after) }}</DiffValue>
            <DiffValue v-else-if="count.before" kind="removed">{{ value(count.before) }}</DiffValue>
            <span class="k-num">{{ t("drafts.objects", { n: count.count }, count.count) }}</span>
          </li>
          <li v-if="cascaded">{{ t("drafts.cascaded", { n: cascaded }, cascaded) }}</li>
        </ul>
      </section>
      <p v-if="!props.page">
        <KLink to="/drafts">{{ t("drafts.page") }}</KLink>
      </p>
    </template>
  </Stack>
</template>

<style>
@layer components {
  .k-draft-ops {
    min-inline-size: 0;
  }

  .k-draft-ops__muted {
    color: var(--k-text-muted);
  }

  .k-draft-ops__list,
  .k-draft-ops__counts {
    display: grid;
    gap: var(--k-space-2);
    min-inline-size: 0;
  }

  .k-draft-ops__list:focus {
    outline: none;
  }

  .k-draft-ops__item {
    display: grid;
    gap: var(--k-space-1);
    min-inline-size: 0;
    padding-block-end: var(--k-space-2);
    border-block-end: var(--k-border-width) solid var(--k-border);
  }

  /* The mark of the draft, the operation which wraps, its button at its place at the end;
   * the first line centred on the button, the mark on that line (maquette E5). */
  .k-draft-ops__line {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: var(--k-space-2);
    align-items: start;
  }

  .k-draft-ops__line > .k-operation,
  .k-draft-ops__mark {
    margin-block-start: calc((var(--k-control-height-sm) - var(--k-text-md-line)) / 2);
  }

  .k-draft-ops__mark {
    display: inline-flex;
    align-items: center;
    block-size: var(--k-text-md-line);
  }

  .k-draft-ops__counts > li {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-1) var(--k-space-2);
    align-items: baseline;
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }

  .k-draft-ops__subtitle {
    margin-block-end: var(--k-space-1);
    font-size: var(--k-text-md);
    font-weight: var(--k-weight-semibold);
  }
}
</style>
