import type { WidthClass } from "@kartenn/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { held, reviewActions, type Changeset } from "../src/changesets/model.ts";
import { lacking } from "../src/changesets/model.ts";
import { draftActions } from "../src/draft/state.ts";
import { Rights } from "../src/rights/index.ts";
import { offered, play, setViewAction, type ViewActionId } from "../src/shortcuts/view.ts";
import { rankedRefusal, refusalWords, timeOf, writesOf, type Writes } from "../src/time/index.ts";
import { collectionById } from "../src/views/collections.ts";
import { editability, editRight, EDITS, propertyRight } from "../src/views/edit/fields.ts";
import { moveEntryRefusal } from "../src/views/edit/moves.ts";

// One rank for every refusal of a write (task 021-21h, REQ-021-37): the lasting reason (a
// permission lacking, an object delivered, nothing to act in), then the state (the author
// only, held by a policy, a draft empty or in error), then the gate (`past` before
// `narrow`). The rank changes the sentence only: for each function it orders, every
// combination of the right, the period, the width, the author and a policy is refused
// exactly as before it (the implementations of main at 5dffa02a, copied here as oracles),
// and nothing refused before is offered now.

const PERIODS: Record<string, Record<string, string>> = {
  present: {},
  dated: { valid_at: "2026-01-01T00:00:00Z" },
  past: { as_of: "2026-06-30T12:00:00Z" },
  both: { valid_at: "2026-01-01T00:00:00Z", as_of: "2026-06-30T12:00:00Z" },
};
const WIDTHS: readonly WidthClass[] = ["wide", "standard", "compact", "narrow"];
/** Every gate of writes: each period at each width. */
const GATES: readonly { name: string; writes: Writes }[] = Object.entries(PERIODS).flatMap(
  ([period, query]) =>
    WIDTHS.map((width) => ({ name: `${period}@${width}`, writes: writesOf(timeOf(query), width) })),
);
const t = (key: string) => `<${key}>`;

describe("the rank of a refusal (rankedRefusal)", () => {
  it("says the lasting reason, then the state, then the gate; nothing when none", () => {
    expect(rankedRefusal({ lasting: "l", state: "s", gate: "g" })).toBe("l");
    expect(rankedRefusal({ lasting: null, state: "s", gate: "g" })).toBe("s");
    expect(rankedRefusal({ state: undefined, gate: "g" })).toBe("g");
    expect(rankedRefusal({ lasting: null, state: null, gate: null })).toBeUndefined();
    expect(rankedRefusal({})).toBeUndefined();
  });
});

describe("the keys `c` and `e` and the palette (play, offered)", () => {
  const remove: (() => void)[] = [];
  afterEach(() => remove.splice(0).forEach((r) => r()));

  /** `play` of main at 5dffa02a, `lasting` unset (every view but the administration). */
  function playBefore(
    action: { allowed: boolean; why: string | undefined; dated: boolean },
    id: ViewActionId,
    writes: Writes,
  ): { ran: boolean; said?: string; taken: boolean } {
    const fits = id === "filter" || (writes.draft && (writes.quickEdit || action.dated));
    if (action.allowed && fits) return { ran: true, taken: true };
    const why = action.allowed ? undefined : action.why;
    if (!action.allowed && why === undefined) return { ran: false, taken: false };
    const words = fits ? why : (refusalWords(writes, t) ?? t("edit.why.past"));
    return words ? { ran: false, said: words, taken: true } : { ran: false, taken: false };
  }

  it("runs and refuses exactly as before, the view's reason said before the gate", () => {
    const why = "Lecture seule : il faut core.prefix.edit";
    for (const id of ["create", "edit"] as const)
      for (const allowed of [true, false])
        for (const reason of [why, undefined])
          for (const dated of [true, false])
            for (const { name, writes } of GATES) {
              const said: string[] = [];
              const run = vi.fn();
              remove.push(
                setViewAction(id, {
                  label: () => "Agir",
                  run,
                  allowed: () => allowed,
                  why: () => reason,
                  say: (text) => said.push(text),
                  dated,
                }),
              );
              const before = playBefore({ allowed, why: reason, dated }, id, writes);
              const taken = play(id, writes, t);
              const label = `${id} allowed=${allowed} why=${reason} dated=${dated} ${name}`;
              // The verdict: the same run, the key taken alike.
              expect(run.mock.calls.length > 0, label).toBe(before.ran);
              expect(taken, label).toBe(before.taken);
              // The sentence: the view's reason first, whatever the gate.
              if (!allowed && reason !== undefined) expect(said, label).toEqual([reason]);
              else expect(said, label).toEqual(before.said ? [before.said] : []);
              remove.pop()!();
            }
  });

  it("keeps a command the gate alone holds, never one a right lacks", () => {
    for (const allowed of [true, false])
      for (const { name, writes } of GATES) {
        const said: string[] = [];
        const run = vi.fn();
        remove.push(
          setViewAction("edit", {
            label: () => "Modifier",
            run,
            allowed: () => allowed,
            why: () => "Il faut core.prefix.edit",
            say: (text) => said.push(text),
            dated: true,
          }),
        );
        const [period, width] = name.split("@") as [string, WidthClass];
        const time = timeOf(PERIODS[period]!);
        const command = offered("edit", time, width, t);
        const label = `allowed=${allowed} ${name}`;
        if (!allowed) expect(command, label).toBeNull();
        else if (writes.draft) expect(command?.run, label).toBe(run);
        else if (writes.refused === "narrow") {
          // Kept, its run says the width, nothing run.
          command!.run();
          expect(said, label).toEqual(["<edit.why.narrow>"]);
          expect(run, label).not.toHaveBeenCalled();
        } else expect(command, label).toBeNull();
        remove.pop()!();
      }
  });
});

describe("a cell in place (editability)", () => {
  const PREFIXES = collectionById("prefixes")!;
  const COLUMNS = ["value", "prop.core.owner", "domain", "id"];
  const RIGHTS = [
    new Rights(["core.prefix.edit", "prop.core.owner.edit"]),
    new Rights(["core.prefix.edit"]),
    new Rights(["prop.core.owner.edit"]),
    new Rights([]),
  ];

  /** `editability` of main at 5dffa02a: whether it offers an editor. */
  function editableBefore(rights: Rights, writes: Writes, column: string): boolean {
    const edit = EDITS[PREFIXES.id];
    const property = /^prop\.(.+)$/.exec(column)?.[1];
    if (edit.fixed[column]) return false;
    if (!property && !edit.modification.some((field) => field.name === column)) return false;
    if (!writes.draft) return false;
    if (!rights.has(property ? propertyRight(property) : editRight(PREFIXES.type))) return false;
    return writes.quickEdit;
  }

  it("offers an editor exactly as before, the permission said before the gate", () => {
    for (const rights of RIGHTS)
      for (const column of COLUMNS)
        for (const { name, writes } of GATES) {
          const now = editability(rights, writes, PREFIXES, column);
          const label = `${column} ${name} ${JSON.stringify(rights)}`;
          expect(now.editable, label).toBe(editableBefore(rights, writes, column));
          const property = /^prop\.(.+)$/.exec(column)?.[1];
          const permission = property ? propertyRight(property) : editRight(PREFIXES.type);
          const field = column === "value" || column === "prop.core.owner";
          if (field && !rights.has(permission))
            expect(now, label).toEqual({ editable: false, why: { kind: "forbidden", permission } });
        }
  });
});

describe("the decisions on a changeset (reviewActions)", () => {
  const changeset = (over: Partial<Changeset>): Changeset =>
    ({
      id: "c1",
      author: "bob",
      state: "pending",
      kind: "proposal",
      operations: [{ op: "update_prefix", id: { id: "p1" } }],
      ...over,
    }) as Changeset;
  const RIGHTS = [
    new Rights(["core.prefix.apply", "core.prefix.approval.self", "core.prefix.edit"]),
    new Rights(["core.prefix.apply"]),
    new Rights(["core.prefix.approval.self"]),
    new Rights(["core.prefix.edit"]),
    new Rights([]),
  ];

  /** `reviewActions` of main at 5dffa02a. */
  function before(
    c: Changeset,
    me: string,
    rights: Rights,
    refused: "past" | "narrow" | null,
  ): { id: string; forbidden?: string; reason?: string }[] {
    const author = c.author === me;
    const gate = refused === null ? null : { reason: refused };
    if (c.state === "pending") {
      const lack = lacking(rights, c.operations, author ? "approval.self" : "apply");
      const decide = gate
        ? gate
        : author && held(c)
          ? { reason: "held" }
          : lack
            ? { forbidden: lack }
            : {};
      return [
        { id: "validation", ...decide },
        ...(author ? [] : [{ id: "rejection", ...decide }]),
        { id: "withdrawal", ...(gate ?? (author ? {} : { reason: "author" })) },
      ];
    }
    if (c.state === "applied") {
      const edit = lacking(rights, c.operations, "edit");
      return [{ id: "undo", ...(gate ?? (edit ? { forbidden: edit } : {})) }];
    }
    return [];
  }
  const refusedIds = (actions: readonly { id: string; forbidden?: string; reason?: string }[]) =>
    actions.map((a) => `${a.id}:${a.forbidden !== undefined || a.reason !== undefined}`);

  it("refuses exactly the decisions it refused (REQ-024-11, 12), the lasting reason first", () => {
    for (const state of ["pending", "applied", "rejected"] as const)
      for (const kind of ["proposal", "approval"] as const)
        for (const policy of [undefined, "p1"])
          for (const me of ["bob", "alice"])
            for (const rights of RIGHTS)
              for (const refused of [null, "past", "narrow"] as const) {
                const c = changeset({ state, kind, ...(policy ? { policy } : {}) });
                const now = reviewActions(c, { me, rights, refused });
                const label = `${state} ${kind} ${policy} ${me} ${JSON.stringify(rights)} ${refused}`;
                // The same decisions, each refused or offered as before: none offered now.
                expect(refusedIds(now), label).toEqual(refusedIds(before(c, me, rights, refused)));
                // What refuses it with the gate open, it says under the gate too.
                const at = reviewActions(c, { me, rights, refused: null });
                for (const action of now) {
                  const open = at.find((a) => a.id === action.id)!;
                  if (open.forbidden || open.reason) expect(action, label).toEqual(open);
                }
                // The permission lacking before any other reason (the author, a policy).
                const verb =
                  state === "applied" ? "edit" : me === "bob" ? "approval.self" : "apply";
                const lack = lacking(rights, c.operations, verb);
                const decision = now.find((a) => a.id === "validation" || a.id === "undo");
                if (lack && decision) expect(decision.forbidden, label).toBe(lack);
              }
  });
});

describe("« Changer de parent… » (moveEntryRefusal)", () => {
  const words = { dated: "<edit.move.dated>", narrow: "<edit.why.narrow>" };
  /** `moveEntryRefusal` of main at 5dffa02a. */
  function before(refused: object, writes: Writes): object {
    if (writes.refused === "narrow") return { disabled: true, reason: words.narrow };
    return "forbidden" in refused || writes.quickEdit
      ? refused
      : { disabled: true, reason: words.dated };
  }
  const isRefused = (r: object) => "forbidden" in r || "disabled" in r;

  it("refuses exactly as before, the permission said before the gate", () => {
    for (const { name, writes } of GATES) {
      const gate = refusalWords(writes, t);
      const forbidden = { forbidden: "core.entity.edit" };
      // As the record gives it (`blockedBy`): the permission, else the gate, else nothing.
      for (const refused of [forbidden, gate ? { disabled: true, reason: gate } : {}]) {
        const now = moveEntryRefusal(refused, writes, words);
        expect(isRefused(now), name).toBe(isRefused(before(refused, writes)));
        if ("forbidden" in refused) expect(now, name).toBe(refused);
      }
    }
  });
});

describe("the split button of a draft (draftActions)", () => {
  /** The actions of `useDraftQueries` of main at 5dffa02a, by their refusal. */
  function before(
    forbidden: string | undefined,
    state: string | undefined,
    gate: string | undefined,
  ) {
    const blocked = gate ? gate : state;
    const refusedBy = (reason: string | undefined, lacks?: string) =>
      lacks && !reason
        ? { disabled: true, forbidden: lacks }
        : reason
          ? { disabled: true, reason }
          : {};
    return [refusedBy(blocked, forbidden), refusedBy(blocked), refusedBy(gate)];
  }

  it("refuses exactly as before, the permission, then the state, then the gate", () => {
    for (const forbidden of ["core.prefix.approval.self", undefined])
      for (const state of ["<drafts.why.empty>", undefined])
        for (const gate of ["<drafts.why.narrow>", "<drafts.why.past>", undefined]) {
          const now = draftActions(t, { forbidden, state, gate });
          const old = before(forbidden, state, gate);
          const label = `${forbidden} ${state} ${gate}`;
          expect(
            now.map((a) => a.disabled === true),
            label,
          ).toEqual(old.map((a) => "disabled" in a));
          const [apply, review, save] = now;
          const first = forbidden ?? state ?? gate;
          if (forbidden) expect(apply, label).toMatchObject({ forbidden });
          else if (first) expect(apply, label).toMatchObject({ reason: first });
          expect(review!.reason, label).toBe(state ?? gate);
          expect(save!.reason, label).toBe(gate);
        }
  });
});
