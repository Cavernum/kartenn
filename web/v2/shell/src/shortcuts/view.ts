import { onScopeDispose, shallowRef, type Ref } from "vue";

// The actions of the current view behind the shell's keys `/`, `c` and `e` (design 021
// « Raccourcis ») and the palette's commands « Créer… », « Modifier… » (REQ-021-3): a view
// declares them while it is shown (`useViewAction`), the frame plays them. `c` and `e` act
// at the present only, and only when the view says the user may (`allowed`: the write
// right; the server stays the judge).

export type ViewActionId = "filter" | "create" | "edit";

export interface ViewAction {
  /** Its label in the palette (« Créer un préfixe »), in the language of the moment. */
  label: () => string;
  run: () => void;
  /** Whether the user may now (the write right, an object or a cell current). */
  allowed?: () => boolean;
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

/** The action `id` if it may run now (`present` for `create` and `edit`). */
export function runnable(id: ViewActionId, present: boolean): ViewAction | null {
  const action = actions.value[id];
  if (!action || action.allowed?.() === false) return null;
  if (id !== "filter" && !present) return null;
  return action;
}
