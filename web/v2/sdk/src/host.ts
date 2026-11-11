// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/sdk/host`: the hooks the shell plugs into the SDK. Outside the semver
 * contract: plugins never import it.
 */

import type { MoveSubject } from "./data/moves.ts";
import { moveInDraft, type DraftMove } from "./draft.ts";
import { hooks } from "./hooks.ts";
import { drafts, stream, writes } from "./instance.ts";

export { startData } from "./instance.ts";

/**
 * The decisions on a changeset of the shell's review screens (REQ-021-10): validation,
 * rejection, withdrawal, undo, under `If-Match` of the changeset read. Not offered to the
 * plugins (outside the semver contract).
 */
export const review = { decide: writes.decide };
export type { Decision, DecisionOutcome } from "./data/write.ts";

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

/**
 * The shell's own additions to the active draft (in-place edits, the edit panel): as a
 * plugin's `draft.add`, without its promise to open the draft — the shell says them where
 * they were made.
 */
export function addToActiveDraft(
  operations: Parameters<typeof drafts.addToActive>[0],
): ReturnType<typeof drafts.addToActive> {
  return drafts.addToActive(operations);
}

/**
 * « Modifier » of an operation of a draft (task 14l, REQ-021-11): the edit panel puts its
 * operation in place of the one of index `index` of the draft as `read` (the version its
 * line was shown at), under `If-Match`; under `valid_at` alone, the date read at the call
 * dates the draft as an addition does. Not offered to the plugins.
 */
export const replaceInDraft = drafts.replaceOperation;

/**
 * The shell's own change of parent in the active draft, then its simulation: as a plugin's
 * `draft.move`, without its promise to open the draft (REQ-021-33).
 */
export function moveInActiveDraft(
  op: Extract<Parameters<typeof drafts.addToActive>[0], readonly unknown[]>[number],
  node: MoveSubject,
): Promise<DraftMove> {
  return moveInDraft(op, node, drafts.addToActive);
}

/**
 * The period of a draft (its dates of effect), kept with it for the tab, a reload included:
 * the date of its first addition under `valid_at` alone, or the foot's choice. Read by the
 * shell's foot for its simulation and its submission; not offered to the plugins.
 */
export const draftPeriod = {
  get: drafts.period,
  set: drafts.setPeriod,
  on: drafts.onPeriod,
};

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
