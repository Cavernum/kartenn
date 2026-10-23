// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/sdk`: the public API of kartenn web plugins (semver contract).
 *
 * The shell serves a single instance of this module through its import map; plugins mark
 * it `external` at build time and share that instance.
 */

import { createClient } from "./api/client.ts";
import { context } from "./context.ts";
import { hooks } from "./hooks.ts";

/** Version of the SDK, checked against the `sdk` range of each plugin manifest. */
export const SDK_VERSION = "1.0.0";

export { createClient } from "./api/client.ts";
export type {
  Answer,
  Client,
  ClientOptions,
  DeleteOptions,
  GetOptions,
  Page,
  PatchOptions,
  PathsOf,
  PostOptions,
  PutOptions,
} from "./api/client.ts";
export { ApiProblem } from "./api/problem.ts";
export type { Violation } from "./api/problem.ts";
export type { components, paths, queries } from "./api/schema";
export { context } from "./context.ts";
export type { Context, Locale, ObjectRef, ObjectType, TimeContext, User } from "./context.ts";
export { definePlugin } from "./extensions.ts";
export type { Extension, ExtensionPoint, PluginDefinition, PluginHandle } from "./extensions.ts";
export { draft } from "./draft.ts";
export type { Operation } from "./draft.ts";

/** The client of the API, on the same origin, with the dated context of the shell. */
export const api = createClient({
  fetch: (input, init) => (hooks.fetch ?? globalThis.fetch)(input, init),
  time: () => context.get().time,
  onUnauthorized: () => hooks.onUnauthorized?.(),
});
