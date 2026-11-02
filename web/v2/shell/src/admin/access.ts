import {
  ApiProblem,
  FORBIDDEN_RULE,
  type components,
  type Operation,
  type Violation,
} from "@kartenn/sdk";
import { covers } from "../rights/index.ts";
import type { Refusal as WriteRefusal } from "../views/edit/inline.ts";
import type { Pending, Refusal } from "./model.ts";

// The access to kartenn as the administration reads and writes it (005, REQ-005-17;
// spec 025, REQ-025-11; REQ-021-12; design 021 « Administration »; maquette E7, vues
// `identite`, `secret`): the identities, their groups and their effective rights in read;
// the service accounts and their secrets; the issuers attached to the entities. Their
// permissions are global only (`core.account.admin`, `core.issuer.admin`,
// `core.identity.pseudonymise`, 005): held on the whole organisation, not on an entity.
// The state of an identity, the revocation of a secret and the attachments go to the
// draft; a secret is made by the server alone (its digest is in the changeset), so the
// creation of an account and a new secret are written at once, the secret shown once.

export type AccountItem = components["schemas"]["AccountItem"];
export type SecretItem = components["schemas"]["SecretItem"];
export type SecretShown = components["schemas"]["SecretShown"];
export type AttachmentItem = components["schemas"]["AttachmentItem"];
export type IdentityDetail = components["schemas"]["IdentityDetail"];
type Me = components["schemas"]["Me"];

/** Who manages the service accounts and the state of the identities (global, 005). */
export const ACCOUNT_ADMIN = "core.account.admin";
/** Who lists the identities and reads their page (global): an actor of the audit by name. */
export const IDENTITIES_READ = "core.rbac.read";
/** Who attaches the issuers (global, 005). */
export const ISSUER_ADMIN = "core.issuer.admin";
/** Who pseudonymises a person at once (global, 005, REQ-004-33). */
export const PSEUDONYMISE = "core.identity.pseudonymise";
/** The name of a service account: the Basic user, without `:` (RFC 7617 § 2). */
export const ACCOUNT_NAME = /^[^:\s][^:]*$/;

/** Whether `me` holds `permission` on the whole organisation. */
export function holdsGlobally(me: Me | null | undefined, permission: string): boolean {
  return me?.global_permissions.some((entry) => covers(entry, permission)) ?? false;
}

/** Why an action under a global permission is refused: the permission, then the past. */
export function globalRefusal(input: {
  me: Me | null | undefined;
  permission: string;
  present: boolean;
  past: string;
}): Refusal | undefined {
  if (!holdsGlobally(input.me, input.permission)) return { forbidden: input.permission };
  return input.present ? undefined : { reason: input.past };
}

/** The operations of the shapes this file reads. */
type Op = {
  op: string;
  identity?: string;
  account?: string;
  secret?: string;
  issuer?: string;
  entity?: { id?: string; local?: number } | null;
  disabled?: boolean | null;
  revoke_sessions?: boolean;
};
const asOp = (op: Operation): Op => op as unknown as Op;

/** A change of the state of the identity `id` waiting in the draft, with its index. */
export interface DraftedState {
  index: number;
  disabled: boolean | null;
  revokeSessions: boolean;
}

/** The changes of the state of the identity `id` the draft holds, in their order. */
export function draftedStates(operations: readonly Operation[], id: string): DraftedState[] {
  return operations.flatMap((operation, index) => {
    const op = asOp(operation);
    return op.op === "set_identity_state" && op.identity === id
      ? [{ index, disabled: op.disabled ?? null, revokeSessions: op.revoke_sessions === true }]
      : [];
  });
}

/** A secret of an account as its list shows it: a revocation waiting is marked. */
export interface SecretRow extends SecretItem {
  pending?: Pending;
  /** The index of the operation of the draft that revokes it. */
  index?: number;
}

/** The secrets of `account`, newest first; those the draft revokes marked, in place. */
export function secretRows(
  secrets: readonly SecretItem[],
  operations: readonly Operation[],
  account: string,
): SecretRow[] {
  const revoking = new Map<string, number>();
  operations.forEach((operation, index) => {
    const op = asOp(operation);
    if (op.op === "revoke_service_secret" && op.account === account && op.secret) {
      revoking.set(op.secret, index);
    }
  });
  return [...secrets]
    .sort((a, b) => b.created_at.localeCompare(a.created_at))
    .map((secret) => {
      const index = revoking.get(secret.id);
      return index === undefined || secret.revoked
        ? { ...secret }
        : { ...secret, pending: "removed" as const, index };
    });
}

/** The secrets of an account that still open a session. */
export function activeSecrets(account: Pick<AccountItem, "secrets">): number {
  return account.secrets.filter((s) => !s.revoked).length;
}

/** An attachment as the list of the issuers shows it, marked by the draft (a grid row). */
export type AttachmentRow = AttachmentItem & {
  /** `issuer` and `entity`: the key of its row. */
  key: string;
  pending?: Pending;
  /** The index of the operation of the draft that adds or removes it. */
  index?: number;
};

/** The key of an attachment: its issuer, then its entity (none: the organisation). */
export const attachmentKey = (issuer: string, entity: string | null | undefined): string =>
  `${issuer} ${entity ?? ""}`;

/**
 * The attachments as the draft would leave them: a detachment waiting stays in place,
 * marked; an attachment waiting comes at the end. A local entity (`#n`, made in the same
 * draft) is named by its reference.
 */
export function attachmentRows(
  items: readonly AttachmentItem[],
  operations: readonly Operation[],
): AttachmentRow[] {
  const entityOf = (ref: Op["entity"]) =>
    ref == null ? null : (ref.id ?? (ref.local === undefined ? null : `#${ref.local}`));
  const rows: AttachmentRow[] = items.map((item) => ({
    ...item,
    key: attachmentKey(item.issuer, item.entity),
  }));
  const byKey = new Map(rows.map((row) => [row.key, row]));
  operations.forEach((operation, index) => {
    const op = asOp(operation);
    if ((op.op !== "attach_issuer" && op.op !== "detach_issuer") || !op.issuer) return;
    const entity = entityOf(op.entity);
    const key = attachmentKey(op.issuer, entity);
    const served = byKey.get(key);
    if (op.op === "detach_issuer" && served && served.pending === undefined) {
      Object.assign(served, { pending: "removed", index });
    } else if (op.op === "attach_issuer" && !served) {
      const added: AttachmentRow = { issuer: op.issuer, entity, key, pending: "added", index };
      rows.push(added);
      byKey.set(key, added);
    }
  });
  return rows;
}

/** The operation that attaches or detaches `issuer`, on an entity or the organisation. */
export function attachmentOp(
  op: "attach_issuer" | "detach_issuer",
  issuer: string,
  entity: string | null | undefined,
): Operation {
  return { op, issuer, entity: entity ? { id: entity } : null } as Operation;
}

/** A group of the issuer at the last login, and the groups of kartenn it gives. */
export interface IdpGroupRow {
  name: string;
  groups: string[];
}

/** The groups of its issuer at the last login of an identity, each with what it gives. */
export function idpGroupRows(identity: IdentityDetail): IdpGroupRow[] {
  return identity.last_login_idp_groups.map((name) => ({
    name,
    groups: identity.groups.filter((g) => g.via === name).map((g) => g.key),
  }));
}

/** The words of a write refused by the server, for `RefusalAlert`. */
export function refusalOf(error: unknown): WriteRefusal {
  if (!(error instanceof ApiProblem)) return { kind: "failed", input: null, problem: null };
  const denied = error.violations.some((v) => FORBIDDEN_RULE.test(v.rule));
  if (error.status === 403 || denied) return { kind: "forbidden", input: null, problem: error };
  if (error.status === 400 || error.status === 422) {
    const byField: Record<string, Violation[]> = {};
    const general: Violation[] = [];
    for (const violation of error.violations) {
      const field = /^\/([^/]+)/.exec(violation.pointer ?? "")?.[1];
      if (field) (byField[field] ??= []).push(violation);
      else general.push(violation);
    }
    return { kind: "invalid", byField, general, input: null, problem: error };
  }
  return { kind: "failed", input: null, problem: error };
}
