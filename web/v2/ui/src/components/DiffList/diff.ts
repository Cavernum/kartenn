// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/** The nature of a change: always shown by its sign and its word, never colour alone. */
export type DiffKind = "added" | "removed" | "changed" | "anomaly";

/** One attribute or property of an object, before and after. */
export interface DiffChange {
  attribute: string;
  kind: DiffKind;
  before?: string;
  after?: string;
  /** Values in fixed pitch (addresses, prefixes, keys). */
  mono?: boolean;
}

/** An object of the differential and its changes (design « Inventaire », line 29). */
export interface DiffGroup {
  id: string;
  title: string;
  /** The key of the object, in fixed pitch beside its title. */
  objectKey?: string;
  kind: DiffKind;
  changes: DiffChange[];
}

/** The sign of each kind. */
export const DIFF_SIGNS: Record<DiffKind, string> = {
  added: "+",
  removed: "−",
  changed: "~",
  anomaly: "!",
};
