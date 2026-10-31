<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The typed filters of a list (design « Inventaire », line 6; REQ-020-20): its chips
// (`FilterChip`) and « Ajouter un filtre », which opens the picker in a popover (Reka's,
// non modal: Escape or a click outside closes it, the focus goes back to what opened it).
// The picker goes in sequence (APG Button then Combobox): the property among the closed
// list of the model, with autocompletion (its list open as the picker opens, the key shown
// beside the label); the operators its type and the route serve; the value by a closed
// list with autocompletion when the domain is closed, otherwise a typed field (number,
// prefix, address, date) checked before the filter is applied: a value that does not fit
// is refused, its reason linked to the field and said. Two Escapes close the list, then the
// popover. A filter begun and left (Escape, a click outside) stays a draft chip, never in
// the URL; « Annuler » drops it. A chip opens the same picker to edit its filter, its
// button removes it; what is applied or removed is said by a polite region. Two filters on
// one parameter cannot both stand: the one applied replaces the other. The filters are
// the host's (`v-model`); their place in the URL is `filtersToParams` and
// `filtersFromParams` (filters.ts).
import { PopoverAnchor, PopoverContent, PopoverPortal, PopoverRoot } from "reka-ui";
import { computed, nextTick, onMounted, reactive, ref, watch, type Ref } from "vue";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import Button from "../Button/Button.vue";
import Combobox from "../Combobox/Combobox.vue";
import { filterOptions, type Suggest } from "../Combobox/combobox.ts";
import FilterChip from "../FilterChip/FilterChip.vue";
import IconButton from "../IconButton/IconButton.vue";
import { setRoom } from "../internal/room.ts";
import Select from "../Select/Select.vue";
import type { ListOption } from "../Select/select.ts";
import TextField from "../TextField/TextField.vue";
import {
  arity,
  checkFilter,
  checkValue,
  filterParam,
  filterQuery,
  filterSentence,
  filterWords,
  monoValues,
  operatorLabel,
  propertyOf,
  valueLabel,
  valueType,
  type Filter,
  type FilterModel,
  type FilterOperator,
  type FilterProperty,
} from "./filters.ts";

const props = withDefaults(
  defineProps<{
    /** The properties of the list, their types, operators and domains. */
    model: FilterModel;
    /** The name of the group of filters. */
    label?: string | undefined;
    /** The shortcut of « Ajouter un filtre » (`/`, the filter of the view), shown on it. */
    shortcut?: string[] | undefined;
    /** Open at first render: `true` to add a filter, the index of one to edit it (the catalogue). */
    defaultOpen?: boolean | number;
  }>(),
  { label: undefined, shortcut: undefined, defaultOpen: false },
);
/** The filters, applied, invalid (read from the URL) or drafts. */
const filters = defineModel<Filter[]>({ default: () => [] });

const portal = injectPortalTarget();
const root = ref<HTMLElement | null>(null);
const form = ref<HTMLFormElement | null>(null);

/** What the picker writes: a new filter, a draft chip (live), or an applied one (on apply). */
type Target = { kind: "new" } | { kind: "draft" | "edit"; index: number };
const open = ref(false);
const target = ref<Target>({ kind: "new" });
const anchor = ref<HTMLElement | undefined>();
/** What opened it: a chip (its index), or « Ajouter un filtre » (-1). */
const opener = ref(-1);
/** One per opening: the fields start afresh (and the property list opens to add). */
const session = ref(0);
/** A click outside closed it: the focus stays where it went. */
let outside = false;
/** The focus was given back for this close. */
let restored = false;

const draft = reactive({
  propertyText: "",
  key: undefined as string | undefined,
  operator: undefined as FilterOperator | undefined,
  /** The values chosen so far for `eq` (one of). */
  tokens: [] as string[],
  text: "",
  picked: undefined as string | undefined,
  low: "",
  high: "",
});
const errors = reactive({
  property: undefined as string | undefined,
  operator: undefined as string | undefined,
  value: undefined as string | undefined,
  low: undefined as string | undefined,
  high: undefined as string | undefined,
});
const said = ref("");
/** A value of « l'un de » added or removed, said inside the popover (Reka hides the page). */
const valueSaid = ref("");

const property = computed(() => (draft.key ? propertyOf(props.model, draft.key) : undefined));
const count = computed(() => (draft.operator ? arity(draft.operator) : 1));
/** The type of the value the operator takes (`within`: a prefix). */
const type = computed(() =>
  property.value ? valueType(property.value, draft.operator) : undefined,
);
const closed = computed(() => type.value === "enum");
const editedIndex = computed(() => (target.value.kind === "new" ? -1 : target.value.index));

/** The properties, label then key in fixed pitch (REQ-020-21: the key is what one types). */
const propertyOptions = computed<ListOption[]>(() =>
  props.model.map((p) =>
    p.label
      ? { value: p.key, label: p.label, detail: p.key }
      : { value: p.key, label: p.key, mono: true },
  ),
);
const suggestProperty: Suggest = async (query) => filterOptions(propertyOptions.value, query);

const operatorOptions = computed<ListOption[]>(() =>
  (property.value?.operators ?? []).map((op) => ({ value: op, label: operatorLabel(op) })),
);

/** A value of a closed domain: its label, its key beside it when they differ. */
function withKey(option: ListOption): ListOption {
  if (option.detail || option.label === option.value) return option;
  return { ...option, detail: option.value };
}
const suggestValue: Suggest = async (query, signal) => {
  const p = property.value;
  if (!p) return [];
  // The key beside the label, and searched too, whether the host or the model holds them.
  const found = p.suggest
    ? (await p.suggest(query, signal)).map(withKey)
    : filterOptions((p.options ?? []).map(withKey), query);
  return found.filter((option) => !draft.tokens.includes(option.value));
};

const HINT: Partial<Record<FilterProperty["type"], Parameters<typeof t>[0]>> = {
  number: "filterHintNumber",
  prefix: "filterHintPrefix",
  address: "filterHintAddress",
  date: "filterHintDate",
};
const valueHint = computed(() => {
  const key = type.value ? HINT[type.value] : undefined;
  return key ? t(key) : undefined;
});

const labelOf = (value: string) => valueLabel(property.value, value) ?? value;

/** The values the picker holds now, for the operator chosen. */
function currentValues(): string[] {
  const field = closed.value ? (draft.picked ?? "") : draft.text.trim();
  if (count.value === 0) return [];
  if (count.value === 2) return [draft.low.trim(), draft.high.trim()];
  if (count.value === 1) return field ? [field] : [];
  return field && !draft.tokens.includes(field) ? [...draft.tokens, field] : [...draft.tokens];
}

function currentFilter(): Filter | undefined {
  if (!draft.key || !draft.operator) return undefined;
  return { key: draft.key, operator: draft.operator, values: currentValues() };
}

/**
 * The property the operator and the values were chosen for: the field's own value is set
 * (`v-model:value`) before `select` comes, so it cannot tell a new choice from the same one.
 */
let chosen: string | undefined;

function reset(): void {
  chosen = undefined;
  Object.assign(draft, {
    propertyText: "",
    key: undefined,
    operator: undefined,
    tokens: [],
    text: "",
    picked: undefined,
    low: "",
    high: "",
  });
  clearErrors();
}

function clearErrors(): void {
  Object.assign(errors, {
    property: undefined,
    operator: undefined,
    value: undefined,
    low: undefined,
    high: undefined,
  });
}

/** The fields of the picker from a filter (an edit, a draft taken up again). */
function load(filter: Filter): void {
  const p = propertyOf(props.model, filter.key);
  reset();
  chosen = filter.key;
  draft.key = filter.key;
  draft.propertyText = p?.label ?? filter.key;
  draft.operator = filter.operator;
  const [first = "", second = ""] = filter.values;
  const many = arity(filter.operator) === "many";
  const enumerated = !!p && valueType(p, filter.operator) === "enum";
  if (arity(filter.operator) === 2) {
    draft.low = first;
    draft.high = second;
  } else if (enumerated && many) draft.tokens = [...filter.values];
  else if (many && filter.values.length > 1) draft.tokens = [...filter.values];
  else if (enumerated) {
    draft.picked = first || undefined;
    draft.text = first ? (valueLabel(p, first) ?? first) : "";
  } else draft.text = first;
  // An invalid filter (read from the URL) shows why at once, on its field.
  if (!filter.draft) {
    const reason = checkFilter(props.model, filter);
    if (reason && !p) errors.property = reason;
    else if (reason && arity(filter.operator) === 0) errors.operator = reason;
    else if (reason && arity(filter.operator) === 2) errors.low = reason;
    else if (reason) errors.value = reason;
  }
}

const chipOf = (index: number) =>
  root.value?.querySelectorAll<HTMLElement>(".k-filter-chip__body")[index];
const addButton = () => root.value?.querySelector<HTMLElement>(".k-filter-picker__add");

/**
 * Opens the picker from an opener; the same opener a second time closes it (a disclosure);
 * another one while it is open takes it over, its fields afresh and the focus moved.
 */
function openFrom(index: number, prepare: () => void): void {
  if (open.value && opener.value === index) {
    outside = false;
    open.value = false;
    return;
  }
  const again = open.value;
  prepare();
  opener.value = index;
  anchor.value = (index < 0 ? addButton() : chipOf(index)) ?? undefined;
  session.value += 1;
  restored = false;
  open.value = true;
  // Reka gives no open focus to a popover already open: the same step, by hand.
  if (again) void nextTick(() => firstStep()?.focus());
}

function openAdd(): void {
  openFrom(-1, () => {
    reset();
    target.value = { kind: "new" };
  });
}

function openEdit(index: number): void {
  const filter = filters.value[index];
  if (!filter) return;
  openFrom(index, () => {
    load(filter);
    target.value = { kind: filter.draft ? "draft" : "edit", index };
  });
}

/**
 * Reka's focus scopes say their auto focus by an event that bubbles: the lists of the
 * operator and of the value, inside the popover, are theirs to handle.
 */
const own = (event: Event) =>
  event.target instanceof Element &&
  (event.target.matches(".k-filter-picker__popover") ||
    event.target.querySelector(":scope > .k-filter-picker__popover") !== null);

function onOpenFocus(event: Event): void {
  if (!own(event)) return;
  event.preventDefault();
  void nextTick(() => firstStep()?.focus());
}

/** Where the focus goes as the picker opens: the property to add (or unknown), the value to edit. */
function firstStep(): HTMLElement | null | undefined {
  return target.value.kind === "new" || !property.value
    ? form.value?.querySelector<HTMLElement>("[data-filter-step='property'] input")
    : focusTarget();
}

/** The first field of the value, else the operator. */
function focusTarget(): HTMLElement | null | undefined {
  return (
    form.value?.querySelector<HTMLElement>("[data-filter-step='value'] input") ??
    form.value?.querySelector<HTMLElement>(".k-select__trigger")
  );
}

// Within the window at any zoom (WCAG 1.4.10): the room around its anchor, by CSSOM; the
// popover scrolls inside it.
watch(open, async (now) => {
  if (!now) return;
  await nextTick();
  const surface = form.value?.closest<HTMLElement>(".k-filter-picker__popover");
  const rect = anchor.value?.getBoundingClientRect();
  if (surface && rect) setRoom(surface, rect, "filter-picker");
});

/** Closed by Escape or by « Appliquer » : back to what opened it; by a click outside, not. */
function onCloseFocus(event: Event): void {
  if (!own(event)) return;
  event.preventDefault();
  // Reka says it twice (its wrapper, then its scope): once is enough.
  if (restored) return;
  restored = true;
  if (outside) {
    outside = false;
    return;
  }
  focusAfterClose();
}

let focusNext: (() => HTMLElement | null | undefined) | undefined;
function focusAfterClose(): void {
  const next = focusNext?.() ?? anchor.value ?? addButton();
  focusNext = undefined;
  next?.focus();
}

/** A click on the bar itself (a chip, « Ajouter ») goes to it, not to closing. */
function onOutside(event: Event): void {
  const node = (event as CustomEvent<{ originalEvent: Event }>).detail?.originalEvent?.target;
  if (node instanceof Node && root.value?.contains(node)) {
    event.preventDefault();
    return;
  }
  outside = true;
}

function onOpenChange(next: boolean): void {
  open.value = next;
}

/** The property is chosen: its first operator; on to the next choice (operator or value). */
function onProperty(option: ListOption): void {
  const p = propertyOf(props.model, option.value);
  if (!p) return;
  draft.key = p.key;
  if (chosen !== p.key || !draft.operator) {
    chosen = p.key;
    draft.operator = p.operators[0];
    Object.assign(draft, { tokens: [], text: "", picked: undefined, low: "", high: "" });
  }
  clearErrors();
  if (target.value.kind === "new") {
    // The model is the host's: it reads back the new list only once the host renders.
    const next = [...filters.value, { ...currentFilter()!, draft: true }];
    filters.value = next;
    target.value = { kind: "draft", index: next.length - 1 };
  }
  void nextTick(() => {
    const next =
      p.operators.length > 1
        ? form.value?.querySelector<HTMLElement>(".k-select__trigger")
        : focusTarget();
    next?.focus();
  });
}

function onOperator(next: string | undefined): void {
  const op = next as FilterOperator | undefined;
  const p = property.value;
  if (op && draft.operator && p) {
    if (arity(op) !== "many") draft.tokens = [];
    // Another type of value (`within` takes a prefix): the field starts empty.
    if (valueType(p, op) !== valueType(p, draft.operator))
      Object.assign(draft, { tokens: [], text: "", picked: undefined });
  }
  errors.value = errors.low = errors.high = undefined;
  draft.operator = op;
}

/** A value of the closed domain is taken: for `eq`, one more among the chosen. */
function onValue(option: ListOption): void {
  errors.value = undefined;
  if (count.value !== "many") return;
  if (!draft.tokens.includes(option.value)) draft.tokens = [...draft.tokens, option.value];
  draft.text = "";
  draft.picked = undefined;
  say(valueSaid, `${t("filterValueAdded")} ${option.label}`);
}

/** Says a sentence in a polite region, again even when it is the same as before. */
function say(region: Ref<string>, sentence: string): void {
  region.value = "";
  void nextTick(() => (region.value = sentence));
}

function removeToken(index: number): void {
  const gone = draft.tokens[index];
  draft.tokens = draft.tokens.filter((_, i) => i !== index);
  if (gone !== undefined) say(valueSaid, `${t("filterValueRemoved")} ${labelOf(gone)}`);
  void nextTick(() => {
    const buttons = form.value?.querySelectorAll<HTMLElement>(".k-filter-picker__value button");
    (buttons?.[Math.min(index, buttons.length - 1)] ?? focusTarget())?.focus();
  });
}

// The draft chip follows what the picker holds.
watch(
  () => [draft.key, draft.operator, draft.tokens, draft.text, draft.picked, draft.low, draft.high],
  () => {
    if (target.value.kind !== "draft") return;
    const now = currentFilter();
    const index = target.value.index;
    const before = filters.value[index];
    if (!now || !before || JSON.stringify(now) === JSON.stringify({ ...before, draft: undefined }))
      return;
    // The same chip (its key kept): the popover stays on it.
    const written: Filter = { ...now, draft: true };
    ids.set(written, idOf(before));
    filters.value = filters.value.map((f, i) => (i === index ? written : f));
  },
  // After the host rendered: the list read is the one it holds.
  { flush: "post" },
);

/** Checks the filter written and applies it, or says why not on its field and goes there. */
function apply(): void {
  clearErrors();
  const p = property.value;
  if (!p || !draft.operator) {
    errors.property = t("filterPropertyPick");
    void nextTick(() =>
      form.value?.querySelector<HTMLElement>("[data-filter-step='property'] input")?.focus(),
    );
    return;
  }
  const filter = currentFilter()!;
  if (closed.value && count.value !== 2 && draft.text.trim() !== "" && !draft.picked)
    errors.value = t("filterValuePick");
  else if (count.value === 2) {
    errors.low = draft.low.trim() ? checkValue(p, draft.low, draft.operator) : undefined;
    errors.high = draft.high.trim() ? checkValue(p, draft.high, draft.operator) : undefined;
    if (!errors.low && !errors.high) errors.low = checkFilter(props.model, filter);
  } else if (count.value === 0) errors.operator = checkFilter(props.model, filter);
  else errors.value = checkFilter(props.model, filter);
  if (errors.operator || errors.value || errors.low || errors.high) {
    void nextTick(() => {
      const fields = form.value?.querySelectorAll<HTMLElement>("[data-filter-step='value'] input");
      const at = errors.high && !errors.low ? 1 : 0;
      (fields?.[at] ?? focusTarget())?.focus();
    });
    return;
  }
  const param = filterParam(props.model, filter);
  const index = editedIndex.value;
  const kept = filters.value.map((f, i) => (i === index ? filter : f));
  if (index < 0) kept.push(filter);
  const at = index < 0 ? kept.length - 1 : index;
  // The one applied replaces any other on its parameter (repeated, it would be « one of »).
  const next = kept.filter((f, i) => i === at || f.draft || filterParam(props.model, f) !== param);
  const place = next.indexOf(filter);
  filters.value = next;
  say(said, `${t("filterApplied")} ${filterSentence(props.model, filter)}`);
  focusNext = () => chipOf(place);
  outside = false;
  open.value = false;
}

/** « Annuler »: a draft is dropped, an edit left as it was. */
function cancel(): void {
  if (target.value.kind === "draft") {
    const index = target.value.index;
    filters.value = filters.value.filter((_, i) => i !== index);
    focusNext = addButton;
  }
  outside = false;
  open.value = false;
}

function remove(index: number): void {
  const filter = filters.value[index];
  if (!filter) return;
  if (open.value && (editedIndex.value === index || opener.value === index)) {
    outside = true;
    open.value = false;
  } else if (open.value) {
    if (editedIndex.value > index && target.value.kind !== "new")
      target.value = { ...target.value, index: target.value.index - 1 };
    if (opener.value > index) opener.value -= 1;
  }
  filters.value = filters.value.filter((_, i) => i !== index);
  say(said, `${t("filterRemoved")} ${filterSentence(props.model, filter)}`);
  void nextTick(() => (chipOf(index) ?? chipOf(index - 1) ?? addButton())?.focus());
}

/**
 * A key per filter, kept while the host keeps the object: removing a chip does not patch the
 * next ones in place (their alert would be said again).
 */
const ids = new WeakMap<Filter, number>();
let lastId = 0;
function idOf(filter: Filter): number {
  let id = ids.get(filter);
  if (id === undefined) ids.set(filter, (id = ++lastId));
  return id;
}

/** How each chip reads, and whether it is invalid (read from the URL) and why. */
const chips = computed(() =>
  filters.value.map((filter, index) => {
    const words = filterWords(props.model, filter);
    const reason = filter.draft ? undefined : checkFilter(props.model, filter);
    return {
      filter,
      id: idOf(filter),
      index,
      words,
      state: filter.draft ? "draft" : reason ? "invalid" : "applied",
      reason,
      sentence: filterSentence(props.model, filter),
      query: filter.draft ? undefined : filterQuery(props.model, filter),
    } as const;
  }),
);

const title = computed(() => (target.value.kind === "edit" ? t("filterEdit") : t("filterAdd")));
const valueMono = computed(() => monoValues(property.value, draft.operator));

onMounted(() => {
  if (props.defaultOpen === true) openAdd();
  else if (typeof props.defaultOpen === "number") openEdit(props.defaultOpen);
});
</script>

<template>
  <PopoverRoot :open="open" @update:open="onOpenChange">
    <PopoverAnchor as-child v-bind="anchor ? { reference: anchor } : {}">
      <div
        ref="root"
        class="k-filter-picker"
        role="group"
        :aria-label="props.label ?? t('filters')"
      >
        <!-- Chips and button in one wrapping row (no list around the chips: the button
             would go to a line of its own as soon as they wrap). -->
        <FilterChip
          v-for="chip in chips"
          :key="chip.id"
          :property="chip.words.property"
          :property-mono="chip.words.propertyMono"
          :operator="chip.words.operator"
          :value="chip.words.value"
          :mono="chip.words.valueMono"
          :state="chip.state"
          :reason="chip.reason"
          :remove-label="`${t('removeFilter')} ${chip.sentence}`"
          :expanded="open && opener === chip.index"
          :query="chip.query"
          @edit="openEdit(chip.index)"
          @remove="remove(chip.index)"
        />
        <Button
          class="k-filter-picker__add"
          variant="ghost"
          size="sm"
          icon="funnel"
          :shortcut="props.shortcut"
          aria-haspopup="dialog"
          :aria-expanded="open && opener === -1"
          @click="openAdd"
          >{{ t("filterAdd") }}</Button
        >
        <span class="k-visually-hidden" role="status" aria-live="polite">{{ said }}</span>
      </div>
    </PopoverAnchor>
    <PopoverPortal :to="portal">
      <PopoverContent
        class="k-popover k-filter-picker__popover"
        side="bottom"
        align="start"
        :side-offset="4"
        :collision-padding="8"
        :aria-label="title"
        @open-auto-focus="onOpenFocus"
        @close-auto-focus="onCloseFocus"
        @interact-outside="onOutside"
      >
        <form
          ref="form"
          :key="session"
          class="k-filter-picker__form"
          novalidate
          @submit.prevent="apply"
        >
          <p class="k-filter-picker__title" aria-hidden="true">{{ title }}</p>
          <div data-filter-step="property">
            <Combobox
              v-model="draft.propertyText"
              v-model:value="draft.key"
              :label="t('filterProperty')"
              :suggest="suggestProperty"
              :debounce="0"
              :mono="!!property && !property.label"
              :hint="property?.label ? `${t('filterKey')} ${property.key}` : undefined"
              :error="errors.property"
              :default-open="target.kind === 'new'"
              size="sm"
              @select="onProperty"
            />
          </div>
          <!-- Keyed by the property: Reka's value shows the label of an option mounted
               before it, not of one that comes with it. -->
          <Select
            :key="draft.key ?? ''"
            :model-value="draft.operator"
            :label="t('filterOperator')"
            :options="operatorOptions"
            :disabled="!property"
            :readonly="operatorOptions.length === 1"
            :error="errors.operator"
            size="sm"
            @update:model-value="onOperator"
          />
          <div v-if="property" data-filter-step="value" class="k-filter-picker__step">
            <template v-if="count === 2">
              <TextField
                v-model="draft.low"
                :label="t('filterLow')"
                :error="errors.low"
                :hint="valueHint"
                :mono="valueMono"
                size="sm"
              />
              <TextField
                v-model="draft.high"
                :label="t('filterHigh')"
                :error="errors.high"
                :hint="t('filterBoundHint')"
                :mono="valueMono"
                size="sm"
              />
            </template>
            <p v-else-if="count === 0" class="k-filter-picker__note">{{ t("filterNoValue") }}</p>
            <Combobox
              v-else-if="closed"
              v-model="draft.text"
              v-model:value="draft.picked"
              :label="t('filterValue')"
              :suggest="suggestValue"
              :error="errors.value"
              size="sm"
              @select="onValue"
            />
            <TextField
              v-else
              v-model="draft.text"
              :label="t('filterValue')"
              :error="errors.value"
              :hint="valueHint"
              :mono="valueMono"
              size="sm"
            />
            <ul
              v-if="count === 'many' && draft.tokens.length"
              class="k-filter-picker__values"
              :aria-label="t('filterValues')"
            >
              <li v-for="(token, i) in draft.tokens" :key="token" class="k-filter-picker__value">
                <span :class="{ 'k-mono': valueMono && labelOf(token) === token }">{{
                  labelOf(token)
                }}</span>
                <IconButton
                  icon="x"
                  size="sm"
                  :label="`${t('filterRemoveValue')} ${labelOf(token)}`"
                  @click="removeToken(i)"
                />
              </li>
            </ul>
          </div>
          <span class="k-visually-hidden k-filter-picker__said" role="status" aria-live="polite">{{
            valueSaid
          }}</span>
          <!-- The primary one last, as in `Dialog` and `AlertDialog`. -->
          <div class="k-filter-picker__actions">
            <Button size="sm" @click="cancel">{{ t("cancel") }}</Button>
            <Button type="submit" variant="primary" size="sm">{{ t("filterApply") }}</Button>
          </div>
        </form>
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style>
@layer components {
  .k-filter-picker {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
    min-inline-size: 0;
  }

  /* At the width of a narrow panel; the surface and its motion are the popover's
   * (`k-popover`, Popover/surface.css). */
  .k-filter-picker__popover {
    inline-size: var(--k-size-panel-md);
    max-inline-size: var(--k-filter-picker-room-inline, var(--k-size-panel-md));
    max-block-size: var(--k-filter-picker-room-block, var(--k-size-dialog-md));
    padding: var(--k-space-3);
    overflow: auto;
  }

  .k-filter-picker__form,
  .k-filter-picker__step {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-3);
  }

  .k-filter-picker__title {
    margin: 0;
    font-weight: var(--k-weight-medium);
  }

  .k-filter-picker__note {
    margin: 0;
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    color: var(--k-text-muted);
  }

  .k-filter-picker__values {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-1);
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .k-filter-picker__value {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    padding-inline-start: var(--k-space-2);
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    /* Neutral: a value chosen is not an applied filter (the chip's accent). */
    background-color: var(--k-surface-sunken);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-md);
  }

  .k-filter-picker__actions {
    display: flex;
    gap: var(--k-space-2);
    justify-content: end;
  }
}
</style>
