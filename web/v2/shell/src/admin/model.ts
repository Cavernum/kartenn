import type { components, Operation, Violation } from "@kartenn/sdk";
import type { Rights } from "../rights/index.ts";
import type { WriteRefusal } from "../time/index.ts";

// The administration of the rights by groups (spec 025, REQ-025-12; REQ-021-12; design 021
// « Administration »; maquette E7): the rows of a group and of a role as the active draft
// marks them — a removal stays in place, an addition comes at the end, each with the index
// of its operation, which its action takes out of the draft —, the operations each action
// adds, and why an action is refused: the permission lacking, the gate of writes (the past,
// a frame under 1024 px: `useAdminGate`), a group delivered, a scope wider than the user's
// as the simulation of the server says it (the server stays the judge; the client never
// guesses a scope). The gate only adds refusals: open, the actions are those of the rights.

export type GroupItem = components["schemas"]["GroupItem"];
export type MemberItem = components["schemas"]["MemberItem"];
export type BindingItem = components["schemas"]["BindingItem"];
export type RoleItem = components["schemas"]["RoleItem"];
export type IdentityItem = components["schemas"]["IdentityItem"];
export type GroupMember = components["schemas"]["GroupMember"];
/** The scope of a binding as served: `{kind, id}`, no `id` when global. */
export type BindingScope = components["schemas"]["Scope"];
/** The scope of a binding as an operation names it: its object by reference. */
export type ScopeRef = components["schemas"]["BindingScope"];

/** The scope an operation names, as served (a local reference as `#n`). */
export function servedScope(scope: ScopeRef): BindingScope {
  if (scope.kind === "global") return { kind: "global" };
  const ref = scope.id as { id?: string; local?: number };
  return { kind: scope.kind, id: ref.id ?? `#${ref.local ?? "?"}` };
}

/** The scope as an operation takes it. */
export function scopeRef(scope: BindingScope): ScopeRef {
  return scope.kind === "global" || !scope.id
    ? { kind: "global" }
    : ({ kind: scope.kind, id: { id: scope.id } } as ScopeRef);
}

/** The permission of every write of the groups, their members and their bindings. */
export const RBAC_ADMIN = "core.rbac.admin";
/** The permission of the writes of the roles. */
export const ROLE_ADMIN = "core.role.admin";
/** The codes the refusals of the administration show (rules of 005 and 025). */
export const GROUP_ESCALATION = "core.group.escalation";
export const BINDING_ESCALATION = "core.binding.escalation";
export const GROUP_BUILTIN = "core.group.builtin";
/** An administration the author may not do on that scope (005). */
export const RBAC_FORBIDDEN = "core.rbac.forbidden";
export const ROLE_BUILTIN = "core.role.builtin";

/** The kinds of scope of a binding (005), in the order of the choice. */
export const SCOPE_KINDS = ["global", "entity", "site", "domain", "prefix"] as const;
export type ScopeKind = (typeof SCOPE_KINDS)[number];

/** A row waiting in the draft: removed (still shown, struck) or added (at the end). */
export type Pending = "removed" | "added";

/** A member of a group as its list shows it. */
export interface MemberRow {
  key: string;
  kind: "identity" | "idp_group";
  identity: string | null;
  issuer: string | null;
  group: string | null;
  name: string | null;
  until: string | null;
  /** The membership, for a current one. */
  id: string | null;
  pending?: Pending;
  /** The index of its operation in the active draft. */
  op?: number;
}

/** A binding of a group (a role on a scope) as its list shows it. */
export interface BindingRow {
  key: string;
  id: string | null;
  role: string;
  scope: BindingScope;
  until: string | null;
  pending?: Pending;
  op?: number;
}

/** A permission of a role as its list shows it: its served name. */
export interface EntryRow {
  entry: string;
  pending?: Pending;
}

type Op = Record<string, unknown>;
const asOp = (op: Operation): Op => op as unknown as Op;

/** The text of a member: an identity by its name, a group of an IdP `émetteur / groupe`. */
export function memberName(
  row: Pick<MemberRow, "kind" | "name" | "identity" | "issuer" | "group">,
) {
  return row.kind === "idp_group"
    ? `${row.issuer ?? "?"} / ${row.group ?? "?"}`
    : (row.name ?? row.identity ?? "?");
}

/** The members of a group, marked by the operations of the active draft. */
export function memberRows(
  members: readonly MemberItem[],
  operations: readonly Operation[],
  group: string,
  identities: ReadonlyMap<string, IdentityItem> = new Map(),
): MemberRow[] {
  const removed = new Map<string, number>();
  const added: MemberRow[] = [];
  operations.forEach((operation, index) => {
    const op = asOp(operation);
    if (op.op === "remove_member" && typeof op.id === "string") removed.set(op.id, index);
    if (op.op === "add_member" && op.group === group) {
      const member = op.member as GroupMember;
      const identity = member.kind === "identity" ? member.id : null;
      added.push({
        key: `draft-${index}`,
        kind: member.kind,
        identity,
        issuer: member.kind === "idp_group" ? member.issuer : null,
        group: member.kind === "idp_group" ? member.group : null,
        name: identity ? (identities.get(identity)?.name ?? null) : null,
        until: null,
        id: null,
        pending: "added",
        op: index,
      });
    }
  });
  const current = members.map((m): MemberRow => {
    const op = removed.get(m.id);
    return {
      key: m.id,
      kind: m.kind === "idp_group" ? "idp_group" : "identity",
      identity: m.identity ?? null,
      issuer: m.issuer ?? null,
      group: m.group ?? null,
      name: m.name ?? null,
      until: m.valid_to ?? null,
      id: m.id,
      ...(op === undefined ? {} : { pending: "removed" as const, op }),
    };
  });
  return [...current, ...added];
}

/** The bindings of a group, marked by the operations of the active draft. */
export function bindingRows(
  bindings: readonly BindingItem[],
  operations: readonly Operation[],
  group: string,
): BindingRow[] {
  const removed = new Map<string, number>();
  const added: BindingRow[] = [];
  operations.forEach((operation, index) => {
    const op = asOp(operation);
    if (op.op === "unbind" && typeof op.id === "string") removed.set(op.id, index);
    if (op.op === "bind" && op.group === group) {
      added.push({
        key: `draft-${index}`,
        id: null,
        role: String(op.role),
        scope: servedScope(op.scope as ScopeRef),
        until: null,
        pending: "added",
        op: index,
      });
    }
  });
  const current = bindings.map((b): BindingRow => {
    const op = removed.get(b.id);
    return {
      key: b.id,
      id: b.id,
      role: b.role,
      scope: b.scope,
      until: b.valid_to ?? null,
      ...(op === undefined ? {} : { pending: "removed" as const, op }),
    };
  });
  return [...current, ...added];
}

/** The count of a list shown: its current rows, the additions of the draft left out. */
export const currentCount = (rows: readonly { pending?: Pending }[]): number =>
  rows.filter((r) => r.pending !== "added").length;

/** The changes the draft brings to a list: `{ removed, added }`. */
export function draftChanges(rows: readonly { pending?: Pending }[]) {
  return {
    removed: rows.filter((r) => r.pending === "removed").length,
    added: rows.filter((r) => r.pending === "added").length,
  };
}

/** Whether an operation names the group `key` (its row is marked in the list). */
export function touchesGroup(operation: Operation, key: string): boolean {
  const op = asOp(operation);
  return (
    ((op.op === "set_group" || op.op === "close_group") && op.key === key) ||
    ((op.op === "add_member" || op.op === "bind") && op.group === key)
  );
}

/** Whether an operation names the role `key`. */
export function touchesRole(operation: Operation, key: string): boolean {
  const op = asOp(operation);
  return (op.op === "set_role" || op.op === "close_role") && op.key === key;
}

/** The last operation of the draft that defines the role `key`, with its index. */
export function draftedRole(
  operations: readonly Operation[],
  key: string,
): { op: number; entries: string[]; closed: boolean } | null {
  for (let index = operations.length - 1; index >= 0; index -= 1) {
    const op = asOp(operations[index]!);
    if (op.key !== key) continue;
    if (op.op === "close_role") return { op: index, entries: [], closed: true };
    if (op.op === "set_role") {
      return { op: index, entries: (op.entries as string[]) ?? [], closed: false };
    }
  }
  return null;
}

/** The entries of a role, marked against those the draft gives it. */
export function entryRows(entries: readonly string[], drafted: readonly string[] | null) {
  if (drafted === null) return entries.map((entry): EntryRow => ({ entry }));
  const next = new Set(drafted);
  const before = new Set(entries);
  return [
    ...entries.map((entry): EntryRow =>
      next.has(entry) ? { entry } : { entry, pending: "removed" },
    ),
    ...drafted
      .filter((e) => !before.has(e))
      .map((entry): EntryRow => ({ entry, pending: "added" })),
  ];
}

/** The entries of a role typed one per line or separated by commas, without blanks. */
export function entriesOf(text: string): string[] {
  return [
    ...new Set(
      text
        .split(/[\n,]/)
        .map((e) => e.trim())
        .filter(Boolean),
    ),
  ];
}

/** The scope of a binding chosen in a form: no `id` when global. */
export function scopeOf(kind: ScopeKind, id: string): BindingScope {
  return kind === "global" ? { kind } : { kind, id };
}

/** The operation that binds `role` on `scope` to the group `group`. */
export const bindOp = (group: string, role: string, scope: BindingScope): Operation =>
  ({ op: "bind", group, role, scope: scopeRef(scope) }) as Operation;

/** The key of a binding: its role and its scope, to compare two of them. */
export const bindingKey = (role: string, scope: BindingScope): string =>
  `${role} ${scope.kind}${scope.id ? ` ${scope.id}` : ""}`;

/** The operations of « Modifier » on a binding: its end, then the new one. */
export function rebind(
  row: BindingRow,
  group: string,
  role: string,
  scope: BindingScope,
): Operation[] {
  return [...(row.id ? [{ op: "unbind" as const, id: row.id }] : []), bindOp(group, role, scope)];
}

/** Why an action is refused: the permission it takes, or a sentence. */
export type Refusal = { forbidden: string } | { reason: string };

/** The refusals of the actions of a group; none when an action is allowed. */
export interface GroupRefusals {
  members?: Refusal;
  edit?: Refusal;
  close?: Refusal;
  addBinding?: Refusal;
  /** By binding identifier: its modification and its removal. */
  bindings: Map<string, Refusal>;
  /** All the writes refused for one reason: said at the top, with its code. */
  readOnly?: { title: string; code: string };
}

/** The sentences of the refusals of the gate of writes, by refusal. */
export type GateWords = Record<WriteRefusal, string>;

export interface RefusalWords extends GateWords {
  wider: (code: string) => string;
  builtin: string;
  builtinMembers: string;
  widerTitle: string;
  builtinTitle: string;
}

/**
 * The simulations that say whether the group is bound wider than the user's scope, each
 * apart so that no operation hinders another (a closure ends the bindings an `unbind`
 * would name: `changeset.operation.no_version`, refused before the rules of the rights)
 * and so that each refusal is that of one binding (the rules of the rights judge the
 * final state: their violations name no operation): its closure alone
 * (`core.group.escalation`, which members and closure share), then each binding bound
 * again as it is (`core.binding.escalation`, or `core.rbac.forbidden` where the user does
 * not administer its scope). Asked only of a user who is not an administrator of the
 * whole organisation; never written.
 */
export function escalationProbe(
  group: GroupItem,
  bindings: readonly BindingItem[],
): { group: Operation[]; bindings: Operation[][] } {
  return {
    group: [{ op: "close_group", key: group.key } as Operation],
    bindings: bindings.map((b) => [bindOp(group.key, b.role, b.scope)]),
  };
}

/** What the simulation of a binding bound again refuses it for: its scope, its role. */
const BINDING_RULES = new Set([BINDING_ESCALATION, RBAC_FORBIDDEN]);

/** The violations of the simulations of `escalationProbe`, a list by binding. */
export interface ProbeViolations {
  group: readonly Violation[];
  bindings: readonly (readonly Violation[])[];
}

export function groupRefusals(input: {
  group: GroupItem;
  bindings: readonly BindingItem[];
  rights: Rights;
  /** `core.rbac.admin` on the whole organisation (a global binding). */
  globalAdmin: boolean;
  /** Why the gate of writes refuses every write (`useAdminGate`); `null` when open. */
  refused: WriteRefusal | null;
  /** The violations of `escalationProbe`, `null` when not asked or not come yet. */
  violations: ProbeViolations | null;
  words: RefusalWords;
}): GroupRefusals {
  const { group, bindings, words } = input;
  const all = (refusal: Refusal): GroupRefusals => ({
    members: refusal,
    edit: refusal,
    close: refusal,
    addBinding: refusal,
    bindings: new Map(bindings.map((b) => [b.id, refusal])),
  });
  if (!input.rights.has(RBAC_ADMIN)) return all({ forbidden: RBAC_ADMIN });
  // The gate after the permission, as the past always was here: its refusal only.
  if (input.refused) return all({ reason: words[input.refused] });
  if (group.builtin) {
    const builtin: GroupRefusals = {
      ...all({ reason: words.builtin }),
      readOnly: { title: words.builtinTitle, code: GROUP_BUILTIN },
    };
    // Its members: managed by a global administrator only.
    if (input.globalAdmin) delete builtin.members;
    else builtin.members = { reason: words.builtinMembers };
    return builtin;
  }
  const refusals: GroupRefusals = { bindings: new Map() };
  if (input.globalAdmin || input.violations === null) return refusals;
  // Only the codes of the scope count: another error says nothing of it.
  const wider = input.violations.group.some(
    (v) => v.severity === "error" && v.rule === GROUP_ESCALATION,
  );
  bindings.forEach((b, index) => {
    const refused = (input.violations!.bindings[index] ?? []).find(
      (v) => v.severity === "error" && BINDING_RULES.has(v.rule),
    );
    if (refused) refusals.bindings.set(b.id, { reason: words.wider(refused.rule) });
  });
  if (wider) {
    const reason = { reason: words.wider(GROUP_ESCALATION) };
    Object.assign(refusals, { members: reason, edit: reason, close: reason });
    refusals.addBinding = { reason: words.wider(BINDING_ESCALATION) };
    if (refusals.bindings.size === bindings.length) {
      refusals.readOnly = { title: words.widerTitle, code: GROUP_ESCALATION };
    }
  }
  return refusals;
}

/**
 * The refusal of « Créer » of a list of the administration: the permission it takes, then
 * the gate of writes (the lasting reason first, as every refusal of the administration).
 */
export function createRefusal(input: {
  rights: Rights;
  permission: string;
  /** Why the gate of writes refuses every write; `null` when open. */
  refused: WriteRefusal | null;
  words: GateWords;
}): Refusal | undefined {
  if (!input.rights.has(input.permission)) return { forbidden: input.permission };
  return input.refused ? { reason: input.words[input.refused] } : undefined;
}

/** The refusals of the actions of a role. */
export function roleRefusals(input: {
  role: RoleItem;
  rights: Rights;
  /** Why the gate of writes refuses every write (`useAdminGate`); `null` when open. */
  refused: WriteRefusal | null;
  /** The groups the role is bound to now. */
  bound: number;
  words: GateWords & { builtin: string; bound: (n: number) => string };
}): { edit?: Refusal; copy?: Refusal; remove?: Refusal; create?: Refusal } {
  if (!input.rights.has(ROLE_ADMIN)) {
    const refusal = { forbidden: ROLE_ADMIN };
    return { edit: refusal, copy: refusal, remove: refusal, create: refusal };
  }
  if (input.refused) {
    const refusal = { reason: input.words[input.refused] };
    return { edit: refusal, copy: refusal, remove: refusal, create: refusal };
  }
  if (input.role.builtin) {
    const refusal = { reason: input.words.builtin };
    return { edit: refusal, remove: refusal };
  }
  return input.bound > 0 ? { remove: { reason: input.words.bound(input.bound) } } : {};
}

/** A label by language: that of `locale`, else none (the key is shown). */
export function ownLabel(labels: Readonly<Record<string, string>>, locale: string) {
  return labels[locale] ?? null;
}

/** The labels written by a form: the one of `locale` replaced, the others kept. */
export function withLabel(
  labels: Readonly<Record<string, string>>,
  locale: string,
  label: string,
): Record<string, string> {
  const next = { ...labels };
  if (label.trim()) next[locale] = label.trim();
  else delete next[locale];
  return next;
}

/** The key of a group as the server takes it: a lower case letter, then `a-z0-9_-`. */
export const GROUP_KEY = /^[a-z][a-z0-9_-]{0,127}$/;
/** The key of a role as the server takes it: a lower case letter, then `a-z0-9_`. */
export const ROLE_KEY = /^[a-z][a-z0-9_]*$/;
