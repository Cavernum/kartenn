<script setup lang="ts">
import { computed, onBeforeUnmount, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { read, send, type Problem } from "../data/api.ts";
import { problemText } from "../i18n/index.ts";
import { useWrites } from "../time/index.ts";
import { IMPORT_BUSY, IMPORT_OPEN, type Import, type ImportError } from "./admin.ts";

// The follow-up of an import (REQ-007-19, REQ-006-17 to 19): its state read again while
// its task sorts or runs it, the failure that paused it, why its sort failed (006/H18),
// resumed or cancelled.

/** Reading again while the import sorts, is ready or runs. */
const POLL_MS = 1_000;

const i18n = useI18n();
const { t } = i18n;
const route = useRoute();
const writes = useWrites();
const id = computed(() => String(route.params.id));
const item = shallowRef<Import | null>(null);
const problem = shallowRef<Problem | null>(null);
const outcome = shallowRef<Problem | null>(null);
const busy = ref(false);
let timer: ReturnType<typeof setTimeout> | null = null;

async function load(): Promise<void> {
  if (timer) clearTimeout(timer);
  timer = null;
  const answer = await read<Import>(`/v1/imports/${id.value}`);
  item.value = answer.ok ? answer.body : null;
  problem.value = answer.ok ? null : answer.problem;
  if (answer.ok && (IMPORT_BUSY as readonly string[]).includes(answer.body.state)) {
    timer = setTimeout(() => void load(), POLL_MS);
  }
}
watch(id, () => void load(), { immediate: true });
onBeforeUnmount(() => {
  if (timer) clearTimeout(timer);
});

async function steer(action: "resume" | "cancel"): Promise<void> {
  busy.value = true;
  const answer = await send<Import>("POST", `/v1/imports/${id.value}/${action}`);
  busy.value = false;
  outcome.value = answer.ok ? null : answer.problem;
  await load();
}

const open = computed(() =>
  item.value ? (IMPORT_OPEN as readonly string[]).includes(item.value.state) : false,
);
const report = computed(() =>
  item.value && Object.keys(item.value.report).length > 0
    ? JSON.stringify(item.value.report, null, 2)
    : null,
);
type Violation = NonNullable<ImportError["violations"]>[number];
/** A violation of the sort, in words: by its rule when known, else as the server says. */
function violationText(v: Violation): string {
  const key = `imports.violations.${v.rule}`;
  return i18n.te(key) ? t(key, { element: v.element ?? "" }) : v.detail;
}
</script>

<template>
  <p v-if="problem" role="alert" class="k-error">{{ problemText(i18n, problem) }}</p>
  <template v-else-if="item">
    <h1>{{ t("imports.title", { source: item.source }) }}</h1>
    <p role="status">{{ t("imports.stateIs", { state: t(`imports.states.${item.state}`) }) }}</p>
    <dl class="k-fields">
      <dt>{{ t("imports.chunks") }}</dt>
      <dd>{{ item.chunks }}</dd>
      <dt>{{ t("imports.elements") }}</dt>
      <dd>{{ item.elements }}</dd>
      <dt>{{ t("imports.batches") }}</dt>
      <dd>{{ t("imports.applied", item.batches) }}</dd>
      <dt>{{ t("field.id") }}</dt>
      <dd>{{ item.id }}</dd>
    </dl>
    <template v-if="item.error">
      <h2>{{ t("imports.sortFailed") }}</h2>
      <p role="alert" class="k-error">{{ problemText(i18n, item.error) }}</p>
      <ul v-if="item.error.violations?.length">
        <li v-for="(v, i) in item.error.violations" :key="i">{{ violationText(v) }}</li>
      </ul>
    </template>
    <template v-if="report">
      <h2>{{ t("imports.report") }}</h2>
      <pre class="k-mono" tabindex="0">{{ report }}</pre>
    </template>
    <div v-if="open" class="k-toolbar">
      <button
        v-if="item.state === 'paused'"
        type="button"
        :disabled="busy || !writes.quickEdit"
        @click="steer('resume')"
      >
        {{ t("imports.resume") }}
      </button>
      <button type="button" :disabled="busy || !writes.quickEdit" @click="steer('cancel')">
        {{ t("imports.cancel") }}
      </button>
    </div>
    <p v-if="outcome" role="alert" class="k-error">{{ problemText(i18n, outcome) }}</p>
  </template>
</template>
