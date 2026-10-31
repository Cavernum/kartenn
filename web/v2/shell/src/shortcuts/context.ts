import { inject, provide, shallowRef, type InjectionKey, type Ref } from "vue";
import type { ShellShortcut } from "./registry.ts";

// The shortcuts the frame plays, given to the views that show them (the page
// /help/shortcuts): the registry of the shell, then those of the plugins.

const KEY: InjectionKey<Readonly<Ref<readonly ShellShortcut[]>>> = Symbol("kartenn.shortcuts");

export function provideShortcuts(list: Readonly<Ref<readonly ShellShortcut[]>>): void {
  provide(KEY, list);
}

/** The shortcuts of the frame; none out of it. */
export function useShortcuts(): Readonly<Ref<readonly ShellShortcut[]>> {
  return inject(KEY, shallowRef([]));
}
