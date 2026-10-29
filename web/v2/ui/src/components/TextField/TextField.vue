<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// A labelled native text field (design « Inventaire », line 4), in a `FormField`: label,
// then error and help linked by `aria-describedby` (error first), the error said in a
// live region when it comes, `aria-invalid` with it; read-only, disabled, pending
// (`aria-busy`, indicator, the word in the description); fixed-pitch variant for
// addresses, prefixes and identifiers. Heights of the controls (REQ-020-17).
import { computed, useId } from "vue";
import type { IconName } from "../../icons/index.ts";
import { t } from "../../i18n.ts";
import FormField from "../FormField/FormField.vue";
import Icon from "../Icon/Icon.vue";
import type { ControlSize } from "../layout.ts";

const props = withDefaults(
  defineProps<{
    label: string;
    /** The label stays for assistive technologies only (a cell editor, a search box). */
    hideLabel?: boolean;
    type?: "text" | "search" | "email" | "url" | "password" | "tel";
    size?: ControlSize;
    hint?: string | undefined;
    /** The error: sets `aria-invalid` and is read with the field. */
    error?: string | undefined;
    placeholder?: string | undefined;
    icon?: IconName | undefined;
    mono?: boolean;
    required?: boolean;
    readonly?: boolean;
    disabled?: boolean;
    /** A check or a save under way. */
    pending?: boolean;
    autocomplete?: string | undefined;
  }>(),
  {
    type: "text",
    size: "md",
    hint: undefined,
    error: undefined,
    placeholder: undefined,
    icon: undefined,
    autocomplete: undefined,
  },
);
const model = defineModel<string>({ default: "" });

// The ids of the error and the help are those of the `FormField` (`<id>-error`,
// `<id>-hint`); pending goes between them, said in words too: under reduced motion the
// indicator no longer turns.
const id = useId();
const hintId = `${id}-hint`;
const errorId = `${id}-error`;
const pendingId = `${id}-pending`;
const describedBy = computed(
  () =>
    [props.error ? errorId : null, props.pending ? pendingId : null, props.hint ? hintId : null]
      .filter(Boolean)
      .join(" ") || undefined,
);
</script>

<template>
  <FormField
    :id="id"
    class="k-text-field"
    :label="props.label"
    :hide-label="props.hideLabel"
    :hint="props.hint"
    :error="props.error"
    :required="props.required"
  >
    <div
      class="k-field-frame"
      :data-size="props.size"
      :data-invalid="props.error ? 'true' : undefined"
    >
      <Icon v-if="props.icon" :name="props.icon" class="k-field-frame__icon" />
      <input
        :id="id"
        v-model="model"
        class="k-field-frame__input"
        :class="{ 'k-mono': props.mono }"
        :type="props.type"
        :placeholder="props.placeholder"
        :required="props.required"
        :readonly="props.readonly"
        :disabled="props.disabled"
        :autocomplete="props.autocomplete"
        :aria-invalid="props.error ? 'true' : undefined"
        :aria-describedby="describedBy"
        :aria-busy="props.pending ? 'true' : undefined"
        spellcheck="false"
      />
      <Icon v-if="props.pending" name="loader-circle" class="k-spinner k-field-frame__icon" />
      <span v-if="props.pending" :id="pendingId" class="k-visually-hidden">{{ t("loading") }}</span>
    </div>
  </FormField>
</template>
