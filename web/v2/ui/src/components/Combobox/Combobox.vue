<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A field with suggestions (design « Inventaire », line 5; APG Combobox, list autocomplete
// with manual selection), on Reka's Combobox: the text is the host's, the suggestions come
// from its function (`suggest`), called once the typing pauses (150 ms) and aborted when a
// newer text replaces it. The focus stays in the field; the highlighted suggestion is its
// `aria-activedescendant`. Keys: typing shows the suggestions, none highlighted; Down
// opens and goes to the first one (once they are there), Up and Down move; Alt+Down opens
// without moving; Enter takes the highlighted one (`select`), or else is the host's
// (`submit`, a search); Escape closes, and a second Escape is the layer's around it (a
// popover, a dialog); Home and End stay in the text. What the list holds (count, loading,
// nothing, failure) is said by a polite region present before it, and shown in the list
// when it has no option. In a `FormField`: label, error then help by `aria-describedby`.
// No ComboboxViewport: it injects a <style> (REQ-020-33). The list has the look of the one
// of `Select` (Select/options.css).
import {
  ComboboxAnchor,
  ComboboxContent,
  ComboboxInput,
  ComboboxItem,
  ComboboxPortal,
  ComboboxRoot,
} from "reka-ui";
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  useId,
  watch,
  type ComponentPublicInstance,
} from "vue";
import { injectPortalTarget } from "../../context.ts";
import type { IconName } from "../../icons/index.ts";
import { t } from "../../i18n.ts";
import FormField from "../FormField/FormField.vue";
import Icon from "../Icon/Icon.vue";
import type { ControlSize } from "../layout.ts";
import type { ListOption } from "../Select/select.ts";
import type { Suggest } from "./combobox.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    suggest: Suggest;
    /** The pause of the typing before `suggest` is called, in milliseconds. */
    debounce?: number;
    hideLabel?: boolean;
    hint?: string | undefined;
    /** The error: sets `aria-invalid` and is read with the field. */
    error?: string | undefined;
    placeholder?: string | undefined;
    icon?: IconName | undefined;
    mono?: boolean;
    size?: ControlSize;
    required?: boolean;
    disabled?: boolean;
    /** Asks for the suggestions of its text at first render, list open (the catalogue). */
    defaultOpen?: boolean;
  }>(),
  {
    debounce: 150,
    hint: undefined,
    error: undefined,
    placeholder: undefined,
    icon: undefined,
    size: "md",
  },
);
/** The text of the field. */
const query = defineModel<string>({ default: "" });
/** The value of the suggestion taken; cleared as soon as the text changes. */
const value = defineModel<string | undefined>("value", { default: undefined });
const emit = defineEmits<{
  select: [option: ListOption];
  /** Enter without a highlighted suggestion: the text as it is (a search). */
  submit: [text: string];
}>();

const portal = injectPortalTarget();
const id = useId();
const root = ref<ComponentPublicInstance | null>(null);
const anchor = ref<ComponentPublicInstance | null>(null);

const options = shallowRef<ListOption[]>([]);
const loading = ref(false);
const failed = ref(false);
/** The text the shown options answer, null before any answer. */
const answered = ref<string | null>(null);
/** The list is wanted (typing, Down, Alt+Down); it opens once it has something to show. */
const wanted = ref(false);
/** Down opened the list: its first suggestion is highlighted when it comes. */
let toFirst = false;
let timer: ReturnType<typeof setTimeout> | undefined;
let controller: AbortController | undefined;

const shown = computed(
  () =>
    options.value.length > 0 ||
    loading.value ||
    failed.value ||
    (answered.value !== null && answered.value.trim() !== ""),
);
const open = computed(() => wanted.value && shown.value && !props.disabled);

/** What the list holds, in words (the polite region, and the note of an empty list). */
const said = computed(() => {
  if (!open.value) return "";
  if (options.value.length > 0) {
    const n = options.value.length;
    return `${n} ${n === 1 ? t("suggestionOne") : t("suggestionMany")}`;
  }
  if (loading.value) return t("suggestionsLoading");
  if (failed.value) return t("suggestionsFailed");
  return t("noResults");
});

/** Asks the host for the suggestions of a text, after `delay`; the newest text wins. */
function request(text: string, delay: number): void {
  clearTimeout(timer);
  controller?.abort();
  loading.value = true;
  timer = setTimeout(async () => {
    timer = undefined;
    const own = new AbortController();
    controller = own;
    try {
      const next = await props.suggest(text, own.signal);
      if (own.signal.aborted) return;
      options.value = next;
      failed.value = false;
    } catch {
      if (own.signal.aborted) return;
      options.value = [];
      failed.value = true;
    }
    answered.value = text;
    loading.value = false;
    settleHighlight();
  }, delay);
}

// Reka highlights a suggestion on its own (the first one as the text changes, the chosen
// one as it opens): the selection is manual here, so the highlight is cleared, as a
// pointer leaving the list does (its root listens), unless Down asked for the first one.
function clearHighlight(): void {
  const el = (anchor.value?.$el as HTMLElement | undefined)?.closest(".k-combobox__root");
  el?.dispatchEvent(new PointerEvent("pointerleave"));
  highlighted.value = undefined;
}

/**
 * The highlighted suggestion is the selected one of the list (APG, manual selection):
 * Reka binds `aria-selected` to the value taken, over any prop; it is set here on the
 * options from their highlight once rendered (Reka writes it again only when the value
 * taken changes, which this watch follows too).
 */
const highlighted = ref<string | undefined>();
function onHighlight(item: unknown): void {
  const picked = (item as { value?: unknown } | undefined)?.value;
  highlighted.value = typeof picked === "string" ? picked : undefined;
}
watch(
  [highlighted, value, options, open],
  async () => {
    await nextTick();
    const input = (anchor.value?.$el as HTMLElement | undefined)?.querySelector("input");
    const list = input && document.getElementById(input.getAttribute("aria-controls") ?? "");
    for (const option of list?.querySelectorAll("[role='option']") ?? [])
      option.setAttribute("aria-selected", String(option.hasAttribute("data-highlighted")));
  },
  { flush: "post" },
);

/** Once Reka's own highlight is done (it runs in microtasks): the first one, or none. */
function settleHighlight(): void {
  setTimeout(() => {
    if (!toFirst) clearHighlight();
    else if (open.value && !loading.value && options.value.length > 0) {
      toFirst = false;
      (root.value as unknown as { highlightFirstItem?: () => void } | null)?.highlightFirstItem?.();
    }
  }, 0);
}

/** Reka asks to open (typing, Down, a click) or to close (Escape, blur, a choice). */
function onOpen(next: boolean): void {
  wanted.value = next;
  if (!next) {
    toFirst = false;
    return;
  }
  if (timer === undefined && !loading.value && answered.value !== query.value)
    request(query.value, 0);
}

function onType(text: string): void {
  query.value = text;
  if (value.value !== undefined) value.value = undefined;
  wanted.value = true;
  toFirst = false;
  request(text, props.debounce);
  settleHighlight();
}

function onPick(picked: unknown): void {
  const option = options.value.find((o) => o.value === picked);
  if (!option) return;
  value.value = option.value;
  query.value = option.label;
  wanted.value = false;
  emit("select", option);
}

/**
 * Before Reka's handlers: Alt+Down opens without moving; Down on a closed or loading list
 * asks for its first suggestion once there; Home and End keep their place in the text.
 */
function onKeydownCapture(event: KeyboardEvent): void {
  if (event.key === "ArrowDown" && event.altKey) {
    event.stopPropagation();
    event.preventDefault();
    toFirst = false;
    onOpen(true);
  } else if (event.key === "ArrowDown" && (!open.value || loading.value)) {
    toFirst = true;
  } else if (event.key === "Home" || event.key === "End") {
    event.stopPropagation();
  }
}

/** After Reka's: an Enter it left (no highlighted suggestion) is the host's. */
function onKeydown(event: KeyboardEvent): void {
  if (event.key === "Enter" && !event.defaultPrevented && !event.isComposing) {
    wanted.value = false;
    emit("submit", query.value);
  }
}

// The list is at least as wide as its field, measured when it opens. Closed, nothing is
// highlighted any more: `aria-activedescendant` never points to a hidden option.
watch(open, async (now) => {
  if (!now) {
    toFirst = false;
    clearHighlight();
    settleHighlight();
    return;
  }
  await nextTick();
  const field = anchor.value?.$el as HTMLElement | undefined;
  const input = field?.querySelector("input");
  const list = input && document.getElementById(input.getAttribute("aria-controls") ?? "");
  list?.style.setProperty("--k-list-anchor-width", `${field!.offsetWidth}px`);
  settleHighlight();
});

onMounted(() => {
  if (props.defaultOpen) onOpen(true);
});

onBeforeUnmount(() => {
  clearTimeout(timer);
  controller?.abort();
});
</script>

<template>
  <FormField
    :id="id"
    class="k-combobox"
    :label="props.label"
    :hide-label="props.hideLabel"
    :hint="props.hint"
    :error="props.error"
    :required="props.required"
  >
    <template #default="field">
      <ComboboxRoot
        ref="root"
        class="k-combobox__root"
        :model-value="value ?? ''"
        :open="open"
        :disabled="props.disabled"
        ignore-filter
        :reset-search-term-on-blur="false"
        :reset-search-term-on-select="false"
        @update:open="onOpen"
        @update:model-value="onPick"
        @highlight="onHighlight"
      >
        <ComboboxAnchor
          ref="anchor"
          class="k-field-frame"
          :data-size="props.size"
          :data-invalid="props.error ? 'true' : undefined"
          @keydown.capture="onKeydownCapture"
          @keydown="onKeydown"
        >
          <Icon v-if="props.icon" :name="props.icon" class="k-field-frame__icon" />
          <ComboboxInput
            :id="field.id"
            :model-value="query"
            class="k-field-frame__input"
            :class="{ 'k-mono': props.mono }"
            :placeholder="props.placeholder"
            :required="props.required"
            :aria-describedby="field.describedBy"
            :aria-invalid="props.error ? 'true' : undefined"
            spellcheck="false"
            @update:model-value="onType"
          />
          <!-- Its place is kept: the field does not widen as a search starts. -->
          <Icon
            name="loader-circle"
            class="k-spinner k-field-frame__icon k-combobox__busy"
            :data-busy="loading ? 'true' : undefined"
          />
        </ComboboxAnchor>
        <ComboboxPortal :to="portal">
          <ComboboxContent
            class="k-list"
            position="popper"
            align="start"
            :side-offset="4"
            :aria-labelledby="field.labelId"
          >
            <ComboboxItem
              v-for="option in options"
              :key="option.value"
              class="k-list__option"
              :value="option.value"
              :text-value="option.label"
              :disabled="option.disabled === true"
            >
              <span class="k-list__check">
                <Icon v-if="option.value === value" name="check" />
              </span>
              <Icon v-if="option.icon" :name="option.icon" class="k-list__icon" />
              <span class="k-list__text" :class="{ 'k-mono': option.mono }">{{
                option.label
              }}</span>
              <code v-if="option.detail" class="k-list__detail">{{ option.detail }}</code>
            </ComboboxItem>
            <p v-if="options.length === 0" class="k-list__note" aria-hidden="true">
              <Icon v-if="failed && !loading" name="triangle-alert" />{{ said }}
            </p>
          </ComboboxContent>
        </ComboboxPortal>
      </ComboboxRoot>
      <!-- `aria-live` written out: a region Reka may hide around an open list stays live. -->
      <span class="k-visually-hidden" role="status" aria-live="polite">{{ said }}</span>
    </template>
  </FormField>
</template>

<style>
@layer components {
  /* The frame, its input and icons are the shared `k-field-frame`. The indicator keeps
   * its place, hidden and still while nothing loads. */
  .k-combobox__busy:not([data-busy]) {
    visibility: hidden;
    animation-name: none;
  }
}
</style>
