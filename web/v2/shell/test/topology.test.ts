import type { Operation, Simulated } from "@kartenn/sdk";
import type { TreeNode } from "@kartenn/ui";
import { describe, expect, it } from "vitest";
import {
  draftStateOf,
  overlay,
  previewOf,
  type Preview,
  type Shift,
} from "../src/views/topology.ts";

// The preview of the draft in the tree of the topography (task 021-24b; REQ-021-17; 021/A12,
// precised by 24a): the places of the simulation laid over the tree served, an object the
// draft creates matched with its creation by the index of its operation (A26, task 24c), a
// parent the author may not read at the first level, a parent not read waiting, the
// truncation kept.

const DOMAIN = "d0000000-0000-4000-8000-000000000000";
const OTHER = "d0000000-0000-4000-8000-000000000001";
const ID = (n: number) => `0b5e3c1a-0000-4000-8000-00000000000${n}`;
const P = ID(1); // 198.51.100.0/24, served at the first level
const C1 = ID(2); // 198.51.100.0/26, under P
const C2 = ID(3); // 198.51.100.64/26, under P
const Q = ID(4); // 203.0.113.0/24, served at the first level
const TEMP = "7e000000-0000-4000-8000-000000000000"; // an identifier of the simulation alone
const TEMP2 = "7e000000-0000-4000-8000-000000000001";
const MACHINE = "7e000000-0000-4000-8000-000000000002";

type Placement = Simulated["placements"][number];
/** A prefix placed, by operation `operation` of the draft, or by effect (`null`). */
const placed = (
  object: string,
  before: string | null,
  after: string | null,
  operation: number | null = null,
): Placement => ({ object, kind: "prefix", before, after, operation });
const simulated = (placements: Placement[], truncated = false) => ({
  placements,
  placements_truncated: truncated,
});
const create = (prefix: string, domain = DOMAIN) =>
  ({
    op: "create_prefix",
    domain: { id: domain },
    prefix,
    nature: "container",
    point_to_point: false,
    anycast: false,
  }) as unknown as Operation;
const update = (id: string, prefix: string) =>
  ({ op: "update_prefix", id: { id }, prefix }) as unknown as Operation;
const within = (operation: Operation) =>
  ({ op: "within", period: { start: 0, end: null }, operation }) as unknown as Operation;
const machine = { op: "machines.create_machine", name: "h1", kind: "physical" } as Operation;

const served: TreeNode[] = [
  {
    id: P,
    label: "198.51.100.0/24",
    children: [
      { id: C1, label: "198.51.100.0/26" },
      { id: C2, label: "198.51.100.64/26" },
    ],
  },
  { id: Q, label: "203.0.113.0/24", hasChildren: true },
];
const known = (id: string) => [P, C1, C2, Q].includes(id);
const LABELS: Record<string, string> = Object.fromEntries(
  served.flatMap((n) => [n, ...(n.children ?? [])]).map((n) => [n.id, n.label]),
);
const nodeOf = (shift: Shift, children: TreeNode[] | undefined): TreeNode => ({
  id: shift.key,
  label: shift.creation?.label ?? LABELS[shift.object!]!,
  ...(children ? { children } : {}),
});
/** The tree as rows « level label », depth first. */
const rows = (nodes: readonly TreeNode[], level = 1): string[] =>
  nodes.flatMap((n) => [`${level} ${n.label}`, ...rows(n.children ?? [], level + 1)]);
const laid = (preview: Preview) => overlay(served, preview, nodeOf);

describe("the preview of the draft in the tree of the prefixes", () => {
  it("shows a prefix inserted with its future children under it, marked where they are", () => {
    const preview = previewOf(
      simulated([placed(TEMP, null, P, 0), placed(C1, P, TEMP), placed(C2, P, TEMP)]),
      [create("198.51.100.0/25")],
      known,
      DOMAIN,
    );
    expect(preview.shifts.map((s) => s.key)).toEqual(["draft:0", `draft:${C1}`, `draft:${C2}`]);
    expect(preview.shifts[0]!.creation).toMatchObject({ label: "198.51.100.0/25", index: 0 });
    // Marked at their present place, which the tree served keeps.
    expect([...preview.moved]).toEqual([C1, C2]);
    const { nodes, shown } = laid(preview);
    expect(shown).toBe(3);
    expect(rows(nodes)).toEqual([
      "1 198.51.100.0/24",
      "2 198.51.100.0/26",
      "2 198.51.100.64/26",
      "2 198.51.100.0/25",
      "3 198.51.100.0/26",
      "3 198.51.100.64/26",
      "1 203.0.113.0/24",
    ]);
    // The nodes served are not written over (no optimism).
    expect(served[0]!.children).toHaveLength(2);
  });

  it("matches a creation by the index of its operation, past those that move nothing", () => {
    // The first update keeps its parent (not placed); the second moves C2 under Q; the
    // creation after them, then the objects moved.
    const preview = previewOf(
      simulated([placed(C2, P, Q, 1), placed(TEMP, null, Q, 2), placed(C1, P, null)]),
      [update(Q, "203.0.113.0/24"), update(C2, "203.0.113.64/26"), create("203.0.113.0/25")],
      known,
      DOMAIN,
    );
    expect(preview.shifts.map((s) => [s.key, s.parent])).toEqual([
      [`draft:${C2}`, Q],
      ["draft:2", Q],
      [`draft:${C1}`, null],
    ]);
    // Q's children are not read yet: its rows of the preview wait for them.
    const { nodes, shown } = laid(preview);
    expect(shown).toBe(1);
    expect(rows(nodes)).toContain("1 198.51.100.0/26");
  });

  it("says each row: moved at its present place, preview at its place after, draft otherwise", () => {
    // A /25 inserted above C1 and C2; Q's value changed in place (not moved).
    const preview = previewOf(
      simulated([placed(TEMP, null, P, 0), placed(C1, P, TEMP), placed(C2, P, TEMP)]),
      [create("198.51.100.0/25"), update(Q, "203.0.113.0/24")],
      known,
      DOMAIN,
    );
    const inDraft = new Set([Q, C1]);
    const states = (list: readonly TreeNode[]): [string, string | undefined][] =>
      list.flatMap((n) => [
        [n.id, draftStateOf(n.id, inDraft, preview)],
        ...states(n.children ?? []),
      ]);
    expect(states(laid(preview).nodes)).toEqual([
      [P, undefined],
      [C1, "moved"],
      [C2, "moved"],
      ["draft:0", "preview"],
      [`draft:${C1}`, "preview"],
      [`draft:${C2}`, "preview"],
      [Q, "draft"],
    ]);
    // Before the simulation answers, a node the draft changes is in the draft only.
    expect(draftStateOf(C1, inDraft, undefined)).toBe("draft");
    expect(draftStateOf(C2, inDraft, undefined)).toBeUndefined();
  });

  it("says moved a node whose parent after is not read, and preview a child taken along", () => {
    // P moved under Q, C1 along with it (not placed), C2 left at the first level.
    const preview = previewOf(
      simulated([placed(P, null, Q, 0), placed(C2, P, null)]),
      [update(P, "203.0.113.0/25")],
      known,
      DOMAIN,
    );
    const inDraft = new Set([P, C1]);
    // Q's children not read: P waits (counted in the notice), moved at its present place.
    expect(laid(preview).shown).toBe(1);
    expect(draftStateOf(P, inDraft, preview)).toBe("moved");
    // C1 keeps its parent: at its present place in the draft only; its copy, a preview row.
    expect(draftStateOf(C1, inDraft, preview)).toBe("draft");
    expect(draftStateOf(`draft:${C1}`, inDraft, preview)).toBe("preview");
  });

  it("keeps the key of a creation from one simulation to the next", () => {
    const ops = [create("198.51.100.0/25")];
    const first = previewOf(simulated([placed(TEMP, null, P, 0)]), ops, known, DOMAIN);
    const second = previewOf(simulated([placed(TEMP2, null, P, 0)]), ops, known, DOMAIN);
    expect(first.shifts[0]!.key).toBe(second.shifts[0]!.key);
  });

  it("places a child moved with a parent that moves under that parent's row after", () => {
    // P inserted under a creation at the first level, C1 leaving P for the creation too.
    const preview = previewOf(
      simulated([placed(TEMP, null, null, 0), placed(P, null, TEMP), placed(C1, P, TEMP)]),
      [create("198.51.0.0/16")],
      known,
      DOMAIN,
    );
    // P takes along the child it keeps (C2); C1 goes under the creation itself.
    expect(rows(laid(preview).nodes).slice(-4)).toEqual([
      "1 198.51.0.0/16",
      "2 198.51.100.0/24",
      "3 198.51.100.64/26",
      "2 198.51.100.0/26",
    ]);
  });

  it("takes along the children a moved prefix keeps, as rows of the preview", () => {
    // P moved under Q (its value changed): C1 and C2 follow it, not placed; C2 leaves it.
    const preview = previewOf(
      simulated([placed(P, null, Q, 0), placed(C2, P, null)]),
      [update(P, "203.0.113.0/25")],
      known,
      DOMAIN,
    );
    const open: TreeNode[] = [served[0]!, { ...served[1]!, hasChildren: false, children: [] }];
    const { nodes } = overlay(open, preview, nodeOf);
    const q = nodes.find((n) => n.id === Q)!;
    expect(q.children!.map((n) => n.id)).toEqual([`draft:${P}`]);
    // C1 along with P; C2, placed elsewhere, not twice.
    expect(q.children![0]!.children!.map((n) => n.id)).toEqual([`draft:${C1}`]);
    expect(nodes.at(-1)!.id).toBe(`draft:${C2}`);
  });

  it("leaves out what is not of this tree, and keeps the truncation", () => {
    const preview = previewOf(
      simulated(
        [
          placed(TEMP, null, null, 0),
          placed(TEMP2, null, null, 1),
          { object: ID(9), kind: "entity", before: null, after: null, operation: null },
          placed(ID(8), ID(7), ID(6)),
        ],
        true,
      ),
      [create("192.0.2.0/24", OTHER), create("198.51.100.0/23")],
      known,
      DOMAIN,
    );
    // The creation of another domain, an entity, an object not read here: none shown.
    expect(preview.shifts.map((s) => s.key)).toEqual(["draft:1"]);
    expect(preview.truncated).toBe(true);
  });

  it("shows under the first level an object whose parent after the author may not read", () => {
    // `after` `null`: at the root, or a parent hidden — the top of the tree it reads.
    const preview = previewOf(simulated([placed(C1, P, null)]), [], known, DOMAIN);
    expect(rows(laid(preview).nodes).at(-1)).toBe("1 198.51.100.0/26");
  });

  it("labels a creation after a module creation, which the order would mismatch", () => {
    // The machine first, placed from no parent too, then the prefix: each by its index.
    const preview = previewOf(
      simulated([
        { object: MACHINE, kind: "machines.machine", before: null, after: null, operation: 0 },
        placed(TEMP, null, P, 1),
      ]),
      [machine, create("198.51.100.128/25")],
      known,
      DOMAIN,
    );
    expect(preview.shifts.map((s) => s.key)).toEqual(["draft:1"]);
    expect(rows(laid(preview).nodes)).toContain("2 198.51.100.128/25");
  });

  it("labels a creation under `within` by the index of its envelope", () => {
    const preview = previewOf(
      simulated([placed(TEMP, null, P, 0), placed(TEMP2, null, Q, 1)]),
      [within(create("198.51.100.128/25")), create("203.0.113.128/25")],
      known,
      DOMAIN,
    );
    expect(preview.shifts.map((s) => [s.key, s.creation?.label, s.parent])).toEqual([
      ["draft:0", "198.51.100.128/25", P],
      ["draft:1", "203.0.113.128/25", Q],
    ]);
    const open: TreeNode[] = [served[0]!, { ...served[1]!, hasChildren: false, children: [] }];
    expect(rows(overlay(open, preview, nodeOf).nodes)).toEqual([
      "1 198.51.100.0/24",
      "2 198.51.100.0/26",
      "2 198.51.100.64/26",
      "2 198.51.100.128/25",
      "1 203.0.113.0/24",
      "2 203.0.113.128/25",
    ]);
  });
});
