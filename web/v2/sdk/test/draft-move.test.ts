// @vitest-environment happy-dom
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { afterEach, describe, expect, it } from "vitest";
import { draft, moveInDraft, type Operation } from "../src/draft.ts";
import { resetContext, setContext } from "../src/context.ts";
import { setFetch } from "../src/host.ts";
import { drafts, queryClient } from "../src/instance.ts";
import { fakeFetch } from "../src/testing/index.ts";

// A change of parent put in the active draft and simulated (task 18b; REQ-021-33): the move
// of the same node replaced, the verdict of its own operation only, a simulation not done.

const D = "1b5e4fc3-4071-4f61-9e82-3d2a1f0e9b86";
const M = "4c9f2a52-0f1e-4f55-9a43-6f1f2d1b7a10";
const node = { id: M, type: "machines.machine" };
const to = (cluster: string) =>
  ({
    op: "machines.update_machine",
    id: { id: M },
    host: { cluster: { id: cluster } },
  }) as Operation;
const other = { op: "rename", kind: "entity", id: { id: "e1" }, slug: "taken" } as Operation;

const body = (operations: unknown[], version: number) =>
  new Response(
    JSON.stringify({ id: D, title: "t", version, operation_count: operations.length, operations }),
    { status: 200, headers: { "Content-Type": "application/json", ETag: `"${version}"` } },
  );
const simulated = (valid: boolean, violations: unknown[]) => ({
  valid,
  violations,
  diff: [],
  sample: [],
  cascaded: [],
  sections: [],
});
const violation = (rule: string, severity: string, at: number | null, subjects: string[] = []) => ({
  rule,
  severity,
  detail: rule,
  subjects,
  operation: at,
  pointer: at === null ? null : `/operations/${at}`,
});

/** A server holding the active draft `operations`, simulated as `simulation` answers. */
function server(
  operations: unknown[],
  simulation: (period: unknown) => unknown,
  patched: () => void = () => undefined,
) {
  const seen: string[] = [];
  let version = 1;
  setFetch(
    fakeFetch({
      "GET /v1/drafts/{id}": () => body(operations, version),
      "PATCH /v1/drafts/{id}": ({ body: change }) => {
        seen.push("PATCH");
        patched();
        operations = (change as { operations: unknown[] }).operations;
        return body(operations, ++version);
      },
      "POST /v1/drafts/{id}/operations": ({ body: added }) => {
        seen.push("POST operations");
        operations = [...operations, ...(added as { operations: unknown[] }).operations];
        return body(operations, ++version);
      },
      "POST /v1/drafts/{id}/simulation": ({ body: period }) => simulation(period) as never,
    }),
  );
  drafts.setActive(D);
  return { seen, operations: () => operations };
}

afterEach(() => {
  setFetch(null);
  queryClient.clear();
  sessionStorage.clear();
  resetContext();
});

describe("moveInDraft", () => {
  it("replaces an earlier move of the same node in place, never a second one", async () => {
    const at = server([other, to("c1")], () => simulated(true, []));
    const outcome = await moveInDraft(to("c2"), node, drafts.addToActive);
    expect(outcome).toMatchObject({ kind: "drafted", verdict: "valid", warnings: [] });
    expect(at.seen).toEqual(["PATCH"]);
    expect(at.operations()).toEqual([other, to("c2")]);
  });

  it("adds a first move; a violation of another operation does not refuse it", async () => {
    const at = server([other], () =>
      simulated(false, [
        violation("core.slug.duplicate", "error", 0, ["e1"]),
        violation("machines.mac.duplicate", "warning", 0),
        violation("machines.host.site_mismatch", "warning", null, [M]),
      ]),
    );
    const outcome = await moveInDraft(to("c1"), node, drafts.addToActive);
    expect(at.seen).toEqual(["POST operations"]);
    // Valid for the node; only the warnings about it, not another operation's.
    expect(outcome).toMatchObject({ kind: "drafted", verdict: "valid" });
    expect(outcome.kind === "drafted" && outcome.verdict === "valid" && outcome.warnings).toEqual([
      violation("machines.host.site_mismatch", "warning", null, [M]),
    ]);
  });

  it("refused by a rule of its own operation, the operation kept in the draft", async () => {
    const at = server([other], () =>
      simulated(false, [
        violation("core.slug.duplicate", "error", 0, ["e1"]),
        violation("machines.host.invalid", "error", 1, [M]),
      ]),
    );
    const outcome = await moveInDraft(to("c1"), node, drafts.addToActive);
    expect(outcome).toMatchObject({
      kind: "drafted",
      verdict: "refused",
      violation: { rule: "machines.host.invalid" },
    });
    expect(at.operations()).toEqual([other, to("c1")]);
  });

  it("a simulation that fails is no verdict: `unknown` with its problem", async () => {
    server([], () => new Response("{}", { status: 503, headers: { "Retry-After": "1" } }));
    const outcome = await moveInDraft(to("c1"), node, drafts.addToActive);
    expect(outcome).toMatchObject({ kind: "drafted", verdict: "unknown" });
    expect(outcome.kind === "drafted" && outcome.verdict === "unknown" && outcome.problem).not.toBe(
      null,
    );
  });

  it("under valid_at alone, simulated at the date of the addition, kept with the draft (REQ-021-5)", async () => {
    const at = "2031-03-01T08:00:00Z";
    const periods: unknown[] = [];
    server([other], (period) => {
      periods.push(period);
      return simulated(true, []);
    });
    setContext({ time: { validAt: at, asOf: null } });
    await moveInDraft(to("c1"), node, drafts.addToActive);
    expect(periods).toEqual([{ valid_from: at }]);
    expect(drafts.period(D)).toEqual({ valid_from: at });
    // Back at the present, replaced in place: still simulated at the date of its addition.
    setContext({ time: { validAt: null, asOf: null } });
    queryClient.clear();
    await moveInDraft(to("c2"), node, drafts.addToActive);
    expect(periods).toEqual([{ valid_from: at }, { valid_from: at }]);
  });

  it("replaced in place, dated at the date read before it is sent", async () => {
    const at = "2031-03-01T08:00:00Z";
    const periods: unknown[] = [];
    const later = () => setContext({ time: { validAt: "2032-01-01T00:00:00Z", asOf: null } });
    server(
      [other, to("c1")],
      (period) => {
        periods.push(period);
        return simulated(true, []);
      },
      later,
    );
    setContext({ time: { validAt: at, asOf: null } });
    await moveInDraft(to("c2"), node, drafts.addToActive);
    expect(drafts.period(D)).toEqual({ valid_from: at });
    expect(periods).toEqual([{ valid_from: at }]);
  });
});

describe("draft.onActive", () => {
  it("tells each change of the active draft, none included, until stopped", () => {
    const seen: (string | null)[] = [];
    const stop = draft.onActive((id) => seen.push(id));
    drafts.setActive(D);
    drafts.setActive(D);
    drafts.setActive(null);
    stop();
    drafts.setActive(D);
    expect(seen).toEqual([D, null]);
  });
});
