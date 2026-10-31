<!-- This Source Code Form is subject to the terms of the Mozilla Public
     License, v. 2.0. If a copy of the MPL was not distributed with this
     file, You can obtain one at https://mozilla.org/MPL/2.0/. -->

<script setup lang="ts">
// The toasts of the application (design « Inventaire », line 14), one region in the frame,
// at the bottom of the window, in its middle, under every modal layer. Two live regions,
// present and empty before any toast (a region inserted with its text is not announced):
// `alert` for the errors, said at once and kept until dismissed, `status` for the others,
// said politely; each toast added to one is said alone (`aria-atomic="false"`). No toast
// takes the focus. They are reached by Tab, after the rest of the frame, and the region
// holding toasts is a landmark (« Notifications »); while the pointer is over them, the
// focus in them, the page hidden or a modal layer over them (`aria-hidden`, `inert`), no
// toast goes (WCAG 2.2.1). Escape in a toast dismisses it; dismissed from the keyboard,
// the focus goes to the next toast, else the previous one, else back where it came from,
// else to the main content (WCAG 2.4.3).
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { t } from "../../i18n.ts";
import Toast from "./Toast.vue";
import { createToasts, injectToasts, type Toasts } from "./toasts.ts";

const props = defineProps<{
  /** The toasts shown; by default, those the host provides (`provideToasts`). */
  toasts?: Toasts | undefined;
}>();
const store = props.toasts ?? injectToasts() ?? createToasts();
const errors = computed(() => store.list.filter((toast) => toast.tone === "ko"));
const others = computed(() => store.list.filter((toast) => toast.tone !== "ko"));

const root = ref<HTMLElement | null>(null);
const hovered = ref(false);
const focused = ref(false);
const hidden = ref(false);
const covered = ref(false);
const paused = computed(() => hovered.value || focused.value || hidden.value || covered.value);

const onVisibility = () => (hidden.value = document.hidden);
// A modal layer hides the rest of the page (`aria-hidden` by Reka, `inert` by a sheet):
// a toast under it would go unseen and unsaid.
const onCover = () =>
  (covered.value = root.value?.closest("[aria-hidden='true'], [inert]") != null);
let cover: MutationObserver | undefined;
onMounted(() => {
  onVisibility();
  document.addEventListener("visibilitychange", onVisibility);
  onCover();
  cover = new MutationObserver(onCover);
  cover.observe(document.body, {
    attributes: true,
    attributeFilter: ["aria-hidden", "inert"],
    subtree: true,
  });
});
onBeforeUnmount(() => {
  document.removeEventListener("visibilitychange", onVisibility);
  cover?.disconnect();
});

/** What had the focus before it came into the toasts; none from outside the page. */
let returnTo: HTMLElement | null = null;
function onFocusIn(event: FocusEvent): void {
  focused.value = true;
  const from = event.relatedTarget;
  if (from instanceof Node && root.value?.contains(from)) return;
  returnTo = from instanceof HTMLElement ? from : null;
}
function onFocusOut(event: FocusEvent): void {
  const to = event.relatedTarget;
  if (!(to instanceof Node && root.value?.contains(to))) focused.value = false;
}

/** The last dismissal came from the pointer: the focus is not handed on. */
let byPointer = false;
function onKey(event: KeyboardEvent): void {
  byPointer = false;
  // Kept by a layer that owns Escape first (an open tooltip, `tooltip.ts`).
  if (event.key !== "Escape" || event.defaultPrevented) return;
  const holder = (event.target as HTMLElement).closest<HTMLElement>("[data-toast-id]");
  if (!holder?.dataset.toastId) return;
  event.preventDefault();
  store.dismiss(holder.dataset.toastId);
}

// A toast that goes with the focus in it: dismissed from the keyboard (its close button,
// Escape) or by the host, the focus moves first, before it leaves the page; by a click,
// it is let go (the button blurred, the toasts no longer held).
watch(
  () => store.list.map((toast) => toast.id),
  (ids) => {
    const at = document.activeElement;
    if (!root.value || !(at instanceof HTMLElement) || !root.value.contains(at)) return;
    const holder = at.closest<HTMLElement>("[data-toast-id]");
    if (!holder || ids.includes(holder.dataset.toastId ?? "")) return;
    if (byPointer) {
      at.blur();
      focused.value = false;
      return;
    }
    const items = [...root.value.querySelectorAll<HTMLElement>("[data-toast-id]")].filter(
      (item) => item === holder || ids.includes(item.dataset.toastId ?? ""),
    );
    const index = items.indexOf(holder);
    const beside = items[index + 1] ?? items[index - 1];
    // Come in from outside the page (Shift+Tab from the browser), there is no way back:
    // the main content, when it takes the focus (the frame's surface, `tabindex="-1"`).
    const target =
      beside?.querySelector<HTMLElement>(".k-toast__close") ??
      (returnTo?.isConnected ? returnTo : document.querySelector<HTMLElement>("main[tabindex]"));
    target?.focus();
  },
  { flush: "pre" },
);

// A node removed with the focus or the pointer on it says nothing (no `focusout`, no
// `pointerleave`): with no toast left, nothing holds the next ones.
watch(
  () => store.list.length,
  (count) => {
    if (count > 0) return;
    focused.value = false;
    hovered.value = false;
  },
);

/** Out, a toast fades (its `data-state`), then leaves the page. */
function onLeave(el: Element, done: () => void): void {
  const item = el as HTMLElement;
  item.dataset.state = "closed";
  const running = item.getAnimations();
  if (running.length === 0) return done();
  void Promise.all(running.map((animation) => animation.finished)).then(done, done);
}
</script>

<template>
  <section
    ref="root"
    class="k-toasts"
    :aria-label="store.list.length > 0 ? t('notifications') : undefined"
    @pointerenter="hovered = true"
    @pointerleave="hovered = false"
    @pointerdown.capture="byPointer = true"
    @focusin="onFocusIn"
    @focusout="onFocusOut"
    @keydown="onKey"
  >
    <div class="k-toasts__lane" role="alert" aria-atomic="false">
      <TransitionGroup
        tag="ol"
        class="k-toasts__list"
        :role="errors.length > 0 ? undefined : 'none'"
        :css="false"
        @leave="onLeave"
      >
        <Toast
          v-for="toast in errors"
          :key="toast.id"
          :toast="toast"
          :paused="paused"
          @dismiss="store.dismiss(toast.id)"
        />
      </TransitionGroup>
    </div>
    <div class="k-toasts__lane" role="status" aria-atomic="false">
      <TransitionGroup
        tag="ol"
        class="k-toasts__list"
        :role="others.length > 0 ? undefined : 'none'"
        :css="false"
        @leave="onLeave"
      >
        <Toast
          v-for="toast in others"
          :key="toast.id"
          :toast="toast"
          :paused="paused"
          @dismiss="store.dismiss(toast.id)"
        />
      </TransitionGroup>
    </div>
  </section>
</template>

<style>
@layer components {
  /* At the bottom of the window, in its middle: the corners hold the actions of the
   * frame's panels and drawers (« Soumettre », a `CommitForm`), which an error, kept until
   * dismissed, would cover. Under a modal layer and its scrim, as the rest of the page it
   * hides (`--k-z-overlay`, not `--k-z-toast`, which is the tooltips'): a modal layer
   * shows its outcomes inside it. Only the toasts take the pointer. Many toasts scroll
   * inside the region, at most half the window (each one has its close button: reached by
   * Tab, the region follows the focus). */
  .k-toasts {
    position: fixed;
    inset-block-end: var(--k-space-4);
    inset-inline: 0;
    margin-inline: auto;
    z-index: var(--k-z-overlay);
    display: flex;
    flex-direction: column;
    inline-size: min(var(--k-size-dialog-sm), calc(100% - var(--k-space-4) * 2));
    max-block-size: 50%;
    overflow: auto;
    pointer-events: none;
  }

  .k-toasts__list {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--k-space-2);
    padding: 0;
    margin: 0;
    list-style: none;
  }

  .k-toasts__lane:has(.k-toast) + .k-toasts__lane:has(.k-toast) {
    margin-block-start: var(--k-space-2);
  }
}
</style>
