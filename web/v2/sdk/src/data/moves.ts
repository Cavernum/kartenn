// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// Whether a node of a tree with a declared parent may go under a target (REQ-021-32; design
// 021 « Déplacements dans les arbres », 020 « Contrat de déplacement »): synchronous, for
// each target hovered or listed, the first refusal in a fixed order — a cycle, a parent the
// contract does not admit for the type (`api/parents.ts`, generated), a right missing on
// either end, a view off the present. The server stays the only judge: a target accepted
// here may still be refused by its rules (REQ-021-33).

import { PARENTS, type ParentRule } from "../api/parents.ts";

/** A node of a tree as a move sees it: its identifier and its qualified type. */
export interface MoveSubject {
  readonly id: string;
  readonly type: string;
  /**
   * Its kind, when its type has kinds that may not be parents (a machine `container` or
   * `appliance` hosts nothing: 009/M5); unknown, the server judges.
   */
  readonly kind?: string;
}

/** Why a target is refused, in the order the checks run. */
export type MoveRefusal = "cycle" | "incompatible" | "forbidden" | "dated";

/** The verdict on a target: the refusal, the code of the server rule it stands for. */
export type MoveVerdict =
  | { readonly ok: true }
  | {
      readonly ok: false;
      readonly refusal: MoveRefusal;
      /** The rule of the server that refuses the same (`core.anchor.cycle`), if one. */
      readonly code?: string;
      /** The permission missing (`forbidden`). */
      readonly permission?: string;
    };

/** What a check needs to know beside the two nodes. */
export interface MoveEnv {
  /** The ancestors of a node known so far (the cache), from its parent up. */
  ancestors(id: string): readonly string[];
  /** Whether the caller holds a permission (`/v1/me`). */
  can(permission: string): boolean;
  /** Whether the view is at the present: nothing moves under `valid_at` or `as_of`. */
  present: boolean;
}

/**
 * The rule of the server each refusal stands for, by type of the node moved (the rules of
 * the core and of the modules: `core.anchor.cycle`, `machines.host.invalid`…); a refusal
 * without a rule of its own (a right, a date) has no code.
 */
export const MOVE_CODES: Readonly<Record<string, Partial<Record<MoveRefusal, string>>>> = {
  "core.entity": { cycle: "core.anchor.cycle" },
  "core.site": { cycle: "core.anchor.cycle" },
  "core.vlan_group": { incompatible: "core.vlan.group_anchor" },
  "machines.machine": { cycle: "machines.host.invalid", incompatible: "machines.host.invalid" },
};

/**
 * The kinds of a type that may be a parent at all, by type of the target (009/M5: a
 * physical machine hosts a virtual one or a container, a virtual one a container; a
 * container or an appliance hosts nothing). Whether the kind of the node suits the kind of
 * the target stays the server's (`machines.host.invalid`).
 */
export const PARENT_KINDS: Readonly<Record<string, readonly string[]>> = {
  "machines.machine": ["physical", "virtual"],
};

/**
 * The permission that writes an object of a type (verbs of the 024): `<type>.edit`, the
 * resource left out when it bears the name of its module (018 R2: `machines.machine` →
 * `machines.edit`, `flow.flow` → `flow.edit`).
 */
function writeRight(type: string): string {
  const [module, resource] = type.split(".");
  return resource === module || `${resource}s` === module ? `${module}.edit` : `${type}.edit`;
}

/** How a type changes parent, from the contract; `undefined`: its parent is not declared. */
export function parentRule(type: string): ParentRule | undefined {
  return PARENTS[type];
}

/**
 * Whether `node` may go under `target` (`null`: to the first level, a root): the first
 * refusal of, in order, a cycle (the target is the node or one of its known descendants),
 * a parent of a type the contract does not admit, the write right missing on the node or
 * on the target, a view off the present.
 */
export function canMove(node: MoveSubject, target: MoveSubject | null, env: MoveEnv): MoveVerdict {
  const refuse = (refusal: MoveRefusal, permission?: string): MoveVerdict => {
    const code = MOVE_CODES[node.type]?.[refusal];
    return {
      ok: false,
      refusal,
      ...(code ? { code } : {}),
      ...(permission ? { permission } : {}),
    };
  };
  if (target && (target.id === node.id || env.ancestors(target.id).includes(node.id))) {
    return refuse("cycle");
  }
  const rule = parentRule(node.type);
  if (!rule || (target ? !rule.parents.includes(target.type) : !rule.root)) {
    return refuse("incompatible");
  }
  const kinds = target ? PARENT_KINDS[target.type] : undefined;
  if (kinds && target?.kind !== undefined && !kinds.includes(target.kind)) {
    return refuse("incompatible");
  }
  for (const type of target ? [node.type, target.type] : [node.type]) {
    if (!env.can(writeRight(type))) return refuse("forbidden", writeRight(type));
  }
  if (!env.present) return refuse("dated");
  return { ok: true };
}
