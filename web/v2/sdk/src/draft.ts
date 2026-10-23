// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The surface of the draft offered to plugins (W10): operations in the format of the
// contract, added to the draft of the current user and tab. The shell keeps the draft
// (IndexedDB, simulation, submission) and syncs it here through `@kartenn/sdk/host`.

import type { components } from "./api/schema";

/** An operation of a changeset, as the contract defines it. */
export type Operation = components["schemas"]["Operation"];

let operations: readonly Operation[] = Object.freeze([]);
const listeners = new Set<(ops: readonly Operation[]) => void>();
let onAdd: ((op: Operation) => void) | null = null;

function publish(next: readonly Operation[]): void {
  operations = Object.freeze([...next]);
  for (const listener of [...listeners]) listener(operations);
}

export const draft = {
  operations(): readonly Operation[] {
    return operations;
  },
  /** Adds an operation at the end; the shell opens the drawer of the draft. */
  add(op: Operation): void {
    publish([...operations, op]);
    onAdd?.(op);
  },
  remove(index: number): void {
    if (index < 0 || index >= operations.length) throw new RangeError(`no operation ${index}`);
    publish(operations.filter((_, i) => i !== index));
  },
  subscribe(listener: (ops: readonly Operation[]) => void): () => void {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
};

/** The shell's side: the operations it holds, and what an addition does. */
export const draftHost = {
  set(ops: readonly Operation[]): void {
    publish(ops);
  },
  onAdd(handler: ((op: Operation) => void) | null): void {
    onAdd = handler;
  },
};
