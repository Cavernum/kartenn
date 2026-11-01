// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The names the hosts of the component benches (`bench.ts`) share with the Playwright
// bench (web/v2/e2e/bench/components.spec.ts imports them): nothing else here, so that
// the bench loads them without Vue.

/**
 * The event the host of a tree sends on `window` once its answer to a move is ready (the
 * new model built), before it gives it to the tree: the bench times the rendering of the
 * answer from it, the host's own work left out.
 */
export const ANSWER_EVENT = "k-bench-answer";

/**
 * The name of the `performance.measure` of a change of `moveState` alone, from the change
 * to the end of the flush of Vue that follows: `refusedKeys` over every row, the window
 * drawn again.
 */
export const STATE_MEASURE = "k-bench-move-state";

/** The rank of a child among its domain's (`v3-105`: 105), none (NaN) for a domain. */
export const rank = (id: string | undefined) => Number(id?.split("-")[1] ?? Number.NaN);

/** A target the check of the tree refuses (`canMove`, with its reason): rank 7k. */
export const lockedTarget = (id: string | undefined) => rank(id) % 7 === 0;

/** A target the « server » refuses (`moveState` refused, with its code): rank 3k. */
export const fullTarget = (id: string | undefined) => rank(id) % 3 === 0;
