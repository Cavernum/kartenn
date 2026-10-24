import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { ADMIN_PERMISSIONS, NAVIGATION, visibleEntries } from "../navigation.ts";
import { rightsOf, type Me } from "./index.ts";
import { covers, Rights } from "./permissions.ts";

// The permissions of the system spec (docs/spec/permissions.md), first column.
const declared = [
  ...readFileSync(
    new URL("../../../../docs/spec/permissions.md", import.meta.url),
    "utf8",
  ).matchAll(/^\| `([a-z*][\w.*<>é-]*)` \|/gm),
].map((m) => m[1]!);

describe("patterns of role entries (REQ-007-20)", () => {
  it("`*` covers every permission", () => {
    for (const p of declared) expect(covers("*", p), p).toBe(true);
  });

  it("an entry without `*` covers itself only", () => {
    expect(covers("core.prefix.read", "core.prefix.read")).toBe(true);
    expect(covers("core.prefix.read", "core.prefix.edit")).toBe(false);
    expect(covers("core.prefix", "core.prefix.read")).toBe(false);
  });

  it("`prop.*.<action>` covers the action on every key, dotted keys included", () => {
    expect(covers("prop.*.edit", "prop.core.owner.edit")).toBe(true);
    expect(covers("prop.*.edit", "prop.policy.environment.edit")).toBe(true);
    expect(covers("prop.*.edit", "prop.core.owner.read")).toBe(false);
    expect(covers("prop.*.edit", "prop.edit")).toBe(false);
    expect(covers("prop.*.edit", "core.prefix.edit")).toBe(false);
  });

  it("the shipped roles open what they say", () => {
    const editor = new Rights(["core.prefix.read", "core.prefix.edit", "prop.*.edit"]);
    expect(editor.has("prop.core.site.edit")).toBe(true);
    expect(editor.has("core.rbac.admin")).toBe(false);
  });
});

describe("navigation", () => {
  it("names permissions of the system spec only", () => {
    expect(declared.length).toBeGreaterThan(10);
    for (const entry of NAVIGATION) {
      for (const p of entry.anyOf ?? []) expect(declared, p).toContain(p);
    }
  });

  it("hides the administration from a reader, shows it to an administrator", () => {
    const reader = new Rights(["core.entity.read", "core.prefix.read"]);
    expect(visibleEntries(reader).map((e) => e.to)).not.toContain("/admin");
    expect(visibleEntries(new Rights(["*"])).map((e) => e.to)).toContain("/admin");
    for (const p of ADMIN_PERMISSIONS) {
      expect(visibleEntries(new Rights([p])).map((e) => e.to)).toContain("/admin");
    }
  });
});

describe("rights of a caller", () => {
  const me = (bindings: Me["bindings"], global: string[]): Me => ({
    identity: "i",
    kind: "person",
    name: "n",
    groups: [],
    roles: [],
    bindings,
    global_permissions: global,
  });

  it("join global entries and those of the roles of scoped bindings", async () => {
    const fetcher = (async (url: string) =>
      url === "/v1/roles/editor"
        ? Response.json({ entries: ["core.prefix.edit"] })
        : new Response(null, { status: 403 })) as typeof fetch;
    const rights = await rightsOf(
      me(
        [
          { role: "editor", scope: { kind: "domain", id: "d" } },
          { role: "secret", scope: { kind: "entity", id: "e" } },
          { role: "reader", scope: { kind: "global" } },
        ],
        ["core.prefix.read"],
      ),
      fetcher,
    );
    expect(rights.has("core.prefix.read")).toBe(true);
    expect(rights.has("core.prefix.edit")).toBe(true);
    expect(rights.has("core.rbac.read")).toBe(false);
  });
});
