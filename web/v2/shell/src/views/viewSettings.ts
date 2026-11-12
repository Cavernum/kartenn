import { computed, ref, watch, type WritableComputedRef } from "vue";
import { useSession } from "../rights/index.ts";

// The settings of a view kept for its user (Q-021-18, task 12i): the columns chosen in a
// list (« Colonnes »: shown, hidden, a property added), by view and by user, in
// `localStorage` under the user's identity (a UUID, never a name), as the recent objects
// are: another user of the same browser never gets them, and the logout clears them with
// every `kartenn.*` key it does not keep (`clearSensitive`). Signed out, a choice holds
// for the page only.

export const VIEW_KEY = "kartenn.view";

/** The choice of the columns of a grid by id (`columnVisibility` of the `DataGrid`). */
export type ColumnVisibility = Record<string, boolean>;

function storage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

/** The key of the columns of the view `view` for the user `owner`. */
export const columnsKey = (owner: string, view: string) => `${VIEW_KEY}.${owner}.${view}.columns`;

/** The columns kept for `view` by `owner`: a choice by column id, nothing when none. */
export function readColumns(owner: string, view: string): ColumnVisibility {
  try {
    const kept = JSON.parse(storage()?.getItem(columnsKey(owner, view)) ?? "{}") as unknown;
    if (typeof kept !== "object" || kept === null || Array.isArray(kept)) return {};
    return Object.fromEntries(
      Object.entries(kept).filter(
        (entry): entry is [string, boolean] => typeof entry[1] === "boolean",
      ),
    );
  } catch {
    return {};
  }
}

/** Keeps the columns of `view` chosen by `owner`. */
export function keepColumns(owner: string, view: string, visibility: ColumnVisibility): void {
  try {
    storage()?.setItem(columnsKey(owner, view), JSON.stringify(visibility));
  } catch {
    // A storage full or refused: the choice holds for the page.
  }
}

/**
 * The columns chosen in the view `view` by the signed-in user, read again when the view or
 * the user changes, kept at each choice.
 */
export function useViewColumns(view: () => string): WritableComputedRef<ColumnVisibility> {
  const session = useSession();
  const owner = () => session.value.me?.identity ?? null;
  const chosen = ref<ColumnVisibility>({});
  watch(
    [owner, view],
    ([who, where]) => (chosen.value = who === null ? {} : readColumns(who, where)),
    { immediate: true },
  );
  return computed({
    get: () => chosen.value,
    set: (next) => {
      chosen.value = next;
      const who = owner();
      if (who !== null) keepColumns(who, view(), next);
    },
  });
}
