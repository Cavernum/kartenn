// The Vue adapter of the data layer (ADR 0037, « Résolution de D-1 »): `@tanstack/vue-query`
// plugged into the one `QueryClient` of `@kartenn/sdk`, which stays without a framework
// (REQ-021-25). Screens read with `useQuery(reads.object(…))`; data stays shallow (lesson
// of the prototype: deep reactive wrappers of large reads cost and lose identity).
// Placed in the shell until task 8 moves it to @kartenn/ui with the shared modules.

import { context, events, label, labelDictionary, queryClient } from "@kartenn/sdk";
import type { StreamState } from "@kartenn/sdk";
import { VueQueryPlugin, useQuery, type QueryClient as VueQueryClient } from "@tanstack/vue-query";
import { computed, onScopeDispose, readonly, ref, shallowRef, type App, type Ref } from "vue";

/** Plugs the cache of the SDK into an application of the shell or of a first-party plugin. */
export function installData(app: App): void {
  // The same instance, not vue-query's subclass: that one only adds the unwrapping of refs
  // in options, which the reads built by the SDK never hold, and `isRestoring`, the ref
  // without which its composables never subscribe (useBaseQuery of 5.102.4, version
  // pinned; test/data.test.ts guards it).
  const client = queryClient as unknown as VueQueryClient;
  if (!client.isRestoring) Object.assign(client, { isRestoring: ref(false) });
  app.use(VueQueryPlugin, { queryClient: client, enableDevtoolsV6Plugin: false });
}

/** The value of a store of the SDK as a ref, followed while the scope lives. */
function follow<T>(
  read: () => T,
  subscribe: (listener: () => void) => () => void,
): Readonly<Ref<T>> {
  const value = shallowRef(read());
  const stop = subscribe(() => {
    value.value = read();
  });
  onScopeDispose(stop);
  return readonly(value) as Readonly<Ref<T>>;
}

/** The state of the event stream: `open`, `connecting`, `closed` (a banner without it). */
export function useEventsState(): Readonly<Ref<StreamState>> {
  return follow(
    () => events.state(),
    (listener) => events.onState(listener),
  );
}

/** The language of the context. */
export function useLocale(): Readonly<Ref<string>> {
  return follow(
    () => context.get().locale,
    (listener) => context.subscribe(listener),
  );
}

/**
 * The label function of the language of the context, reactive: what renders it renders
 * again when the dictionary arrives or another revision is read (REQ-021-19).
 */
export function useLabel(): (space: string, key: string) => string {
  const locale = useLocale();
  const dictionary = useQuery(
    computed(() => ({ ...labelDictionary(locale.value), shallow: true })),
  );
  return (space, key) => {
    void dictionary.data.value;
    return label(space, key, locale.value);
  };
}
