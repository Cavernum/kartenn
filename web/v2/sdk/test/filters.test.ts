// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";
import { filterModel, filterSearch, parseFilters } from "../src/data/filters.ts";
import { keys } from "../src/data/keys.ts";
import { D1 } from "./helpers.ts";

const declarations = [
  { key: "core.owner", type: "ref", carriers: ["prefix", "address"] },
  { key: "policy.environment", type: "enum", carriers: ["prefix"] },
  { key: "ops.priority", type: "int", carriers: [] },
  { key: "machines.os", type: "text", carriers: ["machines.machine"] },
];

describe("filter model (REQ-021-18)", () => {
  it("offers the filters of the route and those of the properties its objects carry", () => {
    const model = filterModel("/v1/prefixes", declarations);
    const names = model.map((f) => f.name);
    expect(names.slice(0, 4)).toEqual(["domain", "value", "within", "nature"]);
    expect(names).toContain("prop.core.owner");
    expect(names).toContain("prop.policy.environment:exists");
    expect(names).toContain("prop.ops.priority:between");
    // `:between` only on ordered types; a property of machines is not a prefix filter.
    expect(names).not.toContain("prop.core.owner:between");
    expect(names.some((n) => n.startsWith("prop.machines.os"))).toBe(false);
    const nature = model.find((f) => f.name === "nature");
    expect(nature?.enum).toEqual(["container", "network"]);
    const environment = model.find((f) => f.name === "prop.policy.environment");
    expect(environment?.collection).toBe("/v1/property-declarations/policy.environment/values");
    expect(model.find((f) => f.name === "prop.ops.priority")?.type).toBe("integer");
    expect(model.find((f) => f.name === "domain")?.collection).toBe("/v1/domains");
  });

  it("refuses a path that is not a list", () => {
    expect(() => filterModel("/v1/me")).toThrow(/not a list/);
  });

  it("parses a URL against the model, setting aside what does not fit", () => {
    const model = filterModel("/v1/prefixes", declarations);
    const { filters, rejected } = parseFilters(
      model,
      `nature=network&domain=${D1}&prop.ops.priority=high&color=red&valid_at=2026-01-01T00:00:00Z` +
        "&prop.policy.environment=prod&prop.policy.environment=preprod&nature=planet",
    );
    expect(filters).toEqual({
      nature: "network",
      domain: D1,
      "prop.policy.environment": ["prod", "preprod"],
    });
    expect(rejected.map((r) => [r.name, r.reason])).toEqual([
      ["prop.ops.priority", "type"],
      ["color", "unknown"],
      ["nature", "enum"],
    ]);
  });

  it("writes the URL in the order of the keys of the cache: same filters, same link", () => {
    const a = filterSearch({ within: "10.0.0.0/8", domain: D1 });
    const b = filterSearch({ domain: D1, within: "10.0.0.0/8", value: null });
    expect(a).toBe(b);
    const { filters } = parseFilters(filterModel("/v1/prefixes"), a);
    expect(keys.list("/v1/prefixes", filters, null)).toEqual(
      keys.list("/v1/prefixes", { domain: D1, within: "10.0.0.0/8" }, null),
    );
  });
});
