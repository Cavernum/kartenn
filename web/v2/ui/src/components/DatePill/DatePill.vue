<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The date pill of the status bar (APG Button opening a non-modal dialog; design
// « Inventaire », line 27; spec 021, « Cadre », REQ-021-5): « Présent », or the dated
// context read (`valid_at`, `as_of` « connu au »), in the colours of the band, with its
// named time zone; named « Date de lecture : … ». Pressed, it opens the period picker
// (`PeriodPicker`) in a layer under it (Reka's Popover, on the plane of the floating
// layers): named by the picker's title, the focus on its first field; Escape or the close
// button close it and give the focus back to the pill, a press outside closes it. Nothing
// around it is hidden or inert: it reads as if it followed the pill (Shift+Tab from its
// start goes back to the pill, Tab past its end to what follows it), and it closes as the
// focus leaves it, so that a layer covering a narrow window never hides the focus behind
// it (2.4.11). Applied, the period goes to the host (`apply`), which reads it: the layer
// closes once the period read changes (`validAt`, `asOf`), and stays open on a refusal of
// the API (a 400 on a date), which comes back on its field by `errors`, the focus on it.
import { PopoverClose, PopoverContent, PopoverPortal, PopoverRoot, PopoverTrigger } from "reka-ui";
import { computed, ref, useId, watch, type ComponentPublicInstance } from "vue";
import { injectPortalTarget } from "../../context.ts";
import { t } from "../../i18n.ts";
import { formatInstant } from "../../time.ts";
import type { Period } from "../DateTimeField/dates.ts";
import Icon from "../Icon/Icon.vue";
import IconButton from "../IconButton/IconButton.vue";
import { setRoom } from "../internal/room.ts";
import { tabOut, tabbables } from "../internal/tabOrder.ts";
import PeriodPicker from "../PeriodPicker/PeriodPicker.vue";

const props = withDefaults(
  defineProps<{
    /** The validity date read (`valid_at`), `null` at the present. */
    validAt?: string | null;
    /** The system date read (`as_of`), `null` at the present. */
    asOf?: string | null;
    /** The named zone (IANA) in which the dates are shown and typed. */
    timeZone: string;
    /** The host's refusal of a date (the API's 400, with its reason), on its field. */
    errors?: Partial<Record<keyof Period, string>> | undefined;
    /** Open at first (the catalogue, a mock-up). */
    defaultOpen?: boolean;
  }>(),
  { validAt: null, asOf: null, errors: undefined },
);
const open = defineModel<boolean>("open", { default: false });
if (props.defaultOpen) open.value = true;
const emit = defineEmits<{ apply: [period: Period] }>();

const portal = injectPortalTarget();
const titleId = `${useId()}-title`;
const trigger = ref<ComponentPublicInstance | null>(null);
const triggerEl = () => trigger.value?.$el as HTMLElement | undefined;
/** Left by Tab or Shift+Tab: where the focus goes once it is closed. */
let leaving: HTMLElement | undefined;

const past = computed(() => props.validAt !== null || props.asOf !== null);

/** What the pill shows: words, and each date a `<time>` (its instant, machine-readable). */
type Part = string | { iso: string; text: string };
const parts = computed<Part[]>(() => {
  const date = (at: string): Part => ({
    iso: new Date(at).toISOString(),
    text: formatInstant(at, props.timeZone),
  });
  if (!past.value) return [t("present")];
  const out: Part[] = [];
  if (props.validAt !== null) out.push(date(props.validAt));
  if (props.asOf !== null)
    out.push(
      props.validAt !== null ? ` · ${t("knownAt")} ` : `${t("periodAsOf")} `,
      date(props.asOf),
    );
  return out;
});

const sameInstant = (a: string | null, b: string | null) =>
  a === b || (a !== null && b !== null && Date.parse(a) === Date.parse(b));

/**
 * The host's refusal (a new object for each one), shown while the layer that sent the
 * period is open: one that arrives with the layer closed is dropped, and opening or
 * closing it clears the last one (the picker starts again from the period read).
 */
const shownErrors = ref(open.value ? props.errors : undefined);
watch(
  () => props.errors,
  (errors) => {
    if (open.value) shownErrors.value = errors;
  },
);
watch(open, () => (shownErrors.value = undefined));

/** The focus on the pill: for the host, when the control that had it goes (the band). */
defineExpose({ focus: () => triggerEl()?.focus() });

/** The period read changed: the host has read it, the layer has done its work. */
watch(
  () => [props.validAt, props.asOf],
  () => (open.value = false),
);

/** The period read again closes the layer at once; another goes to the host. */
function onApply(period: Period): void {
  if (sameInstant(period.validAt, props.validAt) && sameInstant(period.asOf, props.asOf))
    open.value = false;
  else emit("apply", period);
}

/**
 * Open, the dialog is named after the picker's title (Reka names it after its trigger,
 * over any prop), bounded by the viewport (1.4.10), and the focus goes to the first field,
 * or to the one the host refused.
 */
function onOpenFocus(event: Event): void {
  leaving = undefined;
  const wrapper = event.target as HTMLElement;
  const el = wrapper.querySelector<HTMLElement>(":scope > [role='dialog']");
  if (!el) return;
  event.preventDefault();
  el.setAttribute("aria-labelledby", titleId);
  const rect = triggerEl()?.getBoundingClientRect();
  if (rect) setRoom(el, rect, "popover");
  const picker = el.querySelector<HTMLElement>(".k-period-picker");
  const refused = picker?.querySelector<HTMLElement>("[role='spinbutton'][aria-invalid='true']");
  (refused ?? (picker ? tabbables(picker)[0] : el))?.focus({ preventScroll: true });
}

/** Closed by Tab or Shift+Tab, the focus has gone on: Reka would bring it back. */
function onCloseFocus(event: Event): void {
  if (leaving) event.preventDefault();
}

function onKey(event: KeyboardEvent): void {
  const from = triggerEl();
  const to = from && tabOut(event, event.currentTarget as HTMLElement, from);
  if (!to) return;
  event.preventDefault();
  event.stopPropagation();
  leaving = to;
  open.value = false;
  leaving.focus();
}
</script>

<template>
  <PopoverRoot v-model:open="open">
    <PopoverTrigger ref="trigger" as-child>
      <button type="button" class="k-date-pill" :data-period="past ? 'past' : 'present'">
        <Icon :name="props.asOf !== null ? 'lock' : past ? 'rotate-ccw-clock' : 'clock'" />
        <span class="k-visually-hidden">{{ t("datePill") }}{{ t("colon") }} </span>
        <span class="k-date-pill__label">
          <template v-for="(part, i) in parts" :key="i">
            <time v-if="typeof part !== 'string'" :datetime="part.iso">{{ part.text }}</time>
            <template v-else>{{ part }}</template>
          </template>
        </span>
        <span v-if="past" class="k-date-pill__zone">{{ props.timeZone }}</span>
        <Icon name="chevron-down" class="k-date-pill__chevron" />
      </button>
    </PopoverTrigger>
    <PopoverPortal :to="portal">
      <PopoverContent
        class="k-popover k-date-pill__layer"
        side="bottom"
        align="end"
        :side-offset="4"
        :collision-padding="8"
        @open-auto-focus="onOpenFocus"
        @close-auto-focus="onCloseFocus"
        @keydown.capture="onKey"
      >
        <PopoverClose as-child>
          <IconButton
            icon="x"
            :label="t('close')"
            size="sm"
            variant="ghost"
            class="k-date-pill__close"
          />
        </PopoverClose>
        <PeriodPicker
          :title-id="titleId"
          :valid-at="props.validAt"
          :as-of="props.asOf"
          :time-zone="props.timeZone"
          :errors="shownErrors"
          @apply="onApply"
        />
      </PopoverContent>
    </PopoverPortal>
  </PopoverRoot>
</template>

<style>
@layer components {
  .k-date-pill {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
    min-block-size: var(--k-control-height-md);
    padding-inline: var(--k-control-padding-inline-md) var(--k-control-padding-inline-sm);
    font: inherit;
    font-weight: var(--k-weight-medium);
    color: var(--k-text);
    white-space: nowrap;
    cursor: pointer;
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-control);
    border-radius: var(--k-radius-full);
    transition-property: background-color;
    transition-duration: var(--k-duration-fade);
    transition-timing-function: var(--k-ease-standard);
  }

  /* The chevron turns while the picker is open (design « Mouvement »). */
  .k-date-pill__chevron {
    transition-property: rotate;
    transition-duration: var(--k-duration-short);
    transition-timing-function: var(--k-ease-standard);
  }

  .k-date-pill[aria-expanded="true"] .k-date-pill__chevron {
    rotate: var(--k-turn-half);
  }

  .k-date-pill:hover {
    background-color: var(--k-hover);
  }

  .k-date-pill[data-period="past"] {
    color: var(--k-band-fg);
    background-color: var(--k-band-bg);
    border-color: var(--k-band-fg);
  }

  .k-date-pill time {
    font-variant-numeric: tabular-nums;
  }

  .k-date-pill__zone {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
    font-weight: var(--k-weight-regular);
  }

  .k-date-pill[data-period="present"] .k-icon {
    color: var(--k-text-muted);
  }

  /* In the compact frame (its `data-width`, design « Adaptation à la largeur »), the icon
   * alone: the dates, the zone or « présent » stay its name, hidden on screen only. */
  [data-width="compact"] .k-date-pill__label,
  [data-width="compact"] .k-date-pill__zone {
    position: absolute;
    inline-size: var(--k-space-px);
    block-size: var(--k-space-px);
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  /* The layer: the picker, its close button at the end of its title's line, in the same
   * cell (the title is short: the fields keep the whole width); bounded by the viewport
   * (the room set by CSSOM), it scrolls inside. On the floating surface, it reads as laid
   * on the page (024, relecture du 2026-10-26, E6): the shadow of the upper level, a
   * contrasted border, a background of its own, in both modes. */
  .k-popover.k-date-pill__layer {
    display: grid;
    grid-template-areas: "layer";
    grid-template-columns: minmax(0, 1fr);
    align-items: start;
    inline-size: var(--k-size-panel-lg);
    max-inline-size: var(--k-popover-room-inline, var(--k-size-panel-lg));
    max-block-size: var(--k-popover-room-block, var(--k-size-dialog-md));
    padding: var(--k-space-3);
    overflow: auto;
    background-color: var(--k-surface-raised);
    border-color: var(--k-border-control);
    box-shadow: var(--k-shadow-modal);
  }

  .k-date-pill__layer > .k-period-picker {
    grid-area: layer;
  }

  /* Centred on the line of the title (its height less the button's, halved); painted
   * over the picker that shares its cell (`order`; the tab order keeps it first). */
  .k-date-pill__close {
    order: 1;
    grid-area: layer;
    justify-self: end;
    margin-block-start: calc((var(--k-text-md-line) - var(--k-control-height-sm)) / 2);
  }
}
</style>
