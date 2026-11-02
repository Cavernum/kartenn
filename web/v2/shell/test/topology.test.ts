import type { Operation, Simulated } from "@kartenn/sdk";
import type { TreeNode } from "@kartenn/ui";
import { describe, expect, it } from "vitest";
import { overlay, previewOf, type Preview, type Shift } from "../src/views/topology.ts";

// The preview of the draft in the tree of the topography (task 021-24b; REQ-021-17; 021/A12,
// precised by 24a): the places of the simulation laid over the tree served, an object the
// draft creates matched with its creation by the order of the contract, a parent the author
// may not read at the first level, a parent not read waiting, the truncation kept.

const DOMAIN = "d0000000-0000-4000-8000-000000000000";
const OTHER = "d0000000-0000-4000-8000-000000000001";
const ID = (n: number) => `0b5e3c1a-0000-4000-8000-00000000000${n}`;
const P = ID(1); // 198.51.100.0/24, served at the first level
const C1 = ID(2); // 198.51.100.0/26, under P
const C2 = ID(3); // 198.51.100.64/26, under P
const Q = ID(4); // 203.0.113.0/24, served at the first level
const TEMP = "7e000000-0000-4000-8000-000000000000"; // an identifier of the simulation alone
const TEMP2 = "7e000000-0000-4000-8000-000000000001";

type Placement = Simulated["placements"][number];
const placed = (object: string, before: string | null, after: string | null): Placement => ({
  object,
  kind: "prefix",
  before,
  after,
});
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
      simulated([placed(TEMP, null, P), placed(C1, P, TEMP), placed(C2, P, TEMP)]),
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

  it("matches a creation by the order of the operations, past those that move nothing", () => {
    // The first update keeps its parent (not placed); the second moves C2 under Q; the
    // creation after them, then the objects moved.
    const preview = previewOf(
      simulated([placed(C2, P, Q), placed(TEMP, null, Q), placed(C1, P, null)]),
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

  it("keeps the key of a creation from one simulation to the next", () => {
    const ops = [create("198.51.100.0/25")];
    const first = previewOf(simulated([placed(TEMP, null, P)]), ops, known, DOMAIN);
    const second = previewOf(simulated([placed(TEMP2, null, P)]), ops, known, DOMAIN);
    expect(first.shifts[0]!.key).toBe(second.shifts[0]!.key);
  });

  it("places a child moved with a parent that moves under that parent's row after", () => {
    // P inserted under a creation at the first level, C1 leaving P for the creation too.
    const preview = previewOf(
      simulated([placed(TEMP, null, null), placed(P, null, TEMP), placed(C1, P, TEMP)]),
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
      simulated([placed(P, null, Q), placed(C2, P, null)]),
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
          placed(TEMP, null, null),
          placed(TEMP2, null, null),
          { object: ID(9), kind: "entity", before: null, after: null },
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
});
