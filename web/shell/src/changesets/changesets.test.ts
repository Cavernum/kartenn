import { describe, expect, it } from "vitest";
import { actionsOf, changesetCell, operationText } from "./changesets.ts";

describe("actions offered on a changeset (REQ-007-18)", () => {
  it("a pending changeset is validated or rejected by someone else", () => {
    expect(actionsOf({ state: "pending", author: "bob" }, "alice", true)).toEqual([
      "validation",
      "rejection",
    ]);
  });

  it("a pending changeset is withdrawn by its author, who cannot validate it", () => {
    expect(actionsOf({ state: "pending", author: "bob" }, "bob", true)).toEqual(["withdrawal"]);
  });

  it("an applied changeset is undone", () => {
    expect(actionsOf({ state: "applied", author: "bob" }, "alice", true)).toEqual(["undo"]);
  });

  it("an ended changeset has no action", () => {
    for (const state of ["refused", "rejected", "withdrawn", "expired", "failed_revalidation"]) {
      expect(actionsOf({ state, author: "bob" }, "alice", true), state).toEqual([]);
    }
  });

  it("none under a past as_of, nor without a session", () => {
    expect(actionsOf({ state: "applied", author: "bob" }, "alice", false)).toEqual([]);
    expect(actionsOf({ state: "pending", author: "bob" }, null, true)).toEqual([]);
  });
});

describe("cells of the lists of changesets", () => {
  const row = { author: "me", motive: "m", required: [{ permission: "core.address.write" }] };

  it("names the caller and lists the missing permissions", () => {
    expect(changesetCell(row, "author", "me", "you")).toBe("you");
    expect(changesetCell(row, "author", "other", "you")).toBe("me");
    expect(changesetCell(row, "required", "me", "you")).toBe("core.address.write");
    expect(changesetCell(row, "absent", "me", "you")).toBe("—");
  });

  it("splits an operation into its name and its members", () => {
    expect(operationText({ op: "create_address", address: "10.0.0.1" })).toEqual({
      op: "create_address",
      rest: '{"address":"10.0.0.1"}',
    });
  });
});
