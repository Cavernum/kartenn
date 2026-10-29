<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A list of choices (design « Inventaire », line 5): one value among a closed list, on
// Reka's Select (APG Select-Only Combobox: a `combobox` button, its popup a `listbox`
// whose options take the focus). In a `FormField`: label, error then help by
// `aria-describedby`, the error said when it comes. Keys (Reka): Enter, Space, Down and Up
// open it on the chosen option (or the first one); the arrows, Home, End and typing move
// in it; Enter or Space chooses and closes; Escape closes without choosing; the focus goes
// back to the button; Tab chooses, closes and goes on (Reka keeps Tab in the list: it is
// handled here, before it). Read-only: focusable, `aria-readonly`, never opens. `inlineLabel`
// writes the label in the button before the value (a choice in a toolbar), the label
// itself staying for assistive technologies. No SelectViewport: it injects a <style>
// (REQ-020-33); the list scrolls by itself. In the layer of floating layers
// (`providePortalTarget`), at least as wide as its button (CSSOM, REQ-020-14); its look,
// shared with `Combobox`, is `options.css` (in the sheet of the library, style.css).
import {
  SelectContent,
  SelectItem,
  SelectItemIndicator,
  SelectItemText,
  SelectPortal,
  SelectRoot,
  SelectTrigger,
  SelectValue,
} from "reka-ui";
import { nextTick, ref, useId, watch, type ComponentPublicInstance } from "vue";
import { injectPortalTarget } from "../../context.ts";
import type { IconName } from "../../icons/index.ts";
import { t } from "../../i18n.ts";
import FormField from "../FormField/FormField.vue";
import Icon from "../Icon/Icon.vue";
import type { ControlSize } from "../layout.ts";
import type { ListOption } from "./select.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    options: ListOption[];
    /** The label stays for assistive technologies only. */
    hideLabel?: boolean;
    /** The label written in the button before the value (a toolbar), hidden above it. */
    inlineLabel?: boolean;
    hint?: string | undefined;
    /** The error: sets `aria-invalid` and is read with the button. */
    error?: string | undefined;
    /** Shown while no value is chosen. */
    placeholder?: string | undefined;
    icon?: IconName | undefined;
    /** The chosen label in fixed pitch (a key, a prefix). */
    mono?: boolean;
    size?: ControlSize;
    required?: boolean;
    readonly?: boolean;
    disabled?: boolean;
    /** Open at first render (the catalogue). */
    defaultOpen?: boolean;
  }>(),
  { hint: undefined, error: undefined, placeholder: undefined, icon: undefined, size: "md" },
);
const model = defineModel<string | undefined>({ default: undefined });

const portal = injectPortalTarget();
const id = useId();
const open = ref(props.defaultOpen && !props.readonly && !props.disabled);
const trigger = ref<ComponentPublicInstance | null>(null);
const TABBABLE =
  "a[href], button:not(:disabled), input:not(:disabled), select:not(:disabled), " +
  "textarea:not(:disabled), [tabindex]:not([tabindex='-1'])";

// Closed, Reka leaves an empty <div> where its list goes (its options live in a detached
// fragment, for the value and the typing ahead): it stays in place, in the field, rather
// than adding an item to the layer of the host (a grid, its gap). The list goes to the
// layer when it opens, and comes back once it is gone (after its fade).
const teleported = ref(open.value);

function onOpen(next: boolean): void {
  if (props.readonly) return;
  open.value = next;
  if (next) teleported.value = true;
}

// No value is the empty string for Reka (its placeholder), `undefined` for the host.
/** Tab in the list (APG): takes the focused option, closes, and goes on from the button. */
let tabbed: 1 | -1 | 0 = 0;
function onListKey(event: KeyboardEvent): void {
  if (event.key !== "Tab" || event.ctrlKey || event.altKey || event.metaKey) return;
  const list = event.currentTarget as HTMLElement;
  const focused = (document.activeElement as HTMLElement | null)?.closest("[role='option']");
  const index = [...list.querySelectorAll("[role='option']")].indexOf(focused as Element);
  const option = props.options[index];
  if (option && !option.disabled) model.value = option.value;
  tabbed = event.shiftKey ? -1 : 1;
  open.value = false;
}

/** Back to the button (Reka), or, after Tab, to the control before or after it. */
function onClosed(event: Event): void {
  teleported.value = false;
  if (!tabbed) return;
  const button = trigger.value?.$el as HTMLElement | undefined;
  const step = tabbed;
  tabbed = 0;
  if (!button) return;
  event.preventDefault();
  const all = [...document.querySelectorAll<HTMLElement>(TABBABLE)].filter(
    (el) => el === button || (el.tabIndex >= 0 && el.getClientRects().length > 0),
  );
  (all[all.indexOf(button) + step] ?? button).focus();
}

function onChoose(next: unknown): void {
  model.value = typeof next === "string" && next !== "" ? next : undefined;
}

/** A read-only select keeps its value: typing ahead on the button would change it. */
function onReadonlyKey(event: KeyboardEvent): void {
  if (props.readonly && event.key !== "Tab") event.stopPropagation();
}

// The list is at least as wide as its button, measured when it opens.
watch(
  open,
  async (now) => {
    if (!now) return;
    await nextTick();
    const button = trigger.value?.$el as HTMLElement | undefined;
    if (!button) return;
    const list = document.getElementById(button.getAttribute("aria-controls") ?? "");
    list?.style.setProperty("--k-list-anchor-width", `${button.offsetWidth}px`);
  },
  { immediate: true },
);
</script>

<template>
  <FormField
    :id="id"
    class="k-select"
    :label="props.label"
    :hide-label="props.hideLabel || props.inlineLabel"
    :hint="props.hint"
    :error="props.error"
    :required="props.required"
  >
    <template #default="field">
      <SelectRoot
        :model-value="model ?? ''"
        :open="open"
        :disabled="props.disabled"
        :required="props.required"
        @update:open="onOpen"
        @update:model-value="onChoose"
      >
        <SelectTrigger
          :id="field.id"
          ref="trigger"
          class="k-field-frame k-select__trigger"
          :data-size="props.size"
          :data-invalid="props.error ? 'true' : undefined"
          :data-readonly="props.readonly ? 'true' : undefined"
          :aria-labelledby="field.labelId"
          :aria-describedby="field.describedBy"
          :aria-invalid="props.error ? 'true' : undefined"
          :aria-required="props.required ? 'true' : undefined"
          :aria-readonly="props.readonly ? 'true' : undefined"
          @keydown.capture="onReadonlyKey"
          @pointerdown.capture="onReadonlyKey"
        >
          <Icon v-if="props.icon" :name="props.icon" class="k-select__icon" />
          <span v-if="props.inlineLabel" class="k-select__prefix" aria-hidden="true"
            >{{ props.label }}{{ t("colon") }}</span
          >
          <SelectValue
            class="k-select__value"
            :class="{ 'k-mono': props.mono }"
            :placeholder="props.placeholder ?? ''"
          />
          <Icon name="chevron-down" class="k-select__chevron" />
        </SelectTrigger>
        <SelectPortal :to="portal" :disabled="!teleported">
          <SelectContent
            class="k-list"
            position="popper"
            align="start"
            :side-offset="4"
            :aria-labelledby="field.labelId"
            @keydown.capture="onListKey"
            @close-auto-focus="onClosed"
          >
            <!-- Reka names an option by its text alone: with a detail, by both (the name
                 holds what is written, WCAG 2.5.3); the button shows the text alone. -->
            <SelectItem
              v-for="(option, index) in props.options"
              :key="option.value"
              class="k-list__option"
              :value="option.value"
              :text-value="option.label"
              :disabled="option.disabled === true"
              v-bind="
                option.detail
                  ? { 'aria-labelledby': `${id}-option-${index} ${id}-option-${index}-detail` }
                  : {}
              "
            >
              <span class="k-list__check">
                <SelectItemIndicator><Icon name="check" /></SelectItemIndicator>
              </span>
              <Icon v-if="option.icon" :name="option.icon" class="k-list__icon" />
              <SelectItemText
                v-bind="option.detail ? { id: `${id}-option-${index}` } : {}"
                class="k-list__text"
                :class="{ 'k-mono': option.mono }"
                >{{ option.label }}</SelectItemText
              >
              <code
                v-if="option.detail"
                :id="`${id}-option-${index}-detail`"
                class="k-list__detail"
                >{{ option.detail }}</code
              >
            </SelectItem>
          </SelectContent>
        </SelectPortal>
      </SelectRoot>
    </template>
  </FormField>
</template>

<style>
@layer components {
  .k-select__trigger {
    inline-size: 100%;
    font: inherit;
    text-align: start;
    cursor: default;
  }

  .k-select__trigger:focus-visible {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: var(--k-focus-offset);
  }

  .k-select__trigger:disabled {
    color: var(--k-text-muted);
    background-color: var(--k-surface-sunken);
    border-color: var(--k-border-strong);
  }

  .k-select__trigger[data-readonly] {
    background-color: var(--k-surface-sunken);
    border-style: dashed;
  }

  .k-select__prefix {
    flex: none;
    color: var(--k-text-muted);
  }

  /* The chosen label goes to the next line rather than being cut. */
  .k-select__value {
    flex: 1;
    min-inline-size: 0;
    overflow-wrap: anywhere;
  }

  .k-select__value.k-mono:not([data-placeholder]) {
    font-family: var(--k-font-mono);
    font-size: var(--k-text-mono);
  }

  .k-select__value[data-placeholder] {
    color: var(--k-text-muted);
  }

  .k-select__icon,
  .k-select__chevron {
    flex: none;
    color: var(--k-text-muted);
  }

  .k-select__chevron {
    transition-property: rotate;
    transition-duration: var(--k-duration-short);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-select__trigger[aria-expanded="true"] .k-select__chevron {
    rotate: var(--k-turn-half);
  }

  .k-select__trigger:disabled .k-select__chevron,
  .k-select__trigger[data-readonly] .k-select__chevron {
    opacity: 0;
  }
}
</style>
