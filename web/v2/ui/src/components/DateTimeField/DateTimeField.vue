<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A date, or a date and a time, by segments (design « Inventaire », line 7): year, month,
// day, hour, minute in the order of ISO 8601, each an APG Spinbutton (Up and Down step and
// wrap, Home and End go to its bounds, digits typed fill it and go on to the next, a
// separator too; Left and Right move between them, Backspace erases); a named zone shown
// beside the time, with its offset at that date; a button that opens the month grid in a
// modal dialog (APG Date Picker Dialog; Alt+Down from a segment opens it too), named by
// its button (Reka names the popover by its trigger: « Choisir la date : <label> »), its
// grid by the month shown. The value
// is a date `YYYY-MM-DD` or an instant of RFC 3339 in UTC; `null` while the entry is
// empty, incomplete or impossible. An impossible entry (31 février, 25 h, the hour skipped
// when the clocks go forward, an instant whose year in UTC leaves 0001…9999), typed or
// pasted, is kept as typed and refused: its reason comes when the
// field is left (or on `check()`, before a form is sent), linked to the field and said
// (`FormField`), on the segment at fault.
import { PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from "reka-ui";
import { computed, nextTick, onMounted, reactive, ref, useId, watch } from "vue";
import { injectPortalTarget } from "../../context.ts";
import { t, type MessageKey } from "../../i18n.ts";
import FormField from "../FormField/FormField.vue";
import IconButton from "../IconButton/IconButton.vue";
import type { ControlSize } from "../layout.ts";
import DateCalendar from "./DateCalendar.vue";
import {
  checkEntry,
  daysIn,
  emptyEntry,
  entryOf,
  instantsOf,
  LIMITS,
  monthName,
  offsetLabel,
  parseEntry,
  pastedEntry,
  SEGMENTS,
  wallAt,
  type DateGranularity,
  type Entry,
  type SegmentKind,
} from "./dates.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    /** The label stays for assistive technologies only (a cell editor). */
    hideLabel?: boolean;
    hint?: string | undefined;
    /** An error of the host (a bound, a rule of the form); the field's own comes first. */
    error?: string | undefined;
    required?: boolean;
    readonly?: boolean;
    disabled?: boolean;
    size?: ControlSize;
    /** A day only, or a day and a time to the minute (default). */
    granularity?: DateGranularity;
    /** The named zone of the wall clock (IANA); the browser's by default, always shown. */
    timeZone?: string | undefined;
    /**
     * An entry as typed, in the order of ISO 8601 (`2026-02-31 10:30`), shown and checked
     * at first render instead of the value (a value read from a URL, the catalogue).
     */
    defaultEntry?: string | undefined;
    /** Open on its month grid at first render (the catalogue). */
    defaultOpen?: boolean;
  }>(),
  {
    hint: undefined,
    error: undefined,
    size: "md",
    granularity: "minute",
    timeZone: undefined,
    defaultEntry: undefined,
  },
);
/** The value: `YYYY-MM-DD`, or an instant of RFC 3339 in UTC; `null` without a valid entry. */
const model = defineModel<string | null>({ default: null });

const portal = injectPortalTarget();
const id = useId();
const zoneId = `${id}-zone`;
const headingId = `${id}-calendar`;

const zone = computed(() => props.timeZone ?? Intl.DateTimeFormat().resolvedOptions().timeZone);
const kinds = computed(() => SEGMENTS[props.granularity]);
const editable = computed(() => !props.readonly && !props.disabled);

const entry = reactive<Entry>(
  props.defaultEntry !== undefined
    ? parseEntry(props.defaultEntry)
    : (entryOf(model.value, props.granularity, zone.value) ?? emptyEntry()),
);
/** The field's own refusal, and the segment it concerns. */
const own = reactive<{ reason?: string | undefined; segment?: SegmentKind | undefined }>({});
const shownError = computed(() => own.reason ?? props.error);

// The value last written, so that its echo by the host does not reset the entry.
let written: string | null = model.value;
watch(model, (value) => {
  if (value === written) return;
  written = value;
  Object.assign(entry, entryOf(value, props.granularity, zone.value) ?? emptyEntry());
  own.reason = own.segment = undefined;
});
watch(zone, () => {
  const read = entryOf(model.value, props.granularity, zone.value);
  if (read) Object.assign(entry, read);
});

const LABEL: Record<SegmentKind, MessageKey> = {
  year: "dateYear",
  month: "dateMonth",
  day: "dateDay",
  hour: "dateHour",
  minute: "dateMinute",
};
const HOLE: Record<SegmentKind, MessageKey> = {
  year: "dateHoleYear",
  month: "dateHoleMonth",
  day: "dateHoleDay",
  hour: "dateHoleHour",
  minute: "dateHoleMinute",
};
const SEPARATORS = new Set(["-", "/", ":", ".", " ", "T", "t"]);

/** The upper bound of a segment: the days of the month typed, else 31. */
function maxOf(kind: SegmentKind): number {
  if (kind !== "day") return LIMITS[kind].max;
  const year = Number(entry.year);
  const month = Number(entry.month);
  return year >= 1 && month >= 1 && month <= 12 ? daysIn(year, month) : LIMITS.day.max;
}

const pad = (kind: SegmentKind, text: string) =>
  text === "" ? "" : text.padStart(LIMITS[kind].digits, "0");

/** The wall clock now in the zone, where Up or Down starts on an empty segment. */
const now = () => wallAt(Date.now(), zone.value);

/** After each change: the value written (or `null`), the refusal lifted once mended. */
function changed(): void {
  const result = checkEntry(entry, props.granularity, zone.value);
  const value = result.ok ? result.value : null;
  if (value !== written) {
    written = value;
    model.value = value;
  }
  if (own.reason && result.ok) own.reason = own.segment = undefined;
}

/**
 * Checks the entry and shows its refusal, if any (as the field is left, before a form is
 * sent): `true` when it holds a value or nothing.
 */
function check(): boolean {
  for (const kind of kinds.value) entry[kind] = pad(kind, entry[kind]);
  const result = checkEntry(entry, props.granularity, zone.value);
  own.reason = result.ok ? undefined : result.reason;
  own.segment = result.ok ? undefined : result.segment;
  return result.ok;
}

// Segments: one in the tab order each (as a native date field), the arrows between them.
const segments = ref<HTMLElement[]>([]);
/** The segment being typed in: its first digit replaces what it held. */
let fresh = true;

function focusSegment(index: number): void {
  const target = segments.value[Math.max(0, Math.min(index, kinds.value.length - 1))];
  target?.focus();
}

/** Puts the focus on the segment at fault, else the first (after a refusal). */
function focus(): void {
  focusSegment(own.segment ? kinds.value.indexOf(own.segment) : 0);
}

function typeDigit(index: number, digit: string): void {
  const kind = kinds.value[index]!;
  const { digits, max } = LIMITS[kind];
  const text = fresh ? digit : entry[kind] + digit;
  fresh = false;
  entry[kind] = text;
  changed();
  // Complete when it has its digits, or when no further digit would stay in bounds
  // (« 4 » in the month is April). An out-of-bounds number (25 h) is kept and refused.
  if (text.length >= digits || (kind !== "year" && Number(`${text}0`) > max)) {
    entry[kind] = pad(kind, text);
    if (index < kinds.value.length - 1) focusSegment(index + 1);
    else fresh = true;
  }
}

function step(index: number, delta: number): void {
  const kind = kinds.value[index]!;
  const { min } = LIMITS[kind];
  const max = maxOf(kind);
  let value: number;
  if (entry[kind] === "") value = now()[kind];
  else {
    value = Number(entry[kind]) + delta;
    if (value > max) value = min;
    if (value < min) value = max;
  }
  entry[kind] = pad(kind, String(value));
  fresh = true;
  changed();
}

function erase(index: number): void {
  const kind = kinds.value[index]!;
  if (entry[kind] === "") return focusSegment(index - 1);
  entry[kind] = fresh ? "" : entry[kind].slice(0, -1);
  fresh = false;
  changed();
}

function onKeydown(event: KeyboardEvent, index: number): void {
  const kind = kinds.value[index]!;
  if (event.key === "ArrowLeft" || event.key === "ArrowRight") {
    event.preventDefault();
    focusSegment(index + (event.key === "ArrowLeft" ? -1 : 1));
    return;
  }
  if (!editable.value) return;
  if (event.altKey && event.key === "ArrowDown") {
    event.preventDefault();
    open.value = true;
    return;
  }
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  const keys: Record<string, () => void> = {
    ArrowUp: () => step(index, 1),
    ArrowDown: () => step(index, -1),
    Home: () => {
      entry[kind] = pad(kind, String(LIMITS[kind].min));
      fresh = true;
      changed();
    },
    End: () => {
      entry[kind] = pad(kind, String(maxOf(kind)));
      fresh = true;
      changed();
    },
    Backspace: () => erase(index),
    Delete: () => {
      entry[kind] = "";
      fresh = true;
      changed();
    },
    // A segment is no input: Enter sends the form around it, as a field would.
    Enter: () => (event.target as HTMLElement).closest("form")?.requestSubmit(),
  };
  const action = keys[event.key];
  if (action) {
    event.preventDefault();
    action();
  } else if (/^\d$/.test(event.key)) {
    event.preventDefault();
    typeDigit(index, event.key);
  } else if (SEPARATORS.has(event.key)) {
    event.preventDefault();
    if (entry[kind] !== "" && !fresh) {
      entry[kind] = pad(kind, entry[kind]);
      focusSegment(index + 1);
    }
  } else if (event.key.length === 1) {
    // Nothing else goes into a segment.
    event.preventDefault();
  }
}

/** A virtual keyboard sends no key, only its input: the same, from its text. */
function onBeforeInput(event: InputEvent, index: number): void {
  event.preventDefault();
  if (!editable.value) return;
  if (event.inputType.startsWith("delete")) return erase(index);
  for (const char of event.data ?? "") {
    const at = segments.value.indexOf(document.activeElement as HTMLElement);
    if (/^\d$/.test(char)) typeDigit(at < 0 ? index : at, char);
  }
}

/**
 * A pasted date in the order of ISO 8601 fills the segments it has; a pasted instant
 * with its offset (`…+05:00`, `…Z`) is that instant, shown in the zone of the field.
 */
function onPaste(event: ClipboardEvent): void {
  event.preventDefault();
  if (!editable.value) return;
  const text = event.clipboardData?.getData("text") ?? "";
  const pasted = pastedEntry(text, props.granularity, zone.value);
  if (kinds.value.every((kind) => pasted[kind] === "")) return;
  for (const kind of kinds.value) entry[kind] = pasted[kind];
  // As typed: the value written if it holds, a refusal only as the field is left.
  changed();
}

function onSegmentFocus(): void {
  fresh = true;
}

function onSegmentBlur(index: number): void {
  const kind = kinds.value[index]!;
  entry[kind] = pad(kind, entry[kind]);
}

/** Left (the focus out of the segments, the button and the grid): the entry is checked. */
function onFocusout(event: FocusEvent): void {
  const next = event.relatedTarget;
  const root = event.currentTarget as HTMLElement;
  if (next instanceof Node && root.contains(next)) return;
  if (next instanceof Element && next.closest(`[data-date-field="${id}"]`)) return;
  if (open.value) return;
  check();
}

/** Its number, within its bounds only (an impossible one is said by its text alone). */
function valueNow(kind: SegmentKind): number | undefined {
  if (entry[kind] === "") return undefined;
  const n = Number(entry[kind]);
  return n >= LIMITS[kind].min && n <= maxOf(kind) ? n : undefined;
}

/** A segment is described by the refusal (read again on each segment), the time by its zone. */
function describedBy(kind: SegmentKind): string | undefined {
  const ids = [
    shownError.value ? `${id}-error` : null,
    kind === "hour" || kind === "minute" ? zoneId : null,
  ];
  return ids.filter(Boolean).join(" ") || undefined;
}

const valueText = (kind: SegmentKind) => {
  const text = entry[kind];
  if (text === "") return t("dateEmpty");
  const n = Number(text);
  return kind === "month" && n >= 1 && n <= 12 ? `${text}, ${monthName(n)}` : text;
};

/** The offset of the zone at the instant typed (or now): `UTC+01:00`. */
const offset = computed(() => {
  const result = checkEntry(entry, props.granularity, zone.value);
  const at =
    result.ok && result.value
      ? Date.parse(result.value)
      : (instantsOf(now(), zone.value)[0] ?? Date.now());
  return offsetLabel(at, zone.value);
});

// The month grid, in a modal dialog under the button.
const open = ref(props.defaultOpen && editable.value);
const grid = ref<InstanceType<typeof DateCalendar> | null>(null);

const today = computed(() => {
  const wall = now();
  return { year: wall.year, month: wall.month, day: wall.day };
});
const selected = computed(() => {
  const [year, month, day] = [Number(entry.year), Number(entry.month), Number(entry.day)];
  return entry.year && entry.month && entry.day && month >= 1 && month <= 12 && year >= 1
    ? day >= 1 && day <= daysIn(year, month)
      ? { year, month, day }
      : null
    : null;
});

function onOpenFocus(event: Event): void {
  event.preventDefault();
  void nextTick(() => grid.value?.focusDay());
}

function onSelect(day: { year: number; month: number; day: number }): void {
  entry.year = pad("year", String(day.year));
  entry.month = pad("month", String(day.month));
  entry.day = pad("day", String(day.day));
  // A day chosen without a time is read from midnight.
  if (props.granularity === "minute") {
    if (entry.hour === "") entry.hour = "00";
    if (entry.minute === "") entry.minute = "00";
  }
  changed();
  open.value = false;
  check();
}

onMounted(() => {
  if (props.defaultEntry !== undefined) {
    changed();
    check();
  }
});

defineExpose({ check, focus });
</script>

<template>
  <FormField
    :id="id"
    group
    class="k-date-field"
    :label="props.label"
    :hide-label="props.hideLabel"
    :hint="props.hint"
    :error="shownError"
    :required="props.required"
    @focusout="onFocusout"
  >
    <div
      class="k-field-frame k-date-field__frame"
      :data-size="props.size"
      :data-invalid="shownError ? 'true' : undefined"
      :data-readonly="props.readonly ? 'true' : undefined"
      :data-disabled="props.disabled ? 'true' : undefined"
    >
      <!-- The order of ISO 8601 reads from left to right in every language. -->
      <span class="k-date-field__segments k-mono" dir="ltr">
        <template v-for="(kind, i) in kinds" :key="kind">
          <span
            v-if="kind === 'month' || kind === 'day'"
            class="k-date-field__sep"
            aria-hidden="true"
            >-</span
          >
          <span v-else-if="kind === 'hour'" class="k-date-field__sep" aria-hidden="true"
            >&nbsp;</span
          >
          <span v-else-if="kind === 'minute'" class="k-date-field__sep" aria-hidden="true">:</span>
          <span
            :ref="(el) => (segments[i] = el as HTMLElement)"
            class="k-date-field__segment"
            role="spinbutton"
            :tabindex="props.disabled ? -1 : 0"
            :contenteditable="editable ? 'true' : undefined"
            inputmode="numeric"
            spellcheck="false"
            :aria-label="t(LABEL[kind])"
            :aria-valuemin="LIMITS[kind].min"
            :aria-valuemax="maxOf(kind)"
            :aria-valuenow="valueNow(kind)"
            :aria-valuetext="valueText(kind)"
            :aria-invalid="shownError ? 'true' : undefined"
            :aria-readonly="props.readonly ? 'true' : undefined"
            :aria-disabled="props.disabled ? 'true' : undefined"
            :aria-describedby="describedBy(kind)"
            :data-empty="entry[kind] === '' ? 'true' : undefined"
            :data-fault="own.segment === kind ? 'true' : undefined"
            @keydown="onKeydown($event, i)"
            @beforeinput="onBeforeInput($event as InputEvent, i)"
            @paste="onPaste"
            @focus="onSegmentFocus"
            @blur="onSegmentBlur(i)"
            ><span aria-hidden="true">{{ entry[kind] || t(HOLE[kind]) }}</span></span
          >
        </template>
      </span>
      <!-- A long name is cut, never its offset; the whole describes the time segments. -->
      <span v-if="props.granularity === 'minute'" :id="zoneId" class="k-date-field__zone">
        <span class="k-date-field__zone-name">{{ zone }}</span>
        {{ " " }}<span class="k-date-field__offset">{{ offset }}</span>
      </span>
      <PopoverRoot v-if="!props.readonly" v-model:open="open" modal>
        <PopoverTrigger as-child>
          <IconButton
            class="k-date-field__open"
            icon="calendar"
            size="sm"
            :label="`${t('dateChoose')}${t('colon')} ${props.label}`"
            :disabled="props.disabled"
            aria-haspopup="dialog"
          />
        </PopoverTrigger>
        <PopoverPortal :to="portal">
          <PopoverContent
            class="k-date-field__popover"
            :data-date-field="id"
            side="bottom"
            align="end"
            :side-offset="4"
            :collision-padding="8"
            aria-modal="true"
            @open-auto-focus="onOpenFocus"
          >
            <DateCalendar
              ref="grid"
              :heading-id="headingId"
              :selected="selected"
              :today="today"
              @select="onSelect"
              @cancel="open = false"
            />
          </PopoverContent>
        </PopoverPortal>
      </PopoverRoot>
    </div>
  </FormField>
</template>

<style>
@layer components {
  .k-date-field__frame {
    gap: var(--k-space-2);
  }

  /* The button sits at the end of the frame, close to its edge (any size). */
  .k-date-field__frame:has(.k-date-field__open) {
    padding-inline-end: var(--k-space-0-5);
  }

  .k-date-field__frame:has(.k-date-field__segment:focus-visible) {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: var(--k-focus-offset);
  }

  .k-date-field__segments {
    display: inline-flex;
    align-items: center;
    font-size: var(--k-text-mono);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  /* The segment typed in is filled with the accent: it is the focus (the frame has the
   * ring). The outline stays transparent, drawn by forced colours where fills are not. */
  .k-date-field__segment {
    padding-inline: var(--k-space-0-5);
    caret-color: transparent;
    border-radius: var(--k-radius-sm);
    outline: var(--k-focus-width) solid transparent;
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  .k-date-field__segment[data-empty] {
    color: var(--k-text-muted);
  }

  .k-date-field__segment:focus {
    color: var(--k-accent-fg);
    background-color: var(--k-accent);
  }

  /* Read only: the focus is shown, softer, nothing will be typed. */
  .k-date-field__segment[aria-readonly="true"]:focus {
    color: var(--k-text);
    background-color: var(--k-accent-soft);
  }

  /* The segment at fault: underlined (a shape, beside the colour of the frame). */
  .k-date-field__segment[data-fault] {
    text-decoration: underline wavy var(--k-state-ko-fg);
    text-underline-offset: var(--k-space-1);
  }

  .k-date-field__sep {
    color: var(--k-text-muted);
  }

  /* Flex items: the space between name and offset is the gap (the text one stays for the
   * description). */
  .k-date-field__zone {
    display: flex;
    gap: var(--k-space-1);
    min-inline-size: 0;
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
    white-space: nowrap;
  }

  .k-date-field__zone-name {
    min-inline-size: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .k-date-field__offset {
    flex: none;
    font-variant-numeric: tabular-nums;
  }

  /* The button at the end of the frame, however wide the field, at the height of the
   * frame whatever its size (no taller than the control); disabled, no box in the box. */
  .k-button.k-date-field__open[data-size] {
    flex: none;
    align-self: stretch;
    min-block-size: 0;
    margin-inline-start: auto;
  }

  .k-button.k-date-field__open:disabled {
    background-color: transparent;
    border-color: transparent;
  }

  /* A modal dialog (Date Picker Dialog): the look of a layer laid on the page, as the one
   * of the date pill it may open from (task 23): the shadow of the upper level, a
   * contrasted border, a background of its own; over that layer, the later one above. */
  .k-date-field__popover {
    z-index: var(--k-z-modal);
    padding: var(--k-space-3);
    color: var(--k-text);
    background-color: var(--k-surface-raised);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-lg);
    box-shadow: var(--k-shadow-modal);
    transition-property: opacity, translate;
    transition-duration: max(var(--k-duration-short), var(--k-duration-fade));
    transition-timing-function: var(--k-ease-standard);
  }

  @starting-style {
    .k-date-field__popover {
      opacity: 0;
      translate: 0 calc(var(--k-motion-shift-sm) * -1);
    }
  }

  .k-date-field__popover[data-state="closed"] {
    pointer-events: none;
    animation-name: k-fade-out;
    animation-duration: var(--k-duration-fade);
    animation-timing-function: var(--k-ease-exit);
    animation-fill-mode: forwards;
  }
}
</style>
