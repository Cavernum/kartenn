<script setup lang="ts">
// The follow-up of an import, in the Détail tab of the right panel (spec 006, REQ-006-17 to
// 19, H18; REQ-007-19, REQ-021-12): its state, read again every second while its task
// sorts it, holds it ready or runs it (no event of the stream says an import); its chunks,
// elements and batches; why its sort failed (006/H18: back to `uploading`, the violations
// by the rank of their element); the report of its batches. « Reprendre » a paused one,
// « Annuler l'import » (confirmed: a cancelled import does not resume; the batches applied
// stay), written at once by their routes: neither is an operation of a changeset.
import { api, keys, reads } from "@kartenn/sdk";
import {
  AlertDialog,
  InlineAlert,
  Skeleton,
  Stack,
  StatusBadge,
  Toolbar,
  type ToolbarAction,
} from "@kartenn/ui";
import { computed, nextTick, ref, shallowRef, useTemplateRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery, useQueryClient } from "../data/query.ts";
import ProblemState from "../problems/ProblemState.vue";
import { useProblems, type ProblemLine } from "../problems/index.ts";
import { useTime } from "../time/index.ts";
import { keepRecordName } from "../views/names.ts";
import AdminLines from "./AdminLines.vue";
import {
  IMPORT_POLL_MS,
  importBadge,
  importBusy,
  importOpen,
  importReport,
  sortViolations,
  type ImportItem,
} from "./imports.ts";

const props = defineProps<{ id: string }>();
const { t, te, locale } = useI18n();
const time = useTime();
const queryClient = useQueryClient();
const problems = useProblems();

const read = useQuery(() => ({
  ...reads.read("/v1/imports/{id}", { params: { id: props.id } }),
  refetchInterval: (query: { state: { data?: { data: ImportItem } | undefined } }) =>
    importBusy(query.state.data?.data) ? IMPORT_POLL_MS : false,
}));
const item = computed(() => read.data.value?.data as ImportItem | undefined);
watch(
  () => item.value?.source,
  (source) => source && keepRecordName(props.id, source),
  { immediate: true },
);
// Its state changed: the list of the imports shows it.
watch(
  () => item.value?.state,
  (now, before) => {
    if (before !== undefined && now !== before) {
      void queryClient.invalidateQueries({ queryKey: keys.allOn("/v1/imports") });
    }
  },
);

const number = computed(() => new Intl.NumberFormat(locale.value));
const stateWords = (state: string) => t(`admin.imports.states.${state}`);
const report = computed(() => (item.value ? importReport(item.value) : null));

/** Why the sort failed: each violation in words by its rule, then the problem and its code. */
const sortLines = computed<ProblemLine[]>(() => {
  const now = item.value;
  if (!now?.error) return [];
  const violations = sortViolations(now).map((v): ProblemLine => {
    const key = `admin.imports.violations.${v.rule}`;
    const message = te(key) ? t(key, { element: v.element ?? "" }) : v.detail;
    return { message, code: v.rule };
  });
  return [
    ...violations,
    ...(now.error.type ? [{ message: t("problem.code"), code: now.error.type }] : []),
  ];
});

// « Reprendre » and « Annuler l'import », written at once.
const busy = ref(false);
const writeError = shallowRef<unknown>(null);
const writeTitle = ref("");
const confirming = ref(false);
const heading = useTemplateRef<HTMLElement>("heading");
const pastRefusal = computed(() => (time.value.present ? {} : { reason: t("edit.why.past") }));
const actions = computed<ToolbarAction[]>(() => {
  const now = item.value;
  if (!now || !importOpen(now)) return [];
  return [
    ...(now.state === "paused"
      ? [
          {
            id: "resume",
            label: t("admin.imports.resume"),
            icon: "refresh-cw",
            variant: "primary",
            ...pastRefusal.value,
          } as ToolbarAction,
        ]
      : []),
    { id: "cancel", label: t("admin.imports.cancel"), icon: "circle-x", ...pastRefusal.value },
  ];
});
async function steer(action: "resume" | "cancel"): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  writeError.value = null;
  try {
    // No body (the contract types none): by URL.
    await api.request("POST", `/v1/imports/${encodeURIComponent(props.id)}/${action}`);
  } catch (error) {
    writeError.value = error;
    writeTitle.value = t(
      action === "resume" ? "admin.imports.resumeFailed" : "admin.imports.cancelFailed",
    );
  } finally {
    busy.value = false;
  }
  await queryClient.invalidateQueries({ queryKey: keys.allOn("/v1/imports/{id}") });
  // Its action gone with the state it served (resumed, cancelled): the focus on the import.
  await nextTick();
  const active = document.activeElement;
  if (!active || active === document.body || !active.isConnected) heading.value?.focus();
}
function onAction(id: string): void {
  if (id === "resume") void steer("resume");
  else if (id === "cancel") confirming.value = true;
}
</script>

<template>
  <ProblemState
    v-if="read.isError.value && !item"
    :title="t('admin.imports.readError')"
    :error="read.error.value"
    :retry="read.refetch"
  />
  <Skeleton v-else-if="!item" variant="rows" :rows="6" />
  <Stack v-else gap="4" class="k-admin-detail">
    <Stack gap="2">
      <h2 ref="heading" class="k-admin-heading k-mono" tabindex="-1">{{ item.source }}</h2>
      <p role="status" class="k-admin-state">
        <span class="k-visually-hidden">{{ t("admin.imports.stateIs") }}</span>
        <StatusBadge :state="importBadge(item.state)" :label="stateWords(item.state)" />
      </p>
      <Toolbar
        v-if="actions.length > 0"
        :label="t('admin.imports.actions')"
        :actions="actions"
        align="start"
        @action="onAction"
      />
      <InlineAlert
        v-if="writeError !== null"
        tone="ko"
        :title="writeTitle"
        :items="problems.lines(writeError)"
      />
      <!-- A read again that failed: the import as last read, the failure said above it. -->
      <InlineAlert
        v-if="read.isError.value"
        tone="warn"
        :title="t('admin.imports.stale')"
        :items="problems.lines(read.error.value)"
      />
    </Stack>

    <AdminLines
      :label="t('admin.imports.facts', { source: item.source })"
      read
      class="k-admin-facts"
    >
      <li>
        <span>{{ t("admin.imports.chunks") }}</span>
        <span class="k-admin-lines__value k-num">{{ number.format(item.chunks) }}</span>
      </li>
      <li>
        <span>{{ t("admin.imports.elements") }}</span>
        <span class="k-admin-lines__value k-num">{{ number.format(item.elements) }}</span>
      </li>
      <li>
        <span>{{ t("admin.imports.batches") }}</span>
        <span class="k-admin-lines__value k-num">{{
          t("admin.imports.applied", {
            applied: number.format(item.batches.applied),
            remaining: number.format(item.batches.remaining),
          })
        }}</span>
      </li>
      <li>
        <span>{{ t("admin.imports.id") }}</span>
        <span class="k-admin-lines__value k-mono">{{ item.id }}</span>
      </li>
    </AdminLines>

    <InlineAlert
      v-if="item.error"
      tone="warn"
      :title="t('admin.imports.sortFailed')"
      :items="sortLines"
    >
      {{ t("admin.imports.sortFailedText") }}
    </InlineAlert>

    <section v-if="report">
      <Stack gap="2">
        <h3 class="k-admin-heading">
          {{ t("admin.imports.report") }}
        </h3>
        <pre
          class="k-admin-json k-mono"
          tabindex="0"
          role="region"
          :aria-label="t('admin.imports.report')"
          >{{ report }}</pre>
      </Stack>
    </section>

    <AlertDialog
      v-model:open="confirming"
      :title="t('admin.imports.cancelTitle', { source: item.source })"
      :description="t('admin.imports.cancelText')"
      :confirm="t('admin.imports.cancel')"
      :cancel="t('admin.imports.keep')"
      danger
      @confirm="steer('cancel')"
    />
  </Stack>
</template>

<style>
@layer components {
  .k-admin-state {
    display: inline-flex;
    margin: 0;
  }
}
</style>
