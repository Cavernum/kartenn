// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * What the shell, or the test sandbox, plugs into the single instance of the SDK, through
 * `@kartenn/sdk/host`. Internal: plugins never see it.
 */
export const hooks: {
  /** A 401: the shell offers to log in again (design « Session »). */
  onUnauthorized: (() => void | Promise<void>) | null;
  /** The `fetch` of the client; the global one when `null`. */
  fetch: typeof fetch | null;
} = {
  onUnauthorized: null,
  fetch: null,
};
