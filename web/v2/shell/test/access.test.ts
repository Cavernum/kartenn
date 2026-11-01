import { ApiProblem, type components, type Operation } from "@kartenn/sdk";
import { describe, expect, it } from "vitest";
import {
  ACCOUNT_ADMIN,
  ACCOUNT_NAME,
  activeSecrets,
  attachmentOp,
  attachmentRows,
  draftedStates,
  globalRefusal,
  holdsGlobally,
  idpGroupRows,
  refusalOf,
  secretRows,
  type IdentityDetail,
  type SecretItem,
} from "../src/admin/access.ts";
import { frameRoutes } from "../src/frame/routes.ts";

// The access to kartenn in the administration (task 021-15b; REQ-021-12, REQ-025-11;
// maquette E7, vues `identite`, `secret`): global permissions held on the whole
// organisation only, the state of an identity, the secrets and the attachments as the
// draft marks them, the groups of the IdP at the last login, and the routes whose objects
// open in the right panel.

const ops = (...list: unknown[]) => list as Operation[];
type Me = components["schemas"]["Me"];
const me = (global: string[]) => ({ global_permissions: global }) as unknown as Me;
const secret = (id: string, created: string, extra: Partial<SecretItem> = {}): SecretItem => ({
  id,
  key_id: "k1",
  created_at: created,
  revoked: false,
  ...extra,
});

describe("global permissions", () => {
  it("holds a permission on the whole organisation only, patterns included", () => {
    expect(holdsGlobally(me(["core.account.admin"]), ACCOUNT_ADMIN)).toBe(true);
    expect(holdsGlobally(me(["*"]), ACCOUNT_ADMIN)).toBe(true);
    // Administrator of an entity: core.rbac.read global, the rest on the entity.
    expect(holdsGlobally(me(["core.rbac.read"]), ACCOUNT_ADMIN)).toBe(false);
    expect(holdsGlobally(null, ACCOUNT_ADMIN)).toBe(false);
  });

  it("refuses by the permission first, then by the past", () => {
    const words = { permission: ACCOUNT_ADMIN, past: "passé" };
    expect(globalRefusal({ ...words, me: me([]), present: false })).toEqual({
      forbidden: ACCOUNT_ADMIN,
    });
    expect(globalRefusal({ ...words, me: me(["*"]), present: false })).toEqual({
      reason: "passé",
    });
    expect(globalRefusal({ ...words, me: me(["*"]), present: true })).toBeUndefined();
  });
});

describe("identities", () => {
  it("finds the changes of the state of an identity in the draft", () => {
    const draft = ops(
      { op: "set_identity_state", identity: "a", disabled: true },
      { op: "set_group", key: "g", labels: {} },
      { op: "set_identity_state", identity: "a", revoke_sessions: true },
      { op: "set_identity_state", identity: "b", disabled: false },
    );
    expect(draftedStates(draft, "a")).toEqual([
      { index: 0, disabled: true, revokeSessions: false },
      { index: 2, disabled: null, revokeSessions: true },
    ]);
    expect(draftedStates(draft, "c")).toEqual([]);
  });

  it("gives each group of the IdP at the last login the groups of kartenn it makes", () => {
    const identity = {
      groups: [
        { key: "admins", via: "ops" },
        { key: "lecteurs", via: "ops" },
        { key: "astreinte", via: "direct" },
      ],
      last_login_idp_groups: ["ops", "compta"],
    } as unknown as IdentityDetail;
    expect(idpGroupRows(identity)).toEqual([
      { name: "ops", groups: ["admins", "lecteurs"] },
      { name: "compta", groups: [] },
    ]);
  });
});

describe("service accounts", () => {
  it("lists the secrets newest first, a revocation waiting marked in place", () => {
    const secrets = [
      secret("s1", "2026-01-01T00:00:00Z"),
      secret("s2", "2026-03-01T00:00:00Z"),
      secret("s3", "2026-02-01T00:00:00Z", { revoked: true }),
    ];
    const draft = ops(
      { op: "revoke_service_secret", account: "acc", secret: "s1" },
      { op: "revoke_service_secret", account: "other", secret: "s2" },
      { op: "revoke_service_secret", account: "acc", secret: "s3" },
    );
    const rows = secretRows(secrets, draft, "acc");
    expect(rows.map((r) => [r.id, r.pending, r.index])).toEqual([
      ["s2", undefined, undefined],
      // Revoked already: nothing to mark.
      ["s3", undefined, undefined],
      ["s1", "removed", 0],
    ]);
    expect(activeSecrets({ secrets })).toBe(2);
  });

  it("takes a name without a colon, the user of Basic", () => {
    expect(ACCOUNT_NAME.test("sync-ipam")).toBe(true);
    expect(ACCOUNT_NAME.test("a:b")).toBe(false);
    expect(ACCOUNT_NAME.test(" lead")).toBe(false);
  });

  it("says a refusal of the server by its kind", () => {
    const forbidden = new ApiProblem({ status: 403, type: "t", title: "t" });
    expect(refusalOf(forbidden).kind).toBe("forbidden");
    const invalid = new ApiProblem({
      status: 422,
      type: "t",
      title: "t",
      violations: [{ rule: "core.rbac.invalid", detail: "d", pointer: "/name" } as never],
    });
    const said = refusalOf(invalid);
    expect(said.kind).toBe("invalid");
    expect(said.kind === "invalid" && Object.keys(said.byField)).toEqual(["name"]);
    expect(refusalOf(new Error("network")).kind).toBe("failed");
  });
});

describe("issuers", () => {
  it("marks a detachment in place and puts an attachment at the end", () => {
    const served = [
      { issuer: "https://idp.example", entity: null },
      { issuer: "https://idp.example", entity: "e1" },
    ];
    const draft = ops(
      attachmentOp("detach_issuer", "https://idp.example", "e1"),
      attachmentOp("attach_issuer", "https://other.example", null),
      // Attached already: not twice.
      attachmentOp("attach_issuer", "https://idp.example", null),
    );
    const rows = attachmentRows(served, draft);
    expect(rows.map((r) => [r.issuer, r.entity, r.pending, r.index])).toEqual([
      ["https://idp.example", null, undefined, undefined],
      ["https://idp.example", "e1", "removed", 0],
      ["https://other.example", null, "added", 1],
    ]);
    expect(attachmentOp("attach_issuer", "i", "e1")).toEqual({
      op: "attach_issuer",
      issuer: "i",
      entity: { id: "e1" },
    });
  });
});

describe("routes of the access", () => {
  it("opens an identity and an account of their list in the right panel", () => {
    for (const name of ["identities-record", "accounts-record"]) {
      const route = frameRoutes.find((r) => r.name === name);
      expect(route?.meta?.detail, name).toBeDefined();
    }
    expect(frameRoutes.find((r) => r.name === "accounts-record")?.path).toBe("admin/accounts/:id");
    // The issuers a list alone: an attachment has no record.
    expect(frameRoutes.find((r) => r.name === "issuers-record")).toBeUndefined();
    expect(typeof frameRoutes.find((r) => r.name === "issuers")?.component).toBe("function");
  });
});
