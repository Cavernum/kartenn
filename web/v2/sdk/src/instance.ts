// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The single instance of the data layer (ADR 0037): one client, one cache, one index of
// dependents, shared by the shell and every plugin through the import map. `index.ts`
// exports its public part, `host.ts` what the shell plugs in.

import { createClient } from "./api/client.ts";
import { context } from "./context.ts";
import { createQueries, createQueryClient } from "./data/cache.ts";
import { createInvalidation } from "./data/invalidate.ts";
import { createWrites } from "./data/write.ts";
import { hooks } from "./hooks.ts";

/** The client of the API, on the same origin, with the dated context of the shell. */
export const api = createClient({
  fetch: (input, init) => (hooks.fetch ?? globalThis.fetch)(input, init),
  time: () => context.get().time,
  onUnauthorized: () => hooks.onUnauthorized?.(),
});

/** The cache shared by the shell and every plugin. */
export const queryClient = createQueryClient();

/** The reads of the cache: their keys, query functions and freshness (REQ-021-21). */
export const reads = createQueries({ client: api, queryClient });

/** What the writes and the events invalidate. */
export const invalidation = createInvalidation(queryClient, reads);

export const writes = createWrites({ client: api, queryClient, invalidation });
