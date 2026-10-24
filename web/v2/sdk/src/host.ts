// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/sdk/host`: the hooks the shell plugs into the SDK. Outside the semver
 * contract: plugins never import it.
 */

import { hooks } from "./hooks.ts";
import { drafts, stream } from "./instance.ts";

export { startData } from "./instance.ts";

/** Opens the event stream again after a new login (a 401 left it closed). */
export function restartEvents(): void {
  stream.restart();
}

export { SDK_VERSION } from "./index.ts";
export { resetContext, setContext } from "./context.ts";
export type { ContextUpdate } from "./context.ts";
export { abandonPlugin, expectPlugin } from "./extensions.ts";
export type { PluginManifest } from "./extensions.ts";
export { addCatalog, setTranslator } from "./i18n.ts";
export { draftHost } from "./draft.ts";
export { compare, parseVersion, satisfies, unmetRange } from "./semver.ts";
export type { Ranges, SharedVersions } from "./semver.ts";
export { commandRegistry, reserveKeys, shortcutRegistry } from "./commands.ts";
export type { Registry } from "./commands.ts";

/** The title of the drafts a plugin's `draft.add` creates, in the shell's language. */
export function setDraftTitle(title: (() => string) | null): void {
  drafts.setTitle(title);
}

/** What a 401 of the client does before it throws: the shell offers to log in again. */
export function onUnauthorized(handler: (() => void | Promise<void>) | null): void {
  hooks.onUnauthorized = handler;
}

/** The `fetch` of the client of the SDK; `null` for the global one. */
export function setFetch(fetch: typeof globalThis.fetch | null): void {
  hooks.fetch = fetch;
}
