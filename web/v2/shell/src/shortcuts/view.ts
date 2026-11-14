import { onScopeDispose, shallowRef, type Ref } from "vue";
import type { WidthClass } from "@kartenn/ui";
import { rankedRefusal, refusalWords, writesOf, type Time, type Writes } from "../time/index.ts";

// The actions of the current view behind the shell's keys `/`, `c` and `e` (design 021
// « Raccourcis ») and the palette's commands « Créer… », « Modifier… » (REQ-021-3): a view
// declares them while it is shown (`useViewAction`), the frame plays them. `c` and `e` act
// at the present only — under `valid_at` alone too for an action that opens to the dated
// draft (`dated`, REQ-021-5), never under `as_of` nor in a frame narrower than 1024 px
// (REQ-021-29) — and only when the view says the user may (`allowed`: the write right; the
// server stays the judge). Refused, the key says why by the polite status of the view
// (`say`, REQ-021-37) rather than nothing: the view's reason first (`why`: the permission
// it lacks, no domain), then the gate (the past, the width), as every refusal (task 21h).

export type ViewActionId = "filter" | "create" | "edit";

export interface ViewAction {
  /** Its label in the palette (« Créer un préfixe »), in the language of the moment. */
  label: () => string;
  run: () => void;
  /**
   * Whether the user may now, the gate of writes apart (the write right, an object or a
   * cell current, the view's own reasons): the gate is the frame's to read.
   */
  allowed?: () => boolean;
  /**
   * Why `allowed` says no, in words (« Lecture seule : il faut core.prefix.edit »), said
   * before the gate of writes as its button says it (`rankedRefusal`); `undefined` when
   * nothing is to be said (no object to act on: the key is let go).
   */
  why?: () => string | undefined;
  /** The polite status of the view, which says why the key is refused. */
  say?: (text: string) => void;
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
 * Whether the gate of writes lets the action `id` run: `as_of` and a frame under 1024 px
 * write nothing, `valid_at` alone the `dated` actions only.
 */
function fitsWrites(id: ViewActionId, action: ViewAction, writes: Writes): boolean {
  if (id === "filter") return true;
  return writes.draft && (writes.quickEdit || action.dated === true);
}

/**
 * The action `id` if it may run under `writes` (`useWrites`) — `as_of` and a narrow frame
 * (nothing is written), `valid_at` alone (the `dated` ones), the present — for `create` and
 * `edit`.
 */
export function runnable(id: ViewActionId, writes: Writes): ViewAction | null {
  const action = actions.value[id];
  if (!action || action.allowed?.() === false) return null;
  return fitsWrites(id, action, writes) ? action : null;
}

/**
 * The command of the palette for the action `id` at `time` in a frame of `width`: the
 * action when it may run; when the width under 1024 px is the only bar (the period and the
 * right let it run), a command of its label whose run says why by the view's status
 * (`play`), so that the palette shows what the width holds back; else none (off the
 * present or without the right, as the keys it stands for, at any width).
 */
export function offered(
  id: ViewActionId,
  time: Time,
  width: WidthClass,
  t: (key: string) => string,
): Pick<ViewAction, "label" | "run"> | null {
  const writes = writesOf(time, width);
  const action = runnable(id, writes);
  if (action) return action;
  const held = actions.value[id];
  // Kept only when the width alone holds it back: the period (without a width) lets it run.
  if (!held?.say || writes.refused !== "narrow" || !runnable(id, writesOf(time))) return null;
  return { label: held.label, run: () => play(id, writes, t) };
}

/**
 * The key of the action `id` pressed under `writes`: the action runs, or, refused, the
 * view's status says why, by the rank of every refusal (`rankedRefusal`, task 021-21h) —
 * the view's `why` first (the permission missing, its own reason), as its button and its
 * cells say it, then the gate (`edit.why.past` off the present, `edit.why.narrow` under
 * 1024 px) — and the key is taken. `false` (the key let go) when the view has no such
 * action, no status, or nothing to say (no object to act on).
 */
export function play(id: ViewActionId, writes: Writes, t: (key: string) => string): boolean {
  const action = actions.value[id];
  if (!action) return false;
  const may = action.allowed?.() !== false;
  const fits = fitsWrites(id, action, writes);
  if (may && fits) {
    action.run();
    return true;
  }
  const why = may ? undefined : action.why?.();
  // Nothing to act on (no row, no node chosen): nothing to say, at any date or width.
  if (!may && why === undefined) return false;
  const words = rankedRefusal({
    lasting: why,
    // Under `valid_at` alone, an action that is not `dated` waits for the present.
    gate: fits ? undefined : (refusalWords(writes, t) ?? t("edit.why.past")),
  });
  if (!words || !action.say) return false;
  action.say(words);
  return true;
}
