<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The month grid of the date picker dialog (APG Date Picker Dialog), inside the dialog of
// `DateTimeField`: the month and year as its heading, said when they change (a polite
// region); buttons for the previous and next year and month; the days in a `grid`, weeks
// from Monday (ISO 8601), one day in the tab order (roving). Arrows move by a day and a
// week, Home and End to the start and end of the week, Page Up and Page Down by a month,
// with Shift by a year; Enter and Space choose the day. Today has `aria-current="date"`,
// the chosen day `aria-selected`. Escape is the dialog's.
import { computed, nextTick, ref } from "vue";
import { currentLocale, t } from "../../i18n.ts";
import Button from "../Button/Button.vue";
import IconButton from "../IconButton/IconButton.vue";
import { daysIn, LIMITS, shiftDate, utcOf, weekday } from "./dates.ts";

interface Day {
  year: number;
  month: number;
  day: number;
}

const props = defineProps<{
  /** The day chosen in the field, if any. */
  selected: Day | null;
  today: Day;
  /** The id of the heading, which names the dialog too. */
  headingId: string;
}>();
const emit = defineEmits<{ select: [day: Day]; cancel: [] }>();

const grid = ref<HTMLTableElement | null>(null);

const clamp = (day: Day): Day =>
  day.year < LIMITS.year.min
    ? { year: LIMITS.year.min, month: 1, day: 1 }
    : day.year > LIMITS.year.max
      ? { year: LIMITS.year.max, month: 12, day: 31 }
      : day;

/** The day that has the focus of the grid; its month is the month shown. */
const focused = ref<Day>(clamp(props.selected ?? props.today));

const iso = (d: Day) =>
  `${String(d.year).padStart(4, "0")}-${String(d.month).padStart(2, "0")}-${String(d.day).padStart(2, "0")}`;
const same = (a: Day | null, b: Day) =>
  !!a && a.year === b.year && a.month === b.month && a.day === b.day;

const heading = computed(() =>
  new Intl.DateTimeFormat(currentLocale(), {
    month: "long",
    year: "numeric",
    timeZone: "UTC",
  }).format(utcOf({ ...focused.value, day: 1, hour: 0, minute: 0 })),
);

/** The names of the days, Monday first: short in the header, long as its `abbr`. */
const weekdays = computed(() =>
  Array.from({ length: 7 }, (_, i) => {
    // 2024-01-01 was a Monday.
    const at = utcOf({ year: 2024, month: 1, day: 1 + i, hour: 0, minute: 0 });
    const name = (weekday: "short" | "long") =>
      new Intl.DateTimeFormat(currentLocale(), { weekday, timeZone: "UTC" }).format(at);
    return { short: name("short"), long: name("long") };
  }),
);

/** The weeks of the month shown, empty cells before the first day and after the last. */
const weeks = computed(() => {
  const { year, month } = focused.value;
  const cells: (Day | null)[] = Array<null>(weekday(year, month, 1)).fill(null);
  for (let day = 1; day <= daysIn(year, month); day++) cells.push({ year, month, day });
  while (cells.length % 7) cells.push(null);
  return Array.from({ length: cells.length / 7 }, (_, i) => cells.slice(i * 7, i * 7 + 7));
});

function move(to: Day, focus = true): void {
  focused.value = clamp(to);
  if (focus) void nextTick(focusDay);
}

/** Puts the focus on the focused day of the grid (when the dialog opens, after a key). */
function focusDay(): void {
  grid.value?.querySelector<HTMLElement>(`[data-date="${iso(focused.value)}"]`)?.focus();
}

function onKeydown(event: KeyboardEvent): void {
  const day = focused.value;
  const page = event.shiftKey ? { years: 1 } : { months: 1 };
  const back = event.shiftKey ? { years: -1 } : { months: -1 };
  const actions: Record<string, () => Day> = {
    ArrowRight: () => shiftDate(day, { days: 1 }),
    ArrowLeft: () => shiftDate(day, { days: -1 }),
    ArrowDown: () => shiftDate(day, { days: 7 }),
    ArrowUp: () => shiftDate(day, { days: -7 }),
    Home: () => shiftDate(day, { days: -weekday(day.year, day.month, day.day) }),
    End: () => shiftDate(day, { days: 6 - weekday(day.year, day.month, day.day) }),
    PageDown: () => shiftDate(day, page),
    PageUp: () => shiftDate(day, back),
  };
  const action = actions[event.key];
  if (action) {
    event.preventDefault();
    move(action());
  } else if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    emit("select", day);
  }
}

defineExpose({ focusDay });
</script>

<template>
  <div class="k-date-calendar">
    <div class="k-date-calendar__head">
      <IconButton
        icon="chevrons-left"
        size="sm"
        :label="t('datePreviousYear')"
        @click="move(shiftDate(focused, { years: -1 }), false)"
      />
      <IconButton
        icon="chevron-left"
        size="sm"
        :label="t('datePreviousMonth')"
        @click="move(shiftDate(focused, { months: -1 }), false)"
      />
      <h2 :id="props.headingId" class="k-date-calendar__title" aria-live="polite">
        {{ heading }}
      </h2>
      <IconButton
        icon="chevron-right"
        size="sm"
        :label="t('dateNextMonth')"
        @click="move(shiftDate(focused, { months: 1 }), false)"
      />
      <IconButton
        icon="chevrons-right"
        size="sm"
        :label="t('dateNextYear')"
        @click="move(shiftDate(focused, { years: 1 }), false)"
      />
    </div>
    <table ref="grid" class="k-date-calendar__grid" role="grid" :aria-labelledby="props.headingId">
      <thead>
        <tr>
          <th v-for="name in weekdays" :key="name.long" scope="col" :abbr="name.long">
            {{ name.short }}
          </th>
        </tr>
      </thead>
      <tbody @keydown="onKeydown">
        <tr v-for="(week, w) in weeks" :key="w">
          <template v-for="(cell, i) in week" :key="cell ? cell.day : `e${i}`">
            <td
              v-if="cell"
              class="k-date-calendar__day"
              :data-date="iso(cell)"
              :tabindex="same(focused, cell) ? 0 : -1"
              :aria-selected="same(props.selected, cell) ? 'true' : undefined"
              :aria-current="same(props.today, cell) ? 'date' : undefined"
              @click="emit('select', cell)"
            >
              {{ cell.day }}
            </td>
            <td v-else class="k-date-calendar__empty"></td>
          </template>
        </tr>
      </tbody>
    </table>
    <div class="k-date-calendar__actions">
      <Button size="sm" @click="emit('cancel')">{{ t("cancel") }}</Button>
    </div>
  </div>
</template>

<style>
@layer components {
  .k-date-calendar {
    display: grid;
    gap: var(--k-space-2);
  }

  .k-date-calendar__head {
    display: flex;
    gap: var(--k-space-0-5);
    align-items: center;
  }

  .k-date-calendar__title {
    flex: 1;
    margin: 0;
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
    font-weight: var(--k-weight-medium);
    text-align: center;
  }

  /* Separate borders: the reset collapses those of tables, and the ring of a day would
   * share its sides with its neighbours. */
  .k-date-calendar__grid {
    border-spacing: 0;
    border-collapse: separate;
    font-variant-numeric: tabular-nums;
  }

  .k-date-calendar__grid th {
    block-size: var(--k-control-height-sm);
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
    font-weight: var(--k-weight-medium);
    color: var(--k-text-muted);
  }

  .k-date-calendar__day,
  .k-date-calendar__empty {
    inline-size: var(--k-control-height-md);
    block-size: var(--k-control-height-md);
    padding: 0;
    text-align: center;
  }

  .k-date-calendar__day {
    cursor: pointer;
    border: var(--k-border-width) solid transparent;
    border-radius: var(--k-radius-md);
  }

  .k-date-calendar__day:hover {
    background-color: var(--k-hover);
  }

  /* The focus ring outside the cell: inside, it would vanish on the accent of the day
   * chosen (2.4.7). */
  .k-date-calendar__day:focus-visible {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: var(--k-focus-offset);
  }

  /* Today: a ring (a shape, not a colour alone); the day chosen: filled with the accent,
   * and an outline transparent until forced colours draw it (the fill is dropped there). */
  .k-date-calendar__day[aria-current="date"] {
    font-weight: var(--k-weight-semibold);
    border-color: var(--k-accent);
  }

  .k-date-calendar__day[aria-selected="true"] {
    color: var(--k-accent-fg);
    background-color: var(--k-accent);
    outline: var(--k-focus-width) solid transparent;
    outline-offset: calc(var(--k-focus-width) * -1);
  }

  /* Chosen and today: the ring drawn in the colour of the text, inside the fill. */
  .k-date-calendar__day[aria-selected="true"][aria-current="date"] {
    border-color: var(--k-accent-fg);
  }

  .k-date-calendar__day[aria-selected="true"]:focus-visible {
    outline-color: var(--k-focus-ring);
    outline-offset: var(--k-focus-offset);
  }

  .k-date-calendar__actions {
    display: flex;
    justify-content: end;
  }
}
</style>
