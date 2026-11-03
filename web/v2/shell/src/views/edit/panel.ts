import { shallowRef, watch, type Ref } from "vue";
import type { Period } from "../../time/index.ts";
import type { CollectionId } from "../collections.ts";
import type { VlanNetwork } from "./vlan.ts";

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
      /**
       * A VLAN created for a network (« Créer un VLAN pour ce réseau », REQ-026-6): set on it
       * by `core.vlan`, in the same changeset.
       */
      network?: VlanNetwork;
    }
  | { mode: "edit"; collection: CollectionId; id: string }
  /** « Changer de parent… » of an entity or a site (REQ-021-32): the valid targets only. */
  | { mode: "move"; collection: CollectionId; id: string }
  /** « Clore… » of a record (021/A31): its closure, to the draft or applied (`ClosePanel`). */
  | { mode: "close"; collection: CollectionId; id: string };

/** A request shown: its serial (a new request starts a new form), who asked for it. */
export type ShownRequest = EditRequest & { serial: number; opener: HTMLElement | null };
/** A request of the edit panel's form: a creation, a modification, a move. */
export type FormRequest = Exclude<ShownRequest, { mode: "close" }>;
/** A request of a closure. */
export type CloseRequest = Extract<ShownRequest, { mode: "close" }>;

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
 * In the frame's `setup`: another page, or another date off the present (nothing is written
 * under `as_of`, only the dated draft under `valid_at`, REQ-021-5: what was asked at the
 * date before no longer holds), closes the edit the page left had asked for.
 */
export function closeEditWithPage(path: () => string, period: () => Period): void {
  watch([path, () => period().validAt, () => period().asOf], ([now, validAt, asOf], [before]) => {
    if (current.value && (now !== before || validAt !== null || asOf !== null)) {
      closeEdit(false);
    }
  });
}

/** The request the panel shows now, `null` when none. */
export function editRequest(): Readonly<Ref<ShownRequest | null>> {
  return current;
}
