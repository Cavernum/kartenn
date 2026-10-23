// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/ui`: the design system of kartenn (spec 020) — tokens, cascade layers, fonts,
 * icons and components. The shell and first-party plugins share one instance through
 * the import map (ADR 0038); the sheet is `@kartenn/ui/style.css`.
 */

import "./style.css";

/** Version of the library. */
export const UI_VERSION = "0.1.0";

export { tokens, STATES, ENV_HUES } from "./tokens/index.ts";
export type { Token, State } from "./tokens/index.ts";
