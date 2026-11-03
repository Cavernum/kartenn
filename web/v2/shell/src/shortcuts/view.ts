import { onScopeDispose, shallowRef, type Ref } from "vue";
import type { Period } from "../time/index.ts";

// The actions of the current view behind the shell's keys `/`, `c` and `e` (design 021
// « Raccourcis ») and the palette's commands « Créer… », « Modifier… » (REQ-021-3): a view
// declares them while it is shown (`useViewAction`), the frame plays them. `c` and `e` act
// at the present only — under `valid_at` alone too for an action that opens to the dated
// draft (`dated`, REQ-021-5), never under `as_of` — and only when the view says the user
// may (`allowed`: the write right; the server stays the judge).

export type ViewActionId = "filter" | "create" | "edit";

export interface ViewAction {
  /** Its label in the palette (« Créer un préfixe »), in the language of the moment. */
  label: () => string;
  run: () => void;
  /** Whether the user may now (the write right, an object or a cell current). */
  allowed?: () => boolean;
  /** It writes to the draft, which may be dated: it runs under `valid_at` alone too. */
  dated?: boolean;
}

export type ViewActions = Partial<Record<ViewActionId, ViewAction>>;

const actions = shallowRef<ViewActions>({});

/** The actions of the view shown now. */
export function viewActions(): Readonly<Ref<ViewActions>> {
  return actions;
}

/** Declares an action of the view; the returned function (or the end of the scope) removes it. */
export function setViewAction(id: ViewActionId, action: ViewAction): () => void {
  actions.value = { ...actions.value, [id]: action };
  return () => {
    if (actions.value[id] !== action) return;
    const rest = { ...actions.value };
    delete rest[id];
    actions.value = rest;
  };
}

/** In a view's `setup`: the action lives as long as the view. */
export function useViewAction(id: ViewActionId, action: ViewAction): void {
  onScopeDispose(setViewAction(id, action));
}

/**
 * The action `id` if it may run at `time` — `as_of` (nothing is written), `valid_at` alone
 * (the `dated` ones), the present — for `create` and `edit`.
 */
export function runnable(id: ViewActionId, time: Period): ViewAction | null {
  const action = actions.value[id];
  if (!action || action.allowed?.() === false) return null;
  if (id === "filter") return action;
  if (time.asOf !== null) return null;
  return time.validAt === null || action.dated === true ? action : null;
}
