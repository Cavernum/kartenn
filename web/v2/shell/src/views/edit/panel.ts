import type { Draft, Entry } from "@kartenn/sdk";
import { shallowRef, watch, type Ref } from "vue";
import type { Period } from "../../time/index.ts";
import type { CollectionId } from "../collections.ts";
import type { VlanNetwork } from "./vlan.ts";

// The edit panel of a kind (REQ-021-36; design 021 « Panneau d'édition par sorte »): a view
// asks for it — the list (`c`, `e`), the record (« Modifier », « Changer de parent… »), the
// « + » of a node of the topography, the palette — and the frame shows it in the Détail tab
// of the right panel.
// The same panel, the same fields, wherever it is asked from.

/**
 * « Modifier » of an operation of a draft (task 14l, Q-021-17): the draft as its line was
 * read (its version, for `If-Match`), the index of the operation the form replaces there.
 */
export interface DraftSlot {
  draft: Entry<Draft>;
  index: number;
}

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
      /** The operation of a draft this creation replaces, its values prefilled. */
      replace?: DraftSlot;
    }
  | { mode: "edit"; collection: CollectionId; id: string; replace?: DraftSlot }
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
/** The line of a draft to give the focus back to, once shown again (`lineToReturn`). */
const returning = shallowRef<{ draft: string; index: number } | null>(null);

/** What asks before a new request replaces a modified edit (the frame, Q-021-22). */
let guard: (() => boolean | Promise<boolean>) | null = null;

/**
 * In the frame: `ask` says whether a new request may replace the edit shown — at once when
 * it is intact, after « Abandonner les modifications ? » when it is modified (« Continuer
 * la saisie »: the new request is dropped). `null` when the frame goes.
 */
export function guardEdit(ask: (() => boolean | Promise<boolean>) | null): void {
  guard = ask;
}

/**
 * Opens the edit panel on `request`; the focus will go back to what had it. A modified
 * edit is asked about first (`guardEdit`).
 */
export function openEdit(request: EditRequest): void {
  const active = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  // An entry of a menu goes with its menu: the focus goes back to the button that opened it.
  const menu = active?.closest("[role='menu']");
  const button = menu?.getAttribute("aria-labelledby");
  const opener = (button ? document.getElementById(button) : null) ?? active;
  const show = (): void => {
    current.value = { ...request, serial: ++serial, opener };
  };
  const allowed = guard?.() ?? true;
  if (allowed === true) show();
  else if (allowed !== false) void allowed.then((go) => go && show());
}

/**
 * Closes the edit panel; `back`: the focus goes back to its opener, else (gone: the palette,
 * a row read again) to the work surface.
 */
export function closeEdit(back = true): void {
  const request = current.value;
  const opener = request?.opener ?? null;
  current.value = null;
  if (!back) return;
  const slot = request && "replace" in request ? request.replace : undefined;
  // An opener in a tab not in front (the panel keeps its tabs mounted, hidden) is gone too.
  if (opener?.isConnected && opener !== document.body && !opener.closest("[hidden]")) {
    opener.focus();
  }
  // « Modifier » of the draft tab, which the Détail tab took the place of (or a click that
  // left the focus on `body`): the tab shown again, the line takes the focus back
  // (`DraftOperations.vue`).
  else if (slot) returning.value = { draft: slot.draft.data.id, index: slot.index };
  else document.getElementById("k-surface")?.focus();
}

/**
 * The operation of a draft whose edit closed while its line was not shown: the frame shows
 * the draft tab again, the line takes the focus back and clears it (task 14l).
 */
export function lineToReturn(): Ref<{ draft: string; index: number } | null> {
  return returning;
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
