<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The period picker (design « Inventaire », line 7; spec 021, « Cadre »; REQ-007-22, 23):
// the dated context of a read, `valid_at` (what was true at a date) and `as_of` (what the
// base knew at a date), each a `DateTimeField` to the minute in a named zone, empty for
// the present; shortcuts « Maintenant », « Hier » (the same time a day before) and
// « Début du mois » (its first day at midnight) under each, said by a polite region. Sent,
// an impossible entry is refused with its reason on its field, as is an `as_of` later than
// now (the system time has no future, 006 « Lectures datées »): the focus goes to it.
// That check reads the clock of the browser, strictly; the API compares with the `now()`
// of the base, and its refusal (a 400 on a date) comes back on its field by `errors`.
// Away from the present, « Revenir au présent » clears both. The layer around it (under
// the date pill, spec 021) is the host's, as are Escape and the focus given back.
import { nextTick, reactive, ref, useId, watch } from "vue";
import { fill, t, type MessageKey } from "../../i18n.ts";
import { formatInstant } from "../../time.ts";
import Button from "../Button/Button.vue";
import DateTimeField from "../DateTimeField/DateTimeField.vue";
import { shortcut, type Period } from "../DateTimeField/dates.ts";

const props = withDefaults(
  defineProps<{
    validAt?: string | null;
    asOf?: string | null;
    /** The named zone (IANA) in which the dates are typed and shown. */
    timeZone: string;
    /**
     * The id of its title, given when the layer around it is named by it: the form is
     * then not named a second time (a form landmark of its own otherwise).
     */
    titleId?: string | undefined;
    /** The host's refusal of a date (the API's, with its reason), shown on its field. */
    errors?: Partial<Record<keyof Period, string>> | undefined;
  }>(),
  { validAt: null, asOf: null, titleId: undefined, errors: undefined },
);
const emit = defineEmits<{ apply: [period: Period] }>();

const titleId = props.titleId ?? useId();
const formLabel = props.titleId ? undefined : titleId;

type Axis = keyof Period;
const AXES: { key: Axis; label: MessageKey; hint: MessageKey }[] = [
  { key: "validAt", label: "periodValidAt", hint: "periodValidAtHint" },
  { key: "asOf", label: "periodAsOf", hint: "periodAsOfHint" },
];
const SHORTCUTS: { kind: "now" | "yesterday" | "monthStart"; label: MessageKey }[] = [
  { kind: "now", label: "periodNow" },
  { kind: "yesterday", label: "periodYesterday" },
  { kind: "monthStart", label: "periodMonthStart" },
];

const draft = reactive<Period>({ validAt: props.validAt, asOf: props.asOf });
watch(
  () => [props.validAt, props.asOf] as const,
  ([validAt, asOf]) => Object.assign(draft, { validAt, asOf }),
);

/** What a `DateTimeField` exposes. */
type Field = { check(): boolean; focus(): void };
const fields: Record<Axis, Field | null> = { validAt: null, asOf: null };
const asOfError = ref<string>();
watch(
  () => draft.asOf,
  () => (asOfError.value = undefined),
);

// The host's refusal comes back after the sending (a 400 of the API): the focus goes to
// the refused field, as for a refusal of its own, its reason read with it.
watch(
  () => props.errors,
  async (errors) => {
    const axis = AXES.find((a) => errors?.[a.key])?.key;
    if (!axis) return;
    await nextTick();
    fields[axis]?.focus();
  },
);

const said = ref("");

function useShortcut(axis: Axis, kind: (typeof SHORTCUTS)[number]["kind"]): void {
  const value = shortcut(kind, Date.now(), props.timeZone);
  draft[axis] = value;
  const label = t(AXES.find((a) => a.key === axis)!.label);
  // Emptied first, so that the same sentence twice is said twice.
  said.value = "";
  void nextTick(() => {
    said.value = fill("periodSet", { label, value: formatInstant(value, props.timeZone) });
  });
}

function apply(): void {
  const validOk = fields.validAt?.check() ?? true;
  const asOfOk = fields.asOf?.check() ?? true;
  if (!validOk) return fields.validAt?.focus();
  if (!asOfOk) return fields.asOf?.focus();
  if (draft.asOf && Date.parse(draft.asOf) > Date.now()) {
    asOfError.value = t("periodAsOfFuture");
    return fields.asOf?.focus();
  }
  emit("apply", { validAt: draft.validAt, asOf: draft.asOf });
}

const offPresent = () => props.validAt !== null || props.asOf !== null;
</script>

<template>
  <form class="k-period-picker" :aria-labelledby="formLabel" novalidate @submit.prevent="apply">
    <h2 :id="titleId" class="k-period-picker__title">{{ t("periodTitle") }}</h2>
    <div v-for="axis in AXES" :key="axis.key" class="k-period-picker__axis">
      <DateTimeField
        :ref="(el) => (fields[axis.key] = el as Field | null)"
        v-model="draft[axis.key]"
        :label="t(axis.label)"
        :hint="t(axis.hint)"
        :time-zone="props.timeZone"
        :error="(axis.key === 'asOf' ? asOfError : undefined) ?? props.errors?.[axis.key]"
      />
      <div
        class="k-period-picker__shortcuts"
        role="group"
        :aria-label="`${t('periodShortcuts')}${t('colon')} ${t(axis.label)}`"
      >
        <Button
          v-for="item in SHORTCUTS"
          :key="item.kind"
          size="sm"
          variant="ghost"
          @click="useShortcut(axis.key, item.kind)"
          >{{ t(item.label) }}</Button
        >
      </div>
    </div>
    <span class="k-visually-hidden" role="status" aria-live="polite">{{ said }}</span>
    <!-- The primary one last, as in `Dialog` and `FilterPicker`. -->
    <div class="k-period-picker__actions">
      <Button
        v-if="offPresent()"
        variant="ghost"
        icon="undo-2"
        @click="emit('apply', { validAt: null, asOf: null })"
        >{{ t("backToPresent") }}</Button
      >
      <Button type="submit" variant="primary">{{ t("periodApply") }}</Button>
    </div>
  </form>
</template>

<style>
@layer components {
  .k-period-picker {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-3);
    max-inline-size: var(--k-size-panel-lg);
  }

  .k-period-picker__title {
    margin: 0;
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
    font-weight: var(--k-weight-semibold);
  }

  .k-period-picker__axis {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-1);
  }

  /* The words of the ghost buttons on the edge of the field above (their padding outside). */
  .k-period-picker__shortcuts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    margin-inline-start: calc(var(--k-control-padding-inline-sm) * -1);
  }

  .k-period-picker__actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    justify-content: end;
  }
}
</style>
