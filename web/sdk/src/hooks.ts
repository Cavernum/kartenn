// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * What the shell plugs into the single instance of the SDK, through `@kartenn/sdk/host`.
 * Internal: plugins never see it.
 */
export const hooks: {
  /** The dated context of the reads. */
  time: () => { validAt: string | null; asOf: string | null };
  /** A 401: the shell offers to log in again (design « Session »). */
  onUnauthorized: (() => void | Promise<void>) | null;
} = {
  time: () => ({ validAt: null, asOf: null }),
  onUnauthorized: null,
};
