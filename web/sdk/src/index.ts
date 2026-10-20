// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/sdk`: the public API of kartenn web plugins (semver contract).
 *
 * The shell serves a single instance of this module through its import map; plugins mark
 * it `external` at build time and share that instance.
 */

/** Version of the SDK, checked against the `sdk` range of each plugin manifest. */
export const SDK_VERSION = "0.1.0";
