// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/sdk`: the public API of kartenn web plugins (semver contract).
 *
 * The shell serves a single instance of this module through its import map; plugins mark
 * it `external` at build time and share that instance.
 */

import type { KartennEvent, StreamState } from "./events/stream.ts";
import { commandRegistry, shortcutRegistry } from "./commands.ts";
import { createPrefetch } from "./data/prefetch.ts";
import { drafts as store, labels, queryClient, stream, writes } from "./instance.ts";

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
export { keys, normalize, timeKey } from "./data/keys.ts";
export type {
  FilterValue,
  Filters,
  Key,
  NormalFilters,
  ObjectKey,
  PathKey,
  PathParams,
  SubKey,
  TimeArg,
  TimeKey,
} from "./data/keys.ts";
export { FRESHNESS, IMMUTABLE, MAX_PAGES } from "./data/cache.ts";
export type {
  Entry,
  Family,
  FiltersOf,
  ItemOf,
  ListData,
  ListOptions,
  ListPath,
  ReadOptions,
} from "./data/cache.ts";
export { collectionOf, qualify, registerType } from "./data/types.ts";
export { filterModel, filterSearch, parseFilters } from "./data/filters.ts";
export type { Declaration, FilterField, Rejected } from "./data/filters.ts";
export type { QueryClient } from "@tanstack/query-core";
export { mergePatch } from "./data/write.ts";
export { lookup } from "./data/labels.ts";
export type { Dictionary } from "./data/labels.ts";
export type {
  PendingReason,
  Proposed,
  QuickWrite,
  Refusal,
  Simulated,
  Submission,
  WriteOutcome,
} from "./data/write.ts";
export { SIMULATION_WAIT } from "./data/drafts.ts";
export type {
  Draft,
  DraftChange,
  DraftCreation,
  DraftList,
  DraftOutcome,
  DraftPeriod,
  DraftSubmitOutcome,
  Gone,
  SubmitOptions,
} from "./data/drafts.ts";

/**
 * The client of the API, the cache shared by the shell and every plugin (ADR 0037), and
 * its reads: keys, query functions, freshness (REQ-021-21).
 */
export { api, queryClient, reads } from "./instance.ts";

/**
 * The drafts of the caller on the server (spec 024 § A): their queries, their writes
 * under `If-Match`, the simulation waited 400 ms, the submission (`approvalSelf`), and the
 * active draft of the tab.
 */
export const drafts = {
  list: store.list,
  get: store.get,
  create: store.create,
  update: store.update,
  addOperations: store.addOperations,
  remove: store.remove,
  simulate: store.simulate,
  submit: store.submit,
  active: store.active,
  setActive: store.setActive,
  onActive: store.onActive,
  onGone: store.onGone,
};

/** The counters of the home page (`GET /v1/stats`), a query read again on `stats`. */
export { stats } from "./instance.ts";

/** The quick edit of an object, optimistic at the present (REQ-021-16, 24; D-7). */
export const write = { quick: writes.quick };

/** The changeset of a draft: its simulation as a query, its submission (W10). */
export const changesets = { simulate: writes.simulate, submit: writes.submit };

/**
 * The label of a key in a space (`kind`, `value`, `role`, `permission`, `state`…; a value
 * of a closed list is `<property>/<value>`): the dictionary of the language, the catalogs,
 * else the key (REQ-021-19).
 */
export const label = labels.label;

/** The query of the label dictionary of a language, for a reactive adapter. */
export const labelDictionary = labels.query;

/**
 * The events of the stream shared by the tabs of the browser (REQ-021-23), once applied to
 * the cache: a plugin listens to them, it needs not invalidate anything itself.
 */
export const events = {
  subscribe(listener: (event: KartennEvent) => void): () => void {
    return stream.subscribe(listener);
  },
  state(): StreamState {
    return stream.state();
  },
  onState(listener: (state: StreamState) => void): () => void {
    return stream.onState(listener);
  },
};
export type { EventKind, KartennEvent, StreamState } from "./events/stream.ts";

/** The commands a plugin adds to the palette (REQ-021-3); `register` returns the removal. */
export const commands = { register: commandRegistry.register };

/**
 * The shortcuts of a plugin's view (REQ-021-7): one key each, refused if the shell or
 * another plugin holds it; the view removes them when it leaves (`register` returns the
 * removal).
 */
export const shortcuts = { register: shortcutRegistry.register };
export type { Command, Shortcut } from "./commands.ts";

/** Prefetch on intent and the guard of navigation (REQ-021-22). */
export const prefetch = createPrefetch({ queryClient });
export { GUARD_WAIT, INTENT_DELAY } from "./data/prefetch.ts";
export type { Prefetchable } from "./data/prefetch.ts";
