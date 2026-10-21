import { describe, expect, it } from "vitest";
import { CREATION, creation, initial, MODIFICATION, patch } from "./fields.ts";
import { fieldOf, outcomeOf, submit } from "./submit.ts";

const json = (status: number, body: unknown, headers: Record<string, string> = {}) =>
  new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json", ...headers },
  });

describe("the fields of a quick edit (REQ-007-25)", () => {
  it("fills the form from the object read, or with defaults", () => {
    const prefix = { value: "10.1.0.0/24", nature: "network", anycast: true, domain: { id: "d" } };
    expect(initial(MODIFICATION.prefixes, prefix)).toEqual({
      value: "10.1.0.0/24",
      nature: "network",
      point_to_point: false,
      anycast: true,
    });
    expect(initial(CREATION.addresses, null)).toEqual({ domain: "", value: "", usage: "host" });
  });

  it("patches the changed members only, a domain as a reference", () => {
    const before = { value: "10.1.0.0/24", nature: "network", point_to_point: false };
    expect(
      patch(MODIFICATION.prefixes, before, { ...before, nature: "container", anycast: false }),
    ).toEqual({ nature: "container", anycast: false });
    expect(
      creation(CREATION.ranges, { domain: "d", first: "a", last: "b", usage: "dhcp" }),
    ).toEqual({ domain: { id: "d" }, first: "a", last: "b", usage: "dhcp" });
  });
});

describe("the answers of a write by resource (design « Écriture »)", () => {
  it("names the field of a member pointer, none for the whole body", () => {
    expect(fieldOf("/slug")).toBe("slug");
    expect(fieldOf("")).toBeNull();
    expect(fieldOf(null)).toBeNull();
    expect(fieldOf("/properties/core.owner")).toBeNull();
  });

  it("applied, pending, stale", async () => {
    expect(
      await outcomeOf(json(201, { changeset: { id: "c1" } }, { Location: "/v1/prefixes/p1" })),
    ).toEqual({ kind: "applied", changeset: "c1", location: "/v1/prefixes/p1" });
    expect(await outcomeOf(json(202, { changeset: { id: "c2" } }))).toEqual({
      kind: "pending",
      changeset: "c2",
    });
    expect(await outcomeOf(json(412, { type: "urn:kartenn:problem:precondition-failed" }))).toEqual(
      { kind: "stale" },
    );
  });

  it("puts a violation on its field, the others on the form", async () => {
    const outcome = await outcomeOf(
      json(422, {
        type: "urn:kartenn:problem:changeset-refused",
        violations: [
          { rule: "core.slug.syntax", severity: "error", detail: "bad slug", pointer: "/slug" },
          { rule: "core.x", severity: "error", detail: "whole", pointer: null },
        ],
      }),
    );
    expect(outcome).toEqual({
      kind: "invalid",
      byField: { slug: ["bad slug"] },
      general: ["whole"],
      type: "urn:kartenn:problem:changeset-refused",
    });
  });

  it("reads rights missing on the objects as a refusal", async () => {
    const outcome = await outcomeOf(
      json(422, {
        type: "urn:kartenn:problem:changeset-refused",
        violations: [
          {
            rule: "core.object.forbidden",
            severity: "error",
            detail: "1 object(s) need core.entity.write",
            pointer: "",
          },
        ],
      }),
    );
    expect(outcome).toMatchObject({
      kind: "refused",
      type: "urn:kartenn:problem:forbidden",
      detail: "1 object(s) need core.entity.write",
    });
  });

  it("keeps a refusal as a problem", async () => {
    expect(
      await outcomeOf(json(403, { type: "urn:kartenn:problem:forbidden", title: "Forbidden" })),
    ).toMatchObject({ kind: "refused", status: 403, type: "urn:kartenn:problem:forbidden" });
  });

  it("sends the motive, If-Match, the merge patch type and an idempotency key", async () => {
    const seen: RequestInit[] = [];
    const fetcher = (async (_url: RequestInfo | URL, init?: RequestInit) => {
      seen.push(init!);
      return json(200, { changeset: { id: "c" } });
    }) as typeof fetch;
    await submit(
      { method: "PATCH", url: "/v1/prefixes/p", body: { motive: "m" }, ifMatch: '"e1"' },
      fetcher,
    );
    await submit(
      { method: "POST", url: "/v1/prefixes", body: { motive: "m" }, idempotencyKey: "k" },
      fetcher,
    );
    const [modify, create] = seen.map((init) => init.headers as Record<string, string>);
    expect(modify).toMatchObject({
      "If-Match": '"e1"',
      "Content-Type": "application/merge-patch+json",
    });
    expect(modify).not.toHaveProperty("Idempotency-Key");
    expect(create).toMatchObject({ "Idempotency-Key": "k", "Content-Type": "application/json" });
  });
});
