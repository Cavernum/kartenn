// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";
import { generate, readContract } from "../scripts/gen.mjs";
import { PARENTS } from "../src/api/parents.ts";
import { canMove, type MoveEnv, type MoveSubject } from "../src/data/moves.ts";

// The check of a move in a tree with a declared parent (REQ-021-32; design 021
// « Déplacements dans les arbres »): each refusal, and the order they come in.

const entity = (id: string): MoveSubject => ({ id, type: "core.entity" });
const machine = (id: string): MoveSubject => ({ id, type: "machines.machine" });
const cluster = (id: string): MoveSubject => ({ id, type: "machines.cluster" });

/** Ancestors by node, every right, at the present, unless told otherwise. */
function env(
  parents: Record<string, string> = {},
  { lacking = [] as string[], present = true } = {},
): MoveEnv {
  return {
    ancestors(id) {
      const up: string[] = [];
      for (let at = parents[id]; at !== undefined; at = parents[at]) up.push(at);
      return up;
    },
    can: (permission) => !lacking.includes(permission),
    present,
  };
}

describe("admitted parents, drawn from the contract (REQ-021-32)", () => {
  it("names the write of each parent and the types it admits", () => {
    expect(PARENTS["core.entity"]).toEqual({
      via: "move_entity.parent",
      parents: ["core.entity"],
      root: true,
    });
    expect(PARENTS["core.site"]?.parents).toEqual(["core.site"]);
    expect(PARENTS["core.vlan_group"]).toEqual({
      via: "update_vlan_group.anchor",
      parents: ["core.domain", "core.entity", "core.site"],
      root: false,
    });
    expect(PARENTS["machines.machine"]).toEqual({
      via: "MachinePatch.host",
      parents: ["machines.cluster", "machines.machine"],
      root: true,
    });
    // A prefix has no declared parent: its place follows from its value (D34).
    expect(PARENTS["core.prefix"]).toBeUndefined();
  });

  it("follows the contract: a kind of host withdrawn is no longer admitted", async () => {
    const changed = readContract() as {
      components: { schemas: Record<string, { oneOf?: unknown[] }> };
    } & ReturnType<typeof readContract>;
    changed.components.schemas["MachineHost"]!.oneOf!.pop();
    const { parents } = await generate(changed);
    expect(parents).toContain('parents: ["machines.machine"]');
  }, 60_000);

  it("fails the build when the write of a parent is gone from the contract", async () => {
    const changed = readContract() as {
      components: { schemas: { Operation: { oneOf: { properties?: { op?: unknown } }[] } } };
    } & ReturnType<typeof readContract>;
    const operations = changed.components.schemas.Operation;
    operations.oneOf = operations.oneOf.filter(
      (variant) =>
        JSON.stringify(variant.properties?.op) !== '{"enum":["move_site"],"type":"string"}',
    );
    await expect(generate(changed)).rejects.toThrow(/no operation move_site/);
  }, 60_000);
});

describe("canMove (REQ-021-32)", () => {
  it("accepts a target of an admitted type, with both rights, at the present", () => {
    expect(canMove(entity("a"), entity("b"), env())).toEqual({ ok: true });
    expect(canMove(machine("m"), cluster("c"), env())).toEqual({ ok: true });
    // To the first level, for a type that may be a root.
    expect(canMove(entity("a"), null, env())).toEqual({ ok: true });
  });

  it("refuses the node itself and its known descendants: a cycle, with the rule's code", () => {
    // b under a, c under b: neither b nor c may become the parent of a.
    const tree = env({ b: "a", c: "b" });
    const cycle = { ok: false, refusal: "cycle", code: "core.anchor.cycle" };
    expect(canMove(entity("a"), entity("a"), tree)).toEqual(cycle);
    expect(canMove(entity("a"), entity("b"), tree)).toEqual(cycle);
    expect(canMove(entity("a"), entity("c"), tree)).toEqual(cycle);
    // A machine onto a machine it hosts: the rule of the module.
    expect(canMove(machine("h"), machine("g"), env({ g: "h" }))).toEqual({
      ok: false,
      refusal: "cycle",
      code: "machines.host.invalid",
    });
  });

  it("refuses a parent the contract does not admit, and a root where none may be", () => {
    expect(canMove(entity("a"), { id: "s", type: "core.site" }, env())).toEqual({
      ok: false,
      refusal: "incompatible",
    });
    expect(canMove(machine("m"), entity("e"), env())).toEqual({
      ok: false,
      refusal: "incompatible",
      code: "machines.host.invalid",
    });
    expect(canMove({ id: "g", type: "core.vlan_group" }, null, env())).toEqual({
      ok: false,
      refusal: "incompatible",
      code: "core.vlan.group_anchor",
    });
    // A type whose parent is not declared never moves (a prefix, D34).
    expect(canMove({ id: "p", type: "core.prefix" }, null, env()).ok).toBe(false);
  });

  it("refuses without the write right on the node, or on the target", () => {
    expect(canMove(entity("a"), entity("b"), env({}, { lacking: ["core.entity.edit"] }))).toEqual({
      ok: false,
      refusal: "forbidden",
      permission: "core.entity.edit",
    });
    expect(
      canMove(machine("m"), cluster("c"), env({}, { lacking: ["machines.cluster.edit"] })),
    ).toEqual({ ok: false, refusal: "forbidden", permission: "machines.cluster.edit" });
    expect(
      canMove(machine("m"), cluster("c"), env({}, { lacking: ["machines.machine.edit"] })),
    ).toMatchObject({ refusal: "forbidden", permission: "machines.machine.edit" });
  });

  it("refuses every move off the present", () => {
    expect(canMove(entity("a"), entity("b"), env({}, { present: false }))).toEqual({
      ok: false,
      refusal: "dated",
    });
  });

  it("gives the first refusal: cycle, then type, then rights, then the date", () => {
    const all = { lacking: ["core.entity.edit", "machines.machine.edit"], present: false };
    // Everything wrong at once: the cycle first.
    expect(canMove(machine("h"), machine("g"), env({ g: "h" }, all))).toMatchObject({
      refusal: "cycle",
    });
    // No cycle: the type.
    expect(canMove(machine("h"), entity("e"), env({}, all))).toMatchObject({
      refusal: "incompatible",
    });
    // An admitted type: the rights, the node's before the target's.
    expect(
      canMove(machine("h"), cluster("c"), env({}, { ...all, lacking: [...all.lacking] })),
    ).toMatchObject({ refusal: "forbidden", permission: "machines.machine.edit" });
    // Every right: the date.
    expect(canMove(machine("h"), cluster("c"), env({}, { present: false }))).toMatchObject({
      refusal: "dated",
    });
  });
});
