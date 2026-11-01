import { shallowRef, watch, type Ref } from "vue";
import type { CollectionId } from "../collections.ts";

// The edit panel of a kind (REQ-021-36; design 021 « Panneau d'édition par sorte »): a view
// asks for it — the list (`c`, `e`), the record (« Modifier », « Changer de parent… »), the
// « + » of a node of the topography, the palette — and the frame shows it in the Détail tab
// of the right panel.
// The same panel, the same fields, wherever it is asked from.

export type EditRequest =
  | {
      mode: "create";
      collection: CollectionId;
      /** The kinds the user may choose between (the « + » of a node: address, range, prefix). */
      kinds?: readonly CollectionId[];
      /** Values given beforehand (the domain of the tree). */
      preset?: Readonly<Record<string, string>>;
      /** Where the object goes, said in the title (« Ajouter dans 192.0.2.64/26 »). */
      within?: string;
    }
  | { mode: "edit"; collection: CollectionId; id: string }
  /** « Changer de parent… » of an entity or a site (REQ-021-32): the valid targets only. */
  | { mode: "move"; collection: CollectionId; id: string };

/** A request shown: its serial (a new request starts a new form), who asked for it. */
export type ShownRequest = EditRequest & { serial: number; opener: HTMLElement | null };

const current = shallowRef<ShownRequest | null>(null);
let serial = 0;

/** Opens the edit panel on `request`; the focus will go back to what had it. */
export function openEdit(request: EditRequest): void {
  const active = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  // An entry of a menu goes with its menu: the focus goes back to the button that opened it.
  const menu = active?.closest("[role='menu']");
  const button = menu?.getAttribute("aria-labelledby");
  const opener = (button ? document.getElementById(button) : null) ?? active;
  current.value = { ...request, serial: ++serial, opener };
}

/**
 * Closes the edit panel; `back`: the focus goes back to its opener, else (gone: the palette,
 * a row read again) to the work surface.
 */
export function closeEdit(back = true): void {
  const opener = current.value?.opener ?? null;
  current.value = null;
  if (!back) return;
  if (opener?.isConnected) opener.focus();
  else document.getElementById("k-surface")?.focus();
}

/**
 * In the frame's `setup`: another page, or a date off the present (nothing is written
 * there, REQ-021-5), closes the edit the page left had asked for.
 */
export function closeEditWithPage(path: () => string, present: () => boolean): void {
  watch([path, present], ([now, isPresent], [before]) => {
    if (current.value && (now !== before || !isPresent)) closeEdit(false);
  });
}

/** The request the panel shows now, `null` when none. */
export function editRequest(): Readonly<Ref<ShownRequest | null>> {
  return current;
}
