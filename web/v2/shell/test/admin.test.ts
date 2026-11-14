import type { Operation, Violation } from "@kartenn/sdk";
import { describe, expect, it } from "vitest";
import {
  bindingRows,
  bindOp,
  currentCount,
  draftChanges,
  draftedRole,
  entriesOf,
  entryRows,
  escalationProbe,
  GROUP_KEY,
  groupRefusals,
  memberName,
  memberRows,
  rebind,
  roleRefusals,
  ROLE_KEY,
  servedScope,
  touchesGroup,
  withLabel,
  type BindingItem,
  type GroupItem,
  type MemberItem,
  type RoleItem,
} from "../src/admin/model.ts";
import { frameRoutes } from "../src/frame/routes.ts";
import { Rights } from "../src/rights/index.ts";

// The administration of the rights by groups (task 021-15a; REQ-021-12, REQ-025-12; maquette
// E7): the rows of a group marked by the active draft — a removal kept in place, an addition
// at the end, each with its operation —, the refusals by the rights, the past, a group
// delivered and the simulation of the server (never a scope guessed), the roles and their
// served permissions, and the routes of the space whose objects open in the right panel.

const ops = (...list: unknown[]) => list as Operation[];
const at = "2026-01-01T00:00:00Z";
const member = (id: string, extra: Partial<MemberItem> = {}): MemberItem => ({
  id,
  href: `/v1/groups/g/members/${id}`,
  kind: "identity",
  identity: `i-${id}`,
  name: `Personne ${id}`,
  valid_from: at,
  ...extra,
});
const binding = (id: string, role: string, extra: Partial<BindingItem> = {}): BindingItem => ({
  id,
  href: `/v1/bindings/${id}`,
  group: "supervision",
  role,
  scope: { kind: "global" },
  valid_from: at,
  ...extra,
});
const group = (extra: Partial<GroupItem> = {}): GroupItem => ({
  id: "g",
  href: "/v1/groups/g",
  key: "supervision",
  labels: { fr: "Équipe supervision" },
  builtin: false,
  members: 2,
  bindings: 2,
  ...extra,
});
const words = {
  past: "passé",
  narrow: "étroit",
  wider: (code: string) => `plus large (${code})`,
  builtin: "livré",
  builtinMembers: "livré, membres",
  widerTitle: "Lecture seule : plus large",
  builtinTitle: "Groupe livré",
};

describe("rows of a group by the draft", () => {
  it("keeps a removal in place, adds an addition at the end, each with its operation", () => {
    const draft = ops(
      { op: "set_prefix", id: "p" },
      { op: "remove_member", id: "m2" },
      {
        op: "add_member",
        group: "supervision",
        member: { kind: "idp_group", issuer: "dev", group: "ops" },
      },
      { op: "add_member", group: "autre", member: { kind: "identity", id: "x" } },
    );
    const rows = memberRows([member("m1"), member("m2")], draft, "supervision");
    expect(rows.map((r) => [r.key, r.pending, r.op])).toEqual([
      ["m1", undefined, undefined],
      ["m2", "removed", 1],
      ["draft-2", "added", 2],
    ]);
    expect(memberName(rows[2]!)).toBe("dev / ops");
    expect(currentCount(rows)).toBe(2);
    expect(draftChanges(rows)).toEqual({ removed: 1, added: 1 });
  });

  it("reads the scope of a binding drafted by reference, and writes one so", () => {
    const scope = { kind: "domain", id: "d1" };
    const draft = ops({ op: "unbind", id: "b1" }, bindOp("supervision", "editor", scope));
    expect(draft[1]).toEqual({
      op: "bind",
      group: "supervision",
      role: "editor",
      scope: { kind: "domain", id: { id: "d1" } },
    });
    const rows = bindingRows([binding("b1", "reader")], draft, "supervision");
    expect(rows.map((r) => [r.role, r.pending, r.op, r.scope])).toEqual([
      ["reader", "removed", 0, { kind: "global" }],
      ["editor", "added", 1, scope],
    ]);
    expect(servedScope({ kind: "site", id: { local: 3 } })).toEqual({ kind: "site", id: "#3" });
    expect(bindOp("g", "reader", { kind: "global" })).toMatchObject({ scope: { kind: "global" } });
  });

  it("changes a binding by its end then a new one", () => {
    const row = bindingRows([binding("b1", "reader")], [], "supervision")[0]!;
    expect(rebind(row, "supervision", "editor", { kind: "site", id: "s1" })).toEqual([
      { op: "unbind", id: "b1" },
      {
        op: "bind",
        group: "supervision",
        role: "editor",
        scope: { kind: "site", id: { id: "s1" } },
      },
    ]);
  });

  it("marks a group the draft names", () => {
    expect(touchesGroup({ op: "bind", group: "a" } as unknown as Operation, "a")).toBe(true);
    expect(touchesGroup({ op: "close_group", key: "a" } as unknown as Operation, "a")).toBe(true);
    expect(touchesGroup({ op: "set_role", key: "a" } as unknown as Operation, "a")).toBe(false);
  });
});

describe("refusals of a group", () => {
  const admin = new Rights(["core.rbac.admin", "core.rbac.read"]);
  const two = [binding("b1", "reader"), binding("b2", "editor")];
  const refuse = (extra: Partial<Parameters<typeof groupRefusals>[0]> = {}) =>
    groupRefusals({
      group: group(),
      bindings: two,
      rights: admin,
      globalAdmin: false,
      refused: null,
      violations: { group: [], bindings: [] },
      words,
      ...extra,
    });

  it("refuses every write by the permission lacking, then by the gate of writes", () => {
    const lacking = refuse({ rights: new Rights(["core.rbac.read"]) });
    expect(lacking.members).toEqual({ forbidden: "core.rbac.admin" });
    expect(lacking.bindings.get("b2")).toEqual({ forbidden: "core.rbac.admin" });
    expect(refuse({ refused: "past" }).close).toEqual({ reason: "passé" });
    const narrow = refuse({ refused: "narrow" });
    expect(narrow.close).toEqual({ reason: "étroit" });
    expect(narrow.addBinding).toEqual({ reason: "étroit" });
    expect(narrow.bindings.get("b1")).toEqual({ reason: "étroit" });
    // The permission lacking is said first, at any width.
    expect(refuse({ rights: new Rights([]), refused: "narrow" }).edit).toEqual({
      forbidden: "core.rbac.admin",
    });
  });

  it("never offers under the gate what the rights or the scope refuse (task 021-21e)", () => {
    const violation = (rule: string): Violation =>
      ({ operation: null, rule, severity: "error", detail: "", subjects: [] }) as Violation;
    const cases: Partial<Parameters<typeof groupRefusals>[0]>[] = [
      {},
      { globalAdmin: true, violations: null },
      { rights: new Rights(["core.rbac.read"]) },
      { group: group({ key: "secours", builtin: true }) },
      { group: group({ key: "secours", builtin: true }), globalAdmin: true },
      {
        violations: {
          group: [violation("core.group.escalation")],
          bindings: [[violation("core.rbac.forbidden")], [violation("core.binding.escalation")]],
        },
      },
    ];
    const ACTIONS = ["members", "edit", "close", "addBinding"] as const;
    for (const extra of cases) {
      const open = refuse(extra);
      for (const refused of ["narrow", "past"] as const) {
        const held = refuse({ ...extra, refused });
        // Every action refused, the bindings each: nothing the open gate refuses is offered.
        for (const action of ACTIONS) expect(held[action]).toBeDefined();
        for (const b of two) expect(held.bindings.get(b.id)).toBeDefined();
        // A permission lacking stays the reason said.
        for (const action of ACTIONS) {
          if (open[action] && "forbidden" in open[action]) {
            expect(held[action]).toEqual(open[action]);
          }
        }
      }
    }
  });

  it("allows all to a global administrator, and asks the server nothing for them", () => {
    const allowed = refuse({ globalAdmin: true, violations: null });
    expect(allowed.members ?? allowed.close ?? allowed.readOnly).toBeUndefined();
    expect(allowed.bindings.size).toBe(0);
    // Simulations that do not hinder each other: the closure alone, then each binding
    // bound again as it is, alone (an `unbind` after the closure would name an ended
    // binding; the rules of the rights name no operation).
    expect(escalationProbe(group(), two)).toEqual({
      group: [{ op: "close_group", key: "supervision" }],
      bindings: [
        [{ op: "bind", group: "supervision", role: "reader", scope: { kind: "global" } }],
        [{ op: "bind", group: "supervision", role: "editor", scope: { kind: "global" } }],
      ],
    });
  });

  it("refuses by the codes of the scope only, read only when all are refused", () => {
    // As the server serves them: a rule of the final state, no operation named.
    const violation = (rule: string): Violation =>
      ({ operation: null, rule, severity: "error", detail: "", subjects: [] }) as Violation;
    const one = refuse({
      violations: {
        group: [violation("changeset.operation.no_version")],
        bindings: [[violation("core.role.unknown")], [violation("core.binding.escalation")]],
      },
    });
    expect(one.bindings.get("b2")).toEqual({ reason: "plus large (core.binding.escalation)" });
    // Another error says nothing of the scope.
    expect(one.bindings.has("b1")).toBe(false);
    expect(one.members).toBeUndefined();
    const all = refuse({
      violations: {
        group: [violation("core.group.escalation")],
        bindings: [[violation("core.rbac.forbidden")], [violation("core.binding.escalation")]],
      },
    });
    expect(all.bindings.get("b1")).toEqual({ reason: "plus large (core.rbac.forbidden)" });
    expect(all.members).toEqual({ reason: "plus large (core.group.escalation)" });
    expect(all.addBinding).toEqual({ reason: "plus large (core.binding.escalation)" });
    expect(all.readOnly).toEqual({ title: words.widerTitle, code: "core.group.escalation" });
  });

  it("keeps the members of a delivered group to a global administrator", () => {
    const local = refuse({ group: group({ key: "secours", builtin: true }) });
    expect(local.members).toEqual({ reason: "livré, membres" });
    expect(local.readOnly?.code).toBe("core.group.builtin");
    const global = refuse({ group: group({ key: "secours", builtin: true }), globalAdmin: true });
    expect(global.members).toBeUndefined();
    expect(global.edit).toEqual({ reason: "livré" });
  });
});

describe("roles", () => {
  const role = (extra: Partial<RoleItem> = {}): RoleItem => ({
    key: "lecteur_reseau",
    href: "/v1/roles/lecteur_reseau",
    labels: {},
    builtin: false,
    entries: ["core.prefix.read", "core.domain.read"],
    ...extra,
  });

  it("marks the permissions against the definition the draft holds", () => {
    const draft = ops({
      op: "set_role",
      key: "lecteur_reseau",
      labels: {},
      entries: ["core.prefix.read", "prop.*.read"],
    });
    const drafted = draftedRole(draft, "lecteur_reseau");
    expect(drafted).toEqual({ op: 0, entries: ["core.prefix.read", "prop.*.read"], closed: false });
    expect(entryRows(role().entries, drafted!.entries)).toEqual([
      { entry: "core.prefix.read" },
      { entry: "core.domain.read", pending: "removed" },
      { entry: "prop.*.read", pending: "added" },
    ]);
    expect(
      draftedRole(ops({ op: "close_role", key: "lecteur_reseau" }), "lecteur_reseau")?.closed,
    ).toBe(true);
  });

  it("refuses a built-in role, a role bound, the permission lacking", () => {
    const refusal = (extra: Partial<Parameters<typeof roleRefusals>[0]>) =>
      roleRefusals({
        role: role(),
        rights: new Rights(["core.role.admin"]),
        refused: null,
        bound: 0,
        words: {
          past: "passé",
          narrow: "étroit",
          builtin: "intégré",
          bound: (n) => `lié à ${n}`,
        },
        ...extra,
      });
    expect(refusal({})).toEqual({});
    // The gate of writes after the permission: every action refused with its reason.
    const narrow = { reason: "étroit" };
    expect(refusal({ refused: "narrow" })).toEqual({
      edit: narrow,
      copy: narrow,
      remove: narrow,
      create: narrow,
    });
    expect(refusal({ refused: "past" }).create).toEqual({ reason: "passé" });
    expect(refusal({ rights: new Rights([]), refused: "narrow" }).edit).toEqual({
      forbidden: "core.role.admin",
    });
    // Never an action the open gate refuses offered under it.
    for (const extra of [{ bound: 4 }, { role: role({ builtin: true }) }]) {
      const held = refusal({ ...extra, refused: "narrow" });
      for (const action of ["edit", "copy", "remove", "create"] as const) {
        expect(held[action]).toBeDefined();
      }
    }
    expect(refusal({ bound: 4 }).remove).toEqual({ reason: "lié à 4" });
    expect(refusal({ role: role({ builtin: true }) })).toEqual({
      edit: { reason: "intégré" },
      remove: { reason: "intégré" },
    });
    expect(refusal({ rights: new Rights([]) }).create).toEqual({ forbidden: "core.role.admin" });
  });

  it("reads the entries typed, the keys the server takes, the labels of one language", () => {
    expect(entriesOf("core.prefix.read, prop.*.edit\ncore.prefix.read,")).toEqual([
      "core.prefix.read",
      "prop.*.edit",
    ]);
    expect(ROLE_KEY.test("lecteur_reseau")).toBe(true);
    expect(ROLE_KEY.test("lecteur-reseau")).toBe(false);
    expect(GROUP_KEY.test("equipe-supervision")).toBe(true);
    expect(GROUP_KEY.test("1equipe")).toBe(false);
    expect(withLabel({ fr: "a", en: "b" }, "fr", " ")).toEqual({ en: "b" });
    expect(withLabel({ en: "b" }, "fr", "Lecteur")).toEqual({ en: "b", fr: "Lecteur" });
  });
});

describe("routes of the administration", () => {
  it("opens a group and a role of their list in the right panel", () => {
    for (const name of ["groups-record", "roles-record"]) {
      const route = frameRoutes.find((r) => r.name === name);
      expect(route?.meta?.detail, name).toBeDefined();
    }
    expect(frameRoutes.find((r) => r.name === "groups-record")?.path).toBe("admin/groups/:id");
    // Both screens real, no longer the pending view.
    const groups = frameRoutes.find((r) => r.name === "groups");
    expect(typeof groups?.component).toBe("function");
  });
});
