<script setup lang="ts">
// Host of one extension of a plugin in its place (design 007 « hôtes », 021 REQ-021-13):
// creates the Custom Element of the plugin, gives it its properties (`object`, `row`), and
// confines an error of the plugin to its place — an encart with its code (`plugin.<motif>`),
// the rest of the screen usable. A plugin still loading has a skeleton of fixed size (a
// tile of the home page), one set aside its reason. `kartenn-close` from the element is
// passed on as `close` (an action in its dialog).
import type { ObjectRef } from "@kartenn/sdk";
import { InlineAlert, Skeleton, Stack } from "@kartenn/ui";
import { computed, inject, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { routerKey } from "vue-router";
import { reasonCode, type Status } from "./registry.ts";

const props = withDefaults(
  defineProps<{
    tag: string;
    /** The plugin that brings it (the console names it). */
    plugin: string;
    /** What the encart and the skeleton call it (its label, in the language). */
    label: string;
    object?: ObjectRef | undefined;
    /** The representation of the row (a column), as the API serves it. */
    row?: Record<string, unknown> | undefined;
    /** Where the plugin stands; loaded when unset (the hosts of loaded plugins only). */
    status?: Status | undefined;
  }>(),
  { object: undefined, row: undefined, status: undefined },
);
/**
 * `close`: the element asked it (`kartenn-close`); `followed`: a link of the element was
 * followed by the router; `failed`: an encart stands in its place.
 */
const emit = defineEmits<{ close: []; followed: []; failed: [failed: boolean] }>();

const { t } = useI18n();
const container = ref<HTMLElement | null>(null);
/** The error of the element in its place, if its creation failed. */
const failed = ref<string | null>(null);
let element: HTMLElement | null = null;

const state = computed<Status>(() => props.status ?? { state: "loaded" });
const reason = computed(() => {
  if (failed.value !== null) return "element" as const;
  const now = state.value;
  return now.state === "rejected" ? now.reason : null;
});

function onClose(): void {
  emit("close");
}

// The router of the shell, absent when the host is mounted alone (tests).
const router = inject(routerKey, null);

/**
 * A link of the element to a page of the shell (`<a href>` in its shadow root, the click
 * composed): followed by the router, the dates carried as by any link of the shell, rather
 * than by a reload of the page. A click with a modifier, another target, a download or an
 * address the shell does not route is left to the browser.
 */
function onClick(event: MouseEvent): void {
  if (!router || event.defaultPrevented || event.button !== 0) return;
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
  const anchor = event
    .composedPath()
    .find((node): node is HTMLAnchorElement => node instanceof HTMLAnchorElement);
  if (!anchor?.href || anchor.hasAttribute("download")) return;
  if (anchor.target && anchor.target !== "_self") return;
  const url = new URL(anchor.href, location.href);
  if (url.origin !== location.origin) return;
  // An anchor in the page itself (`#x`): the browser scrolls to it, nothing is pushed.
  if (url.hash && url.pathname === location.pathname && url.search === location.search) return;
  const to = `${url.pathname}${url.search}${url.hash}`;
  if (router.resolve(to).name === "missing") return;
  event.preventDefault();
  void router.push(to);
  // The host of an action closes its dialog: the page it stood on is left.
  emit("followed");
}

function unmount(): void {
  element?.removeEventListener("kartenn-close", onClose);
  element?.remove();
  element = null;
}

/**
 * Creates the element and appends it. An exception of its constructor or its connection is
 * reported by the browser, not thrown: an `error` of the window during the creation, which
 * runs no code but the element's (synchronous reactions), is the element's.
 */
function mount(): void {
  unmount();
  failed.value = null;
  if (!container.value || state.value.state !== "loaded") return;
  if (!customElements.get(props.tag)) {
    failed.value = `${props.tag} is not defined`;
    return;
  }
  let thrown: string | null = null;
  const onError = (event: ErrorEvent) => {
    thrown = event.message || "error";
    // Said once, by the shell, in its place and in the console.
    event.preventDefault();
  };
  window.addEventListener("error", onError);
  try {
    const created = document.createElement(props.tag) as HTMLElement & {
      object?: ObjectRef;
      row?: unknown;
    };
    if (props.object) created.object = props.object;
    if (props.row !== undefined) created.row = props.row;
    created.addEventListener("kartenn-close", onClose);
    container.value.append(created);
    element = created;
  } catch (error) {
    thrown = error instanceof Error ? error.message : String(error);
  } finally {
    window.removeEventListener("error", onError);
  }
  if (thrown !== null) {
    console.warn(`kartenn: extension ${props.tag} of ${props.plugin} failed: ${thrown}`);
    unmount();
    failed.value = thrown;
  }
}

watch(
  [container, () => props.tag, () => props.object, () => props.row, () => state.value.state],
  mount,
  { flush: "post" },
);
watch(reason, (now) => emit("failed", now !== null), { immediate: true });
onBeforeUnmount(unmount);
</script>

<template>
  <Stack v-if="state.state === 'loading'" gap="2" aria-busy="true" class="k-extension">
    <span class="k-visually-hidden">{{ t("plugins.loadingOf", { name: props.label }) }}</span>
    <Skeleton variant="text" :rows="3" />
  </Stack>
  <InlineAlert
    v-else-if="reason !== null"
    tone="ko"
    :title="t('plugins.failedIn', { name: props.label })"
    :items="[{ message: t(`plugins.reason.${reason}`), code: reasonCode(reason) }]"
  />
  <div
    v-show="reason === null && state.state === 'loaded'"
    ref="container"
    class="k-extension"
    @click="onClick"
  ></div>
</template>

<style>
@layer components {
  .k-extension {
    min-inline-size: 0;
  }
}
</style>
