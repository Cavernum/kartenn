import { shallowRef, type Ref } from "vue";
import type { CollectionId } from "../collections.ts";

// The edit panel of a kind (REQ-021-36; design 021 « Panneau d'édition par sorte »): a view
// asks for it — the list (`c`, `e`), the record (« Modifier »), the « + » of a node of the
// topography, the palette — and the frame shows it in the Détail tab of the right panel.
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
  | { mode: "edit"; collection: CollectionId; id: string };

/** A request shown: its serial (a new request starts a new form), who asked for it. */
export type ShownRequest = EditRequest & { serial: number; opener: HTMLElement | null };

const current = shallowRef<ShownRequest | null>(null);
let serial = 0;

/** Opens the edit panel on `request`; the focus will go back to what had it. */
export function openEdit(request: EditRequest): void {
  const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
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

/** The request the panel shows now, `null` when none. */
export function editRequest(): Readonly<Ref<ShownRequest | null>> {
  return current;
}
