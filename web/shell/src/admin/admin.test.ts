import { describe, expect, it } from "vitest";
import { Rights } from "../rights/permissions.ts";
import { entriesOf, exportName, outcomeOf, SECTIONS, subjectText } from "./admin.ts";

describe("sections of the administration (REQ-007-19)", () => {
  const open = (entries: string[]) =>
    SECTIONS.filter((s) => new Rights(entries).hasAny(s.anyOf)).map((s) => s.path);

  it("all for an administrator, none for a reader of the core", () => {
    expect(open(["*"])).toHaveLength(SECTIONS.length);
    expect(open(["core.address.read", "core.changeset.read"])).toEqual([]);
  });

  it("the audit only under core.audit.read", () => {
    expect(open(["core.audit.read"])).toEqual(["/admin/audit"]);
    expect(open(["core.rbac.read"])).not.toContain("/admin/audit");
  });
});

describe("writes of the administration", () => {
  const base = { etag: null };

  it("applied, held or refused, with the secret shown once", () => {
    const secret = { id: "s", value: "v" };
    expect(
      outcomeOf({
        ...base,
        ok: true,
        status: 201,
        body: { changeset: { id: "c" }, state: "applied", secret },
      }),
    ).toEqual({ kind: "applied", changeset: "c", secret });
    expect(
      outcomeOf({ ...base, ok: true, status: 202, body: { changeset: { id: "c" } } }).kind,
    ).toBe("pending");
    expect(
      outcomeOf({
        ...base,
        ok: false,
        status: 403,
        problem: { type: "t", title: "x", detail: null, status: 403 },
      }).kind,
    ).toBe("refused");
  });

  it("entries one per line or by commas, without blanks nor duplicates", () => {
    expect(entriesOf("core.a.read, core.b.read\n\n core.a.read \n")).toEqual([
      "core.a.read",
      "core.b.read",
    ]);
  });

  it("subjects named, exports dated", () => {
    expect(subjectText({ kind: "group", group: "g", issuer: "i" })).toBe("g @ i");
    expect(subjectText({ kind: "identity", id: "x" })).toBe("x");
    expect(exportName(new Date("2026-10-21T10:00:00Z"))).toBe(
      "kartenn-audit-2026-10-21T10-00-00-000Z.ndjson",
    );
  });
});
