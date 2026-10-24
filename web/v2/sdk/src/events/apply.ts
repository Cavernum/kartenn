// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// What each event does to the cache (022 « Contrat consommé par les clients », amended by
// the 021; 018 for `labels`).

import type { Changed, Invalidation } from "../data/invalidate.ts";
import type { KartennEvent } from "./stream.ts";

export interface ApplyDeps {
  invalidation: Invalidation;
  /** The label dictionary: a new revision (spec 018). */
  labels: { changed(revision: string): unknown };
}

function list(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

export async function applyEvent(event: KartennEvent, { invalidation, labels }: ApplyDeps) {
  const data = event.data;
  switch (event.kind) {
    // What was read before `ready` may be stale; `reset` replays nothing (022).
    case "ready":
    case "reset":
      await invalidation.all();
      return;
    case "changes": {
      if (data["truncated"] === true) {
        await invalidation.types(
          list(data["types"]).filter((t): t is string => typeof t === "string"),
        );
        return;
      }
      const objects = list(data["objects"]).filter(
        (o): o is Changed =>
          typeof (o as Changed)?.type === "string" && typeof (o as Changed)?.id === "string",
      );
      // A task of a module has no revision: read again without condition.
      const revision = typeof data["changeset"] === "string" ? data["changeset"] : null;
      await invalidation.objects(objects, revision);
      return;
    }
    case "changeset": {
      const id = data["changeset"];
      await Promise.all([
        typeof id === "string" ? invalidation.objects([{ type: "changeset", id }], null) : null,
        invalidation.path("/v1/proposals"),
      ]);
      return;
    }
    case "labels":
      if (typeof data["revision"] === "string") await labels.changed(data["revision"]);
      return;
  }
}
