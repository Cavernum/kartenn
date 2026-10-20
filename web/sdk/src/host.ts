// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/sdk/host`: the hooks the shell plugs into the SDK. Outside the semver
 * contract: plugins never import it.
 */

import { hooks } from "./hooks.ts";

export { SDK_VERSION } from "./index.ts";

/** What a 401 of the client does before it throws: the shell offers to log in again. */
export function onUnauthorized(handler: (() => void | Promise<void>) | null): void {
  hooks.onUnauthorized = handler;
}
