<script setup lang="ts">
import type { Operation } from "@kartenn/sdk";
import { computed, reactive, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { useWrites } from "../time/index.ts";
import { useDraft, type DraftViolation } from "./index.ts";

// The drawer of the draft (REQ-007-26, design « Écriture »): its operations, each editable
// as the JSON of the contract, the motive and the period, the last simulation, the
// submission. Non-modal: the pages stay usable while it is open.

const { t } = useI18n();
const id = useId();
const draft = useDraft();
const state = draft.state;
const writes = useWrites();
const writable = computed(() => writes.value.draft);
const operations = computed(() => state.record?.operations ?? []);
const newOperation = ref("");
const newError = ref<string | null>(null);
const editErrors = reactive<Record<number, string>>({});

function parse(text: string): { op: Operation } | { error: string } {
  try {
    const value = JSON.parse(text) as unknown;
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      return { error: t("draft.notObject") };
    }
    return { op: value as Operation };
  } catch (error) {
    return { error: error instanceof Error ? error.message : String(error) };
  }
}

function add(): void {
  const parsed = parse(newOperation.value);
  if ("error" in parsed) {
    newError.value = parsed.error;
    return;
  }
  newError.value = null;
  newOperation.value = "";
  void draft.add(parsed.op);
}

function edit(index: number, text: string): void {
  const parsed = parse(text);
  if ("error" in parsed) {
    editErrors[index] = parsed.error;
    return;
  }
  delete editErrors[index];
  void draft.replace(index, parsed.op);
}

function removeAt(index: number): void {
  delete editErrors[index];
  void draft.removeAt(index);
}

const report = computed(() =>
  state.simulation.status === "done" ? state.simulation.report : null,
);
function violationsOf(index: number): DraftViolation[] {
  return report.value?.violations.filter((v) => v.operation === index) ?? [];
}
const general = computed(
  () =>
    report.value?.violations.filter((v) => v.operation === null || v.operation === undefined) ?? [],
);
const errors = computed(() => report.value?.violations.filter((v) => v.severity === "error") ?? []);
const warnings = computed(
  () => report.value?.violations.filter((v) => v.severity !== "error") ?? [],
);
const canSubmit = computed(() => draft.canSubmit(writable.value));
</script>

<template>
  <aside :id="'k-draft'" class="k-draft" :aria-labelledby="`${id}-title`">
    <header class="k-draft-header">
      <h2 :id="`${id}-title`">{{ t("draft.title") }}</h2>
      <button type="button" @click="state.open = false">{{ t("draft.close") }}</button>
    </header>
    <p v-if="!state.available" class="k-notice">{{ t("draft.unavailable") }}</p>
    <template v-else-if="state.record">
      <p v-if="!writable" class="k-notice">{{ t("draft.readonly") }}</p>
      <p v-if="operations.length === 0">{{ t("draft.empty") }}</p>
      <ol class="k-draft-operations">
        <li v-for="(operation, index) in operations" :key="index">
          <label>
            {{ t("draft.operation", { n: index + 1 }) }}
            <textarea
              :value="JSON.stringify(operation, null, 2)"
              rows="4"
              spellcheck="false"
              :disabled="!writable"
              :aria-invalid="editErrors[index] || violationsOf(index).length > 0 ? true : undefined"
              :aria-describedby="`${id}-op-${index}`"
              @change="edit(index, ($event.target as HTMLTextAreaElement).value)"
            />
          </label>
          <div :id="`${id}-op-${index}`">
            <p v-if="editErrors[index]" class="k-error">
              {{ t("draft.invalidJson", { detail: editErrors[index] }) }}
            </p>
            <ul v-if="violationsOf(index).length > 0" class="k-draft-violations">
              <li
                v-for="(violation, v) in violationsOf(index)"
                :key="v"
                :class="violation.severity === 'error' ? 'k-error' : 'k-warning'"
              >
                {{ violation.rule }} — {{ violation.detail }}
              </li>
            </ul>
          </div>
          <button type="button" :disabled="!writable" @click="removeAt(index)">
            {{ t("draft.remove", { n: index + 1 }) }}
          </button>
        </li>
      </ol>
      <form class="k-form" @submit.prevent="add">
        <label>
          {{ t("draft.add.label") }}
          <textarea
            v-model="newOperation"
            rows="3"
            spellcheck="false"
            :disabled="!writable"
            :aria-invalid="newError ? true : undefined"
            :aria-describedby="newError ? `${id}-new-error` : undefined"
          />
        </label>
        <p v-if="newError" :id="`${id}-new-error`" class="k-error">
          {{ t("draft.invalidJson", { detail: newError }) }}
        </p>
        <p class="k-toolbar">
          <button type="submit" :disabled="!writable || newOperation.trim() === ''">
            {{ t("draft.add.button") }}
          </button>
        </p>
      </form>
      <form class="k-form" @submit.prevent="draft.submit(writable)">
        <label>
          {{ t("draft.motive") }}
          <textarea
            :value="state.record.motive"
            rows="2"
            :disabled="!writable"
            @input="draft.setMotive(($event.target as HTMLTextAreaElement).value)"
          />
        </label>
        <label>
          {{ t("draft.validFrom") }}
          <input
            type="text"
            :value="state.record.valid_from ?? ''"
            :disabled="!writable"
            @change="
              draft.setPeriod(
                ($event.target as HTMLInputElement).value.trim() || null,
                state.record?.valid_to ?? null,
              )
            "
          />
        </label>
        <label>
          {{ t("draft.validTo") }}
          <input
            type="text"
            :value="state.record.valid_to ?? ''"
            :disabled="!writable"
            @change="
              draft.setPeriod(
                state.record?.valid_from ?? null,
                ($event.target as HTMLInputElement).value.trim() || null,
              )
            "
          />
        </label>
        <div class="k-draft-simulation" role="status">
          <p v-if="state.simulation.status === 'idle'">{{ t("draft.simulation.idle") }}</p>
          <p v-else-if="state.simulation.status === 'running'">
            {{ t("draft.simulation.running") }}
          </p>
          <p v-else-if="state.simulation.status === 'failed'" class="k-error">
            {{ t("draft.simulation.failed", { detail: state.simulation.detail }) }}
          </p>
          <template v-else-if="report">
            <p v-if="report.valid">
              {{
                t("draft.simulation.valid", {
                  diff: report.diff.length,
                  cascaded: report.cascaded.length,
                })
              }}
            </p>
            <p v-else class="k-error">
              {{ t("draft.simulation.invalid", { count: errors.length }) }}
            </p>
            <p v-if="warnings.length > 0">
              {{ t("draft.simulation.warnings", { count: warnings.length }) }}
            </p>
            <ul v-if="general.length > 0" class="k-draft-violations">
              <li
                v-for="(violation, v) in general"
                :key="v"
                :class="violation.severity === 'error' ? 'k-error' : 'k-warning'"
              >
                {{ violation.rule }} — {{ violation.detail }}
              </li>
            </ul>
          </template>
        </div>
        <div role="status">
          <p v-if="state.submission.status === 'applied' || state.submission.status === 'pending'">
            {{ t(`draft.${state.submission.status}`) }}
            <RouterLink
              v-if="state.submission.changeset"
              :to="`/changesets/${state.submission.changeset}`"
            >
              {{ t("draft.changeset") }}
            </RouterLink>
          </p>
          <p
            v-else-if="state.submission.status === 'failed' && state.submission.network"
            class="k-error"
          >
            {{ t("draft.network") }}
          </p>
          <p v-else-if="state.submission.status === 'failed'" class="k-error">
            {{ t("draft.failed", { detail: state.submission.detail }) }}
          </p>
        </div>
        <p class="k-toolbar">
          <button type="submit" :disabled="!canSubmit">
            {{ state.submission.status === "failed" ? t("draft.retry") : t("draft.submit") }}
          </button>
          <button
            type="button"
            :disabled="!writable || operations.length === 0"
            @click="draft.clear()"
          >
            {{ t("draft.clear") }}
          </button>
        </p>
      </form>
    </template>
  </aside>
</template>
