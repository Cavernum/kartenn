// @vitest-environment happy-dom
import { ApiProblem, drafts, queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import type { GridColumn } from "@kartenn/ui";
import { afterEach, describe, expect, it } from "vitest";
import {
  createApp,
  defineComponent,
  effectScope,
  h,
  nextTick,
  ref,
  shallowRef,
  type App,
} from "vue";
import { closeEditWithPage, editRequest, openEdit } from "../src/views/edit/panel.ts";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { createShellI18n } from "../src/i18n/index.ts";
import { loadSession, Rights, type Me } from "../src/rights/index.ts";
import { collectionById, type Item } from "../src/views/collections.ts";
import {
  approvalOf,
  changesOf,
  checkValue,
  draftedClosures,
  draftedValues,
  editability,
  editRight,
  EDITS,
  fieldOfPointer,
  initialValues,
  lackingRight,
  patchOf,
} from "../src/views/edit/fields.ts";
import {
  useInlineEdit,
  type InlineDeps,
  type InlineEdit,
  type InlineSay,
} from "../src/views/edit/inline.ts";
import { useNodeEdit } from "../src/views/edit/nodes.ts";
import {
  fieldErrors,
  NETWORK_VLAN_EDITS,
  ofAssignment,
  vlanForNetwork,
  vlanForNetworkInDraft,
} from "../src/views/edit/vlan.ts";

// In-place editing and the edit panel of a kind (task 021-12; REQ-021-15, 16, 36, 37): the
// fields of each kind typed by the contract, the operations of the draft they give, whether
// a cell may be edited and why not; then a list edited in place against a fake API — Enter
// to the draft, Ctrl+Enter applied with its motive, 202 in review and never applied, 412
// the input kept beside the value read again, no apply without `X.approval.self`.

const PREFIXES = collectionById("prefixes")!;
/** What may be written at the present, under `valid_at` alone, under `as_of` (`writesOf`). */
const NOW = { quickEdit: true, draft: true };
const DATED = { quickEdit: false, draft: true };
const AS_OF = { quickEdit: false, draft: false };
const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";
const V1 = "0b5e3c1a-0000-4000-8000-0000000000c1";
const V2 = "0b5e3c1a-0000-4000-8000-0000000000c2";
const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";

describe("the fields of an edit", () => {
  const author = new Rights(["core.prefix.edit", "prop.core.owner.edit"]);
  const reader = new Rights(["core.prefix.read"]);

  it("let a cell be edited with the right, at the present, when the patch takes it", () => {
    expect(editability(author, NOW, PREFIXES, "value")).toEqual({ editable: true });
    expect(editability(author, NOW, PREFIXES, "nature")).toEqual({ editable: true });
    // A reader: no editor, the permission named as served.
    expect(editability(reader, NOW, PREFIXES, "value")).toEqual({
      editable: false,
      why: { kind: "forbidden", permission: "core.prefix.edit" },
    });
    // Under `as_of`, nothing is written; under `valid_at` alone, the dated draft by the
    // edit panel (« Modifier »), never in place.
    expect(editability(author, AS_OF, PREFIXES, "value")).toEqual({
      editable: false,
      why: { kind: "past" },
    });
    expect(editability(author, DATED, PREFIXES, "value")).toEqual({
      editable: false,
      why: { kind: "dated" },
    });
    // A reader is told the permission there, not a dated draft he may not write.
    expect(editability(reader, DATED, PREFIXES, "value")).toEqual({
      editable: false,
      why: { kind: "forbidden", permission: "core.prefix.edit" },
    });
    expect(editability(reader, AS_OF, PREFIXES, "value")).toEqual({
      editable: false,
      why: { kind: "past" },
    });
    // The domain never changes after creation; a computed value never does.
    expect(editability(author, NOW, PREFIXES, "domain")).toMatchObject({
      why: { kind: "fixed", reason: "immutable" },
    });
    expect(editability(author, NOW, PREFIXES, "public")).toMatchObject({
      why: { kind: "fixed", reason: "computed" },
    });
    expect(editability(author, NOW, PREFIXES, "id")).toMatchObject({ why: { kind: "none" } });
  });

  it("ask `prop.<key>.edit` for a property, apart from the right to modify the object", () => {
    // The rule (a list offers no editor until it writes properties: `useInlineEdit.why`).
    expect(editability(author, NOW, PREFIXES, "prop.core.owner")).toEqual({ editable: true });
    expect(editability(author, NOW, PREFIXES, "prop.dns.ttl")).toEqual({
      editable: false,
      why: { kind: "forbidden", permission: "prop.dns.ttl.edit" },
    });
    // REQ-003-17: the assignment asks the property's permission, not `X.edit` of the object.
    const owner = new Rights(["prop.core.owner.edit"]);
    expect(editability(owner, NOW, PREFIXES, "prop.core.owner")).toEqual({ editable: true });
    expect(editability(owner, NOW, PREFIXES, "value")).toEqual({
      editable: false,
      why: { kind: "forbidden", permission: "core.prefix.edit" },
    });
  });

  it("give the operations of the contract, the version read as `expected`", () => {
    expect(EDITS.prefixes.update(P1, V1, { value: "192.0.2.0/25" })).toEqual({
      op: "update_prefix",
      id: { id: P1 },
      prefix: "192.0.2.0/25",
      expected: V1,
    });
    expect(EDITS.domains.update(P1, null, { slug: "nord" })).toEqual({
      op: "rename",
      kind: "domain",
      id: { id: P1 },
      slug: "nord",
    });
    expect(EDITS.addresses.create({ domain: D1, value: "192.0.2.10", usage: "host" })).toEqual({
      op: "create_address",
      domain: { id: D1 },
      address: "192.0.2.10",
      usage: "host",
    });
    expect(EDITS.entities.create({ slug: "nord", parent: "" })).toEqual({
      op: "create_entity",
      slug: "nord",
    });
  });

  it("read the values a draft gives the objects of a collection", () => {
    const operations = [
      { op: "update_prefix", id: { id: P1 }, prefix: "192.0.2.0/25", expected: V1 },
      { op: "update_prefix", id: { id: P1 }, nature: "container" },
      { op: "rename", kind: "domain", id: { id: P1 }, slug: "nord" },
      { op: "update_address", id: { id: D1 }, usage: "host" },
    ] as never;
    expect(draftedValues(operations, "prefixes")).toEqual(
      new Map([[P1, { value: "192.0.2.0/25", nature: "container" }]]),
    );
    expect(draftedValues(operations, "domains").get(P1)).toEqual({ slug: "nord" });
  });

  it("read the objects of a collection a draft closes, a range replaced among them", () => {
    // P1 a range replaced (closed, then created with other bounds); D1 a prefix closed; V1
    // a range closed under another period, not the present's.
    const operations = [
      { op: "close", kind: "range", id: { id: P1 }, expected: V2 },
      { op: "create_range", domain: { id: D1 }, first: "192.0.2.10", last: "192.0.2.40" },
      { op: "close", kind: "prefix", id: { id: D1 } },
      {
        op: "within",
        period: { start: "2026-12-01T00:00:00Z", end: null },
        operation: { op: "close", kind: "range", id: { id: V1 } },
      },
    ] as never;
    expect(draftedClosures(operations, "ranges")).toEqual(new Set([P1]));
    expect(draftedClosures(operations, "prefixes")).toEqual(new Set([D1]));
    expect(draftedClosures(operations, "addresses").size).toBe(0);
    expect(draftedClosures(undefined, "ranges").size).toBe(0);
    // A closure changes no value.
    expect(draftedValues(operations, "ranges").size).toBe(0);
  });

  it("check a value by the types of the contract, then write only what changed", () => {
    const [, value, nature] = EDITS.prefixes.creation;
    expect(checkValue(value!, " ")).toBe("edit.check.required");
    expect(checkValue(nature!, "lan")).toBe("edit.check.choice");
    expect(checkValue(nature!, "network")).toBeUndefined();
    const fields = EDITS.prefixes.modification;
    const read = { value: "192.0.2.0/24", nature: "network", point_to_point: false, anycast: true };
    const before = initialValues(fields, read);
    expect(before).toEqual(read);
    const changes = changesOf(fields, before, { ...before, nature: "container" });
    expect(changes).toEqual({ nature: "container" });
    expect(patchOf(fields, changes)).toEqual({ nature: "container" });
    // A creation: the first choice of a closed list, the values given beforehand.
    expect(initialValues(EDITS.prefixes.creation, null, { domain: D1 })).toEqual({
      domain: D1,
      value: "",
      nature: "network",
      point_to_point: false,
      anycast: false,
    });
    expect(patchOf(EDITS.prefixes.creation, { domain: D1 })).toEqual({ domain: { id: D1 } });
  });

  it("write the VLAN and their groups as the contract does (026): numbers, anchors", () => {
    const VLANS = collectionById("vlans")!;
    const GROUPS = collectionById("vlan-groups")!;
    expect(
      EDITS.vlans.create({ group: D1, vid: "105", name: "impression", status: "active" }),
    ).toEqual({
      op: "create_vlan",
      group: { id: D1 },
      vid: 105,
      name: "impression",
      status: "active",
    });
    // No `expected`: the contract has none for the operations of the VLAN.
    expect(EDITS.vlans.update(P1, V1, { vid: "106" })).toEqual({
      op: "update_vlan",
      id: { id: P1 },
      vid: 106,
    });
    // An anchor of the form (`<kind>:<id>`); a range left empty is the contract's default.
    expect(
      EDITS["vlan-groups"].create({
        slug: "campus",
        anchor: `site:${D1}`,
        vid_min: "100",
        vid_max: "",
      }),
    ).toEqual({
      op: "create_vlan_group",
      slug: "campus",
      anchor: { site: { id: D1 } },
      vid_min: 100,
    });
    // Read back: an anchor and a VID as the form holds them; a cell of a reference is chosen
    // in the panel, never typed.
    const group = { slug: "campus", anchor: { domain: { id: D1 } }, vid_min: 100, vid_max: 199 };
    expect(initialValues(EDITS["vlan-groups"].modification, group)).toEqual({
      slug: "campus",
      anchor: `domain:${D1}`,
      vid_min: "100",
      vid_max: "199",
    });
    expect(
      patchOf(EDITS["vlan-groups"].modification, { anchor: `entity:${D1}`, vid_max: "150" }),
    ).toEqual({ anchor: { entity: { id: D1 } }, vid_max: 150 });
    const author = new Rights(["core.vlan.edit", "core.vlan_group.edit"]);
    expect(editability(author, NOW, VLANS, "vid")).toEqual({ editable: true });
    expect(editability(author, NOW, VLANS, "group")).toMatchObject({
      why: { kind: "fixed", reason: "reference" },
    });
    expect(editability(author, NOW, GROUPS, "anchor")).toMatchObject({
      why: { kind: "fixed", reason: "reference" },
    });
    const [, vid] = EDITS.vlans.creation;
    expect(checkValue(vid!, "10a")).toBe("edit.check.integer");
    expect(checkValue(vid!, "100")).toBeUndefined();
    // A draft's VID, as the grid reads it.
    expect(
      draftedValues(
        [{ op: "update_vlan", id: { id: P1 }, vid: 107, name: "voix" }] as never,
        "vlans",
      ),
    ).toEqual(new Map([[P1, { vid: "107", name: "voix" }]]));
    // A violation of the creation applied at once, on the field that names the VLAN.
    expect(fieldOfPointer("vlans", "/operations/0")).toBe("vid");
  });
});

describe("a list edited in place", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
  });

  const ROW: Item = { id: P1, value: "192.0.2.0/24", nature: "network", version: V1 };
  const column = (id: string): GridColumn => ({ id, label: id });
  interface Seen {
    method: string;
    url: string;
    body: unknown;
    ifMatch: string | null;
  }

  /** The composable mounted at the present, the API answered by `answer`. */
  async function mounted(
    permissions: string[],
    answer: (seen: Seen) => { status: number; body: unknown; etag?: string },
    at = "/",
    /** Another editing on the same rows (the nodes of the topography). */
    make?: (deps: Omit<InlineDeps, "collection">) => InlineEdit,
  ) {
    const seen: Seen[] = [];
    const rows = shallowRef<Item[]>([ROW]);
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const headers = new Headers(init?.headers);
      const call = {
        method: init?.method ?? "GET",
        url: String(input),
        body: init?.body ? JSON.parse(String(init.body)) : null,
        ifMatch: headers.get("If-Match"),
      };
      seen.push(call);
      const { status, body, etag } = answer(call);
      const problem = status >= 400;
      return new Response(JSON.stringify(body), {
        status,
        headers: {
          "Content-Type": problem ? "application/problem+json" : "application/json",
          ...(etag ? { ETag: etag } : {}),
        },
      });
    }) as typeof fetch);
    await loadSession(async (url) =>
      url === "/v1/me"
        ? ({
            identity: "alice",
            kind: "person",
            global_permissions: permissions,
            bindings: [],
          } as unknown as Me)
        : null,
    );
    const said: { applied: string[]; proposed: unknown[]; status: string[] } = {
      applied: [],
      proposed: [],
      status: [],
    };
    const say: InlineSay = {
      applied: (what) => said.applied.push(what),
      proposed: (what, changeset, reason) => said.proposed.push({ what, changeset, reason }),
      status: (text) => said.status.push(text),
    };
    let edit!: InlineEdit;
    const Host = defineComponent({
      setup() {
        const deps = {
          rows: () => rows.value,
          t: (key: string, values?: Record<string, unknown>) =>
            `${key}${values ? JSON.stringify(values) : ""}`,
          sentence: (error: unknown) =>
            error instanceof ApiProblem ? `said:${error.code}` : "said:network",
          say,
        };
        edit = make ? make(deps) : useInlineEdit({ collection: () => PREFIXES, ...deps });
        return () => h("div");
      },
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: Host }],
    });
    await router.push(at);
    app = createApp({ render: () => h(RouterView) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(document.createElement("div"));
    await router.isReady();
    await nextTick();
    return { edit: () => edit, seen, said, router, rows };
  }

  const draftBody = (operations: unknown[]) => ({
    id: D1,
    title: "Brouillon",
    version: 1,
    operation_count: operations.length,
    operations,
  });

  it("Enter adds an operation to the draft, the cell then « en brouillon »", async () => {
    const { edit, seen } = await mounted(["core.prefix.edit"], (call) =>
      call.method === "POST" && call.url === "/v1/drafts"
        ? {
            status: 201,
            body: draftBody((call.body as { operations: unknown[] }).operations),
            etag: '"1"',
          }
        : { status: 200, body: { items: [], links: {} } },
    );
    expect(edit().editable(ROW, column("value"))).toBe(true);
    edit().onCommit({ row: P1, column: "value", value: "192.0.2.0/25" });
    expect(edit().cellState(ROW, column("value"))).toBe("pending");
    await expect.poll(() => edit().cellState(ROW, column("value"))).toBe("draft");
    expect(edit().rowState(ROW)).toBe("draft");
    // A row in a draft is held by no one: no reason to say (task 021-12n).
    expect(edit().rowReason(ROW)).toBeUndefined();
    expect(edit().shownRows.value[0]!.value).toBe("192.0.2.0/25");
    const post = seen.find((s) => s.method === "POST")!;
    expect((post.body as { operations: unknown[] }).operations).toEqual([
      { op: "update_prefix", id: { id: P1 }, prefix: "192.0.2.0/25", expected: V1 },
    ]);
  });

  it("a reader has no editor; Ctrl+Enter without approval.self goes to the draft", async () => {
    const reader = await mounted([], () => ({ status: 200, body: { items: [], links: {} } }));
    expect(reader.edit().editable(ROW, column("value"))).toBe(false);
    expect(reader.edit().why("value")).toEqual({
      kind: "forbidden",
      permission: "core.prefix.edit",
    });
    app?.unmount();
    const author = await mounted(["core.prefix.edit"], (call) =>
      call.method === "POST"
        ? { status: 201, body: draftBody([]), etag: '"1"' }
        : { status: 200, body: { items: [], links: {} } },
    );
    expect(author.edit().canApply.value).toBe(false);
    author.edit().onCommit({ row: P1, column: "nature", value: "container", now: true });
    expect(author.edit().asking.value).toBeNull();
    await expect.poll(() => author.seen.some((s) => s.url === "/v1/drafts")).toBe(true);
    // One sentence: why not now, and that it went to the draft.
    await expect.poll(() => author.said.status[0] ?? "").toContain("core.prefix.approval.self");
    expect(author.said.status[0]).toContain("edit.drafted");
  });

  it("Ctrl+Enter applies with its motive: 202 « en revue », never applied", async () => {
    const { edit, seen, said } = await mounted(
      ["core.prefix.edit", "core.prefix.approval.self"],
      (call) =>
        call.method === "PATCH"
          ? {
              status: 202,
              body: {
                changeset: { id: "cs-1", href: "/v1/changesets/cs-1" },
                state: "pending",
                kind: "approval",
                diff: [],
                reason: { code: "changeset.approval.self", message: "policy", objects: [] },
              },
            }
          : { status: 200, body: { items: [], links: {} } },
    );
    edit().onCommit({ row: P1, column: "nature", value: "container", now: true });
    expect(edit().asking.value).toMatchObject({ row: P1, column: "nature", value: "container" });
    expect(await edit().confirm("bout en bout", "container")).toBe(true);
    const patch = seen.find((s) => s.method === "PATCH")!;
    expect(patch.url).toBe(`/v1/prefixes/${P1}?approval=self`);
    expect(patch.ifMatch).toBe(`"${V1}"`);
    expect(patch.body).toEqual({ nature: "container", motive: "bout en bout" });
    expect(edit().cellState(ROW, column("nature"))).toBe("review");
    expect(edit().rowState(ROW)).toBe("review");
    // The cell says why it is held (REQ-021-15; the catalog's sentence, its key here), the
    // other cells nothing.
    expect(edit().stateReason(ROW, column("nature"))).toBe("edit.reason.held");
    expect(edit().stateReason(ROW, column("value"))).toBeUndefined();
    // Its row says it too, whichever column shows it (its first cell, task 021-12n).
    expect(edit().rowReason(ROW)).toEqual(["edit.reason.held"]);
    // The value read stays: the one proposed is not the object's.
    expect(edit().shownRows.value[0]!.nature).toBe("network");
    expect(said.applied).toEqual([]);
    expect(said.proposed).toMatchObject([
      { changeset: "cs-1", reason: { code: "changeset.approval.self" } },
    ]);
  });

  it("412: the input kept beside the value read again, applied again on the new version", async () => {
    let patches = 0;
    const { edit, seen, said } = await mounted(
      ["core.prefix.edit", "core.prefix.approval.self"],
      (call) => {
        if (call.method === "PATCH") {
          patches += 1;
          return patches === 1
            ? {
                status: 412,
                body: { type: "urn:kartenn:problem:precondition-failed", title: "", status: 412 },
              }
            : {
                status: 200,
                body: {
                  changeset: { id: "cs-2", href: "h" },
                  state: "applied",
                  warnings: [],
                  diff: [],
                  cascaded: [],
                },
              };
        }
        if (call.url.startsWith(`/v1/prefixes/${P1}`)) {
          return {
            status: 200,
            body: { ...ROW, nature: "container", version: V2 },
            etag: `"${V2}.0"`,
          };
        }
        return { status: 200, body: { items: [], links: {} } };
      },
    );
    edit().onCommit({ row: P1, column: "value", value: "192.0.2.0/25", now: true });
    expect(await edit().confirm("premier essai", "192.0.2.0/25")).toBe(false);
    expect(edit().asking.value).toMatchObject({
      value: "192.0.2.0/25",
      current: "192.0.2.0/24",
      version: V2,
      refusal: { kind: "conflict" },
    });
    // Nothing shown as written while it was refused.
    expect(edit().cellState(ROW, column("value"))).toBeUndefined();
    expect(await edit().confirm("second essai", "192.0.2.0/25")).toBe(true);
    expect(seen.filter((s) => s.method === "PATCH").map((s) => s.ifMatch)).toEqual([
      `"${V1}"`,
      `"${V2}"`,
    ]);
    expect(said.applied).toHaveLength(1);
  });

  it("422: the violation on the cell with its code, the input kept in the line", async () => {
    const { edit } = await mounted(["core.prefix.edit", "core.prefix.approval.self"], (call) =>
      call.method === "PATCH"
        ? {
            status: 422,
            body: {
              type: "urn:kartenn:problem:invalid-changeset",
              title: "",
              status: 422,
              violations: [
                {
                  rule: "core.prefix.unaligned",
                  severity: "error",
                  detail: "Non aligné.",
                  pointer: "/value",
                },
              ],
            },
          }
        : { status: 200, body: { items: [], links: {} } },
    );
    edit().onCommit({ row: P1, column: "value", value: "192.0.2.1/24", now: true });
    expect(await edit().confirm("essai", "192.0.2.1/24")).toBe(false);
    expect(edit().cellState(ROW, column("value"))).toEqual({
      error: "Non aligné.",
      code: "core.prefix.unaligned",
    });
    expect(edit().asking.value).toMatchObject({
      value: "192.0.2.1/24",
      refusal: { kind: "invalid" },
    });
    edit().cancel();
    expect(edit().cellState(ROW, column("value"))).toBeUndefined();
  });

  const lists = { status: 200, body: { items: [], links: {} } };
  const APPLY = ["core.prefix.edit", "core.prefix.approval.self"];

  it("200: the value applied shown until the list is read again, its changeset linked", async () => {
    const { edit, said, rows } = await mounted(APPLY, (call) =>
      call.method === "PATCH"
        ? {
            status: 200,
            body: {
              changeset: { id: "cs-3", href: "h" },
              state: "applied",
              warnings: [],
              diff: [],
              cascaded: [],
            },
          }
        : lists,
    );
    edit().onCommit({ row: P1, column: "nature", value: "container", now: true });
    expect(await edit().confirm("essai", "container")).toBe(true);
    expect(said.applied).toEqual(['edit.what{"field":"views.field.nature","name":"192.0.2.0/24"}']);
    expect(edit().shownRows.value[0]!.nature).toBe("container");
    expect(edit().cellState(ROW, column("nature"))).toBeUndefined();
    // Read again at its new version: the entry gives way to the row.
    rows.value = [{ ...ROW, nature: "container", version: V2 }];
    await nextTick();
    expect(edit().shownRows.value[0]).toBe(rows.value[0]);
  });

  it("403: a refusal, the input kept in the line, nothing shown as written", async () => {
    const { edit } = await mounted(APPLY, (call) =>
      call.method === "PATCH"
        ? {
            status: 403,
            body: { type: "urn:kartenn:problem:forbidden", title: "Refusé", status: 403 },
          }
        : lists,
    );
    edit().onCommit({ row: P1, column: "value", value: "192.0.2.0/25", now: true });
    expect(await edit().confirm("essai", "192.0.2.0/25")).toBe(false);
    expect(edit().asking.value).toMatchObject({
      value: "192.0.2.0/25",
      refusal: { kind: "forbidden" },
    });
    expect(edit().cellState(ROW, column("value"))).toBeUndefined();
    expect(edit().shownRows.value[0]!.value).toBe("192.0.2.0/24");
  });

  it("off the present: no editor, no draft, and the line of the motive closes unwritten", async () => {
    // Under `valid_at` alone the cell says the dated draft (« Modifier »), not the present.
    const past = await mounted(APPLY, () => lists, "/?valid_at=2026-01-01T00:00:00Z");
    expect(past.edit().why("value")).toEqual({ kind: "dated" });
    past.edit().onCommit({ row: P1, column: "value", value: "192.0.2.0/25" });
    await nextTick();
    expect(past.seen.filter((s) => s.method !== "GET")).toEqual([]);
    app?.unmount();
    const now = await mounted(APPLY, () => lists);
    now.edit().onCommit({ row: P1, column: "value", value: "192.0.2.0/25", now: true });
    expect(now.edit().asking.value).not.toBeNull();
    await now.router.push("/?as_of=2026-01-01T00:00:00Z");
    await nextTick();
    expect(now.edit().why("value")).toEqual({ kind: "past" });
    expect(now.edit().asking.value).toBeNull();
    expect(await now.edit().confirm("essai", "192.0.2.0/25")).toBe(false);
    expect(now.seen.filter((s) => s.method !== "GET")).toEqual([]);
  });

  it("a 412 left open is never replaced by Ctrl+Enter on another cell, said with its input", async () => {
    const { edit, said } = await mounted(APPLY, (call) =>
      call.method === "PATCH"
        ? {
            status: 412,
            body: { type: "urn:kartenn:problem:precondition-failed", title: "", status: 412 },
          }
        : call.url.startsWith(`/v1/prefixes/${P1}`)
          ? { status: 200, body: { ...ROW, version: V2 }, etag: `"${V2}.0"` }
          : lists,
    );
    edit().onCommit({ row: P1, column: "value", value: "192.0.2.0/25", now: true });
    await edit().confirm("essai", "192.0.2.0/25");
    edit().onCommit({ row: P1, column: "nature", value: "container", now: true });
    expect(edit().asking.value).toMatchObject({ column: "value", value: "192.0.2.0/25" });
    expect(said.status.at(-1)).toContain('"value":"container"');
    expect(edit().cellState(ROW, column("nature"))).toMatchObject({
      error: expect.stringContaining("edit.askOpen"),
    });
  });

  it("an error of a cell goes once its row is read again at another version", async () => {
    const { edit, rows } = await mounted(["core.prefix.edit"], (call) =>
      call.method === "POST"
        ? {
            status: 422,
            body: { type: "urn:kartenn:problem:invalid", title: "", status: 422, violations: [] },
          }
        : lists,
    );
    edit().onCommit({ row: P1, column: "value", value: "192.0.2.0/25" });
    await expect.poll(() => typeof edit().cellState(ROW, column("value"))).toBe("object");
    // No violation: the problem in plain words, its type as served for the code.
    expect(edit().cellState(ROW, column("value"))).toEqual({
      error: "said:invalid",
      code: "urn:kartenn:problem:invalid",
    });
    rows.value = [{ ...ROW, version: V2 }];
    await nextTick();
    expect(edit().cellState(rows.value[0]!, column("value"))).toBeUndefined();
  });

  // The nodes of the topography (task 12a): a prefix, a range, an address, as their page
  // serves them, each with its version.
  const R1 = "0b5e3c1a-0000-4000-8000-0000000000a2";
  const A1 = "0b5e3c1a-0000-4000-8000-0000000000a3";
  const VR = "0b5e3c1a-0000-4000-8000-0000000000c3";
  const NODES: Item[] = [
    { kind: "prefix", id: P1, value: "192.0.2.0/24", nature: "network", version: V1, href: "" },
    {
      kind: "range",
      id: R1,
      first: "192.0.2.10",
      last: "192.0.2.20",
      usage: "static",
      version: VR,
      href: "",
    },
    { kind: "address", id: A1, value: "192.0.2.5", version: V2, href: "" },
  ];
  const WORDS: Record<string, Record<string, string>> = {
    nature: { network: "Réseau", container: "Conteneur" },
    usage: { static: "Statique", dhcp: "DHCP" },
  };
  const nodeEdit = (reread: string[]) => (deps: Omit<InlineDeps, "collection">) =>
    useNodeEdit({
      ...deps,
      words: (field, key) => WORDS[field]?.[key] ?? null,
      reread: (id) => void reread.push(id),
    });

  it("a node: its detail edits its nature or usage, the words typed for the key, at its version", async () => {
    const reread: string[] = [];
    const { edit, seen, rows } = await mounted(
      ["core.prefix.edit", "core.range.edit", "core.address.edit"],
      (call) =>
        call.method === "POST" && call.url === "/v1/drafts"
          ? {
              status: 201,
              body: draftBody((call.body as { operations: unknown[] }).operations),
              etag: '"1"',
            }
          : { status: 200, body: { items: [], links: {} } },
      "/",
      nodeEdit(reread),
    );
    rows.value = NODES;
    await nextTick();
    const [prefix, range, address] = NODES as [Item, Item, Item];
    // The node itself (Enter opens its record, 021/A20), an address, the occupancy: none.
    expect(edit().editable(prefix, column(""))).toBe(false);
    expect(edit().editable(prefix, column("occupancy"))).toBe(false);
    expect(edit().editable(address, column("detail"))).toBe(false);
    expect(edit().editable(prefix, column("detail"))).toBe(true);
    expect(edit().editable(range, column("detail"))).toBe(true);
    expect(edit().validate(prefix, column("detail"), "Conteneur")).toBeUndefined();
    expect(edit().validate(prefix, column("detail"), "ailleurs")).toContain("Réseau, Conteneur");

    edit().onCommit({ row: P1, column: "detail", value: "Conteneur" });
    await expect.poll(() => edit().cellState(prefix, column("detail"))).toBe("draft");
    expect(edit().shownRows.value[0]!.nature).toBe("container");
    edit().onCommit({ row: R1, column: "detail", value: "dhcp" });
    await expect.poll(() => seen.filter((s) => s.method === "POST").length).toBe(2);
    const operations = seen
      .filter((s) => s.method === "POST")
      .flatMap((s) => (s.body as { operations: unknown[] }).operations);
    expect(operations).toEqual([
      { op: "update_prefix", id: { id: P1 }, nature: "container", expected: V1 },
      { op: "update_range", id: { id: R1 }, usage: "dhcp", expected: VR },
    ]);
  });

  it("a node applied now: If-Match its version served, 412 its level read again", async () => {
    const reread: string[] = [];
    const { edit, seen, rows } = await mounted(
      ["core.range.edit", "core.range.approval.self"],
      (call) => {
        if (call.method === "PATCH") {
          return {
            status: 412,
            body: { type: "urn:kartenn:problem:precondition-failed", title: "", status: 412 },
          };
        }
        if (call.url.startsWith(`/v1/ranges/${R1}`)) {
          return { status: 200, body: { ...NODES[1], usage: "reserved", version: V2 } };
        }
        return { status: 200, body: { items: [], links: {} } };
      },
      "/",
      nodeEdit(reread),
    );
    rows.value = NODES;
    await nextTick();
    // Without `core.prefix.edit`, the prefix has no editor and says why.
    expect(edit().why("detail", NODES[0])).toEqual({
      kind: "forbidden",
      permission: "core.prefix.edit",
    });
    edit().onCommit({ row: R1, column: "detail", value: "DHCP", now: true });
    expect(edit().asking.value).toMatchObject({
      row: R1,
      column: "detail",
      field: { name: "usage" },
      value: "dhcp",
      version: VR,
    });
    expect(await edit().confirm("motif", "dhcp")).toBe(false);
    const patch = seen.find((s) => s.method === "PATCH")!;
    expect(patch.url).toBe(`/v1/ranges/${R1}?approval=self`);
    expect(patch.ifMatch).toBe(`"${VR}"`);
    expect(reread).toEqual([R1]);
    expect(edit().asking.value).toMatchObject({
      value: "dhcp",
      current: "reserved",
      version: V2,
      refusal: { kind: "conflict" },
    });
  });
});

describe("the content of a node edited in place (task 12a)", () => {
  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
    queryClient.clear();
    resetContext();
    sessionStorage.clear();
    setFetch(null);
    document.body.innerHTML = "";
  });
  const P2 = "0b5e3c1a-0000-4000-8000-0000000000a4";
  const R2 = "0b5e3c1a-0000-4000-8000-0000000000a5";
  const VR = "0b5e3c1a-0000-4000-8000-0000000000c5";
  const CHILDREN = [
    { kind: "prefix", id: P2, value: "192.0.2.0/25", nature: "network", version: V1, href: "" },
    {
      kind: "range",
      id: R2,
      first: "192.0.2.130",
      last: "192.0.2.140",
      usage: "static",
      version: VR,
      href: "",
    },
  ];

  it("a cell without an editor says why, a draft marks its row, Ctrl+Enter its motive, 412 read again", async () => {
    const seen: { method: string; url: string; ifMatch: string | null }[] = [];
    setFetch((async (input: RequestInfo | URL, init?: RequestInit) => {
      const method = init?.method ?? "GET";
      const url = String(input);
      seen.push({ method, url, ifMatch: new Headers(init?.headers).get("If-Match") });
      const json = (status: number, body: unknown) =>
        new Response(JSON.stringify(body), {
          status,
          headers: {
            "Content-Type": status >= 400 ? "application/problem+json" : "application/json",
          },
        });
      if (url.startsWith(`/v1/prefixes/${P1}/children`)) {
        return json(200, { items: CHILDREN, links: {} });
      }
      if (method === "POST" && url === "/v1/drafts") {
        const operations = (JSON.parse(String(init!.body)) as { operations: unknown[] }).operations;
        return json(201, {
          id: D1,
          title: "Brouillon",
          version: 1,
          operation_count: 1,
          operations,
        });
      }
      if (method === "PATCH") {
        return json(412, {
          type: "urn:kartenn:problem:precondition-failed",
          title: "",
          status: 412,
        });
      }
      if (url.startsWith(`/v1/ranges/${R2}`)) {
        return json(200, { ...CHILDREN[1], usage: "dhcp", version: V2 });
      }
      return json(200, { items: [], links: {} });
    }) as typeof fetch);
    await loadSession(async (url) =>
      url === "/v1/me"
        ? ({
            identity: "alice",
            kind: "person",
            global_permissions: ["core.range.edit", "core.range.approval.self"],
            bindings: [],
          } as unknown as Me)
        : null,
    );
    const { default: ContentTab } = await import("../src/views/record/ContentTab.vue");
    const Host = defineComponent({
      setup: () => () => h(ContentTab, { kind: "prefix", id: P1, name: "192.0.2.0/24" }),
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: Host }],
    });
    await router.push("/");
    const root = document.createElement("div");
    document.body.append(root);
    app = createApp({ render: () => h(RouterView) });
    app.use(createShellI18n("fr")).use(router);
    app.mount(root);
    await router.isReady();
    const cell = (row: string) =>
      root.querySelector<HTMLElement>(`[data-row="${row}"][data-column="detail"]`);
    await expect.poll(() => cell(R2)?.textContent).toContain("Statique");

    // Without `core.prefix.edit`: no editor, and the cell says why (its description).
    const described = cell(P2)!.getAttribute("aria-describedby")!;
    expect(document.getElementById(described)?.textContent).toBe(
      "Lecture seule : il faut core.prefix.edit",
    );
    expect(cell(R2)!.getAttribute("aria-describedby")).toBeNull();

    /** The editor of the range opened, `text` typed, then `key` (with Ctrl or not). */
    async function type(text: string, ctrlKey = false): Promise<void> {
      cell(R2)!.click();
      cell(R2)!.focus();
      await nextTick();
      cell(R2)!.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }),
      );
      await expect.poll(() => document.querySelector("[data-k-content] input")).not.toBeNull();
      const input = document.querySelector<HTMLInputElement>("[data-k-content] input")!;
      input.value = text;
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", ctrlKey, bubbles: true }));
      await nextTick();
    }
    // Enter: to the draft, at the version the page served; the row marked « en brouillon ».
    await type("DHCP");
    await expect
      .poll(() => cell(R2)!.closest("[role='row']")!.getAttribute("data-row-state"))
      .toBe("draft");
    const post = seen.find((s) => s.method === "POST" && s.url === "/v1/drafts")!;
    expect(post).toBeDefined();

    // Ctrl+Enter: the motive asked under the grid; applied, 412: the content read again.
    await type("Réservée", true);
    await expect
      .poll(() => root.querySelector("form[aria-label^='Appliquer maintenant']"))
      .not.toBeNull();
    const reads = () => seen.filter((s) => s.url.startsWith(`/v1/prefixes/${P1}/children`)).length;
    const before = reads();
    root
      .querySelector<HTMLFormElement>("form[aria-label^='Appliquer maintenant']")!
      .requestSubmit();
    await expect.poll(() => seen.some((s) => s.method === "PATCH")).toBe(true);
    const patch = seen.find((s) => s.method === "PATCH")!;
    expect(patch.url).toBe(`/v1/ranges/${R2}?approval=self`);
    expect(patch.ifMatch).toBe(`"${VR}"`);
    await expect.poll(reads).toBeGreaterThan(before);
    await expect
      .poll(() => root.textContent)
      // The input kept is the one typed, beside the value read again.
      .toContain("maintenant dhcp, votre saisie reserved");
  });
});

describe("the edit panel of a kind", () => {
  it("closes with its page, and at another date off the present", async () => {
    const path = ref("/addresses");
    const period = ref<{ validAt: string | null; asOf: string | null }>({
      validAt: null,
      asOf: null,
    });
    const scope = effectScope();
    scope.run(() =>
      closeEditWithPage(
        () => path.value,
        () => period.value,
      ),
    );
    openEdit({ mode: "create", collection: "addresses" });
    expect(editRequest().value).toMatchObject({ mode: "create", collection: "addresses" });
    period.value = { validAt: "2026-01-01T00:00:00Z", asOf: null };
    await nextTick();
    expect(editRequest().value).toBeNull();
    // Opened under `valid_at` alone (the dated draft), closed once `as_of` comes.
    openEdit({ mode: "create", collection: "addresses" });
    period.value = { validAt: "2026-01-01T00:00:00Z", asOf: "2026-06-30T12:00:00Z" };
    await nextTick();
    expect(editRequest().value).toBeNull();
    period.value = { validAt: null, asOf: null };
    await nextTick();
    openEdit({ mode: "edit", collection: "prefixes", id: P1 });
    path.value = `/prefixes/${P1}`;
    await nextTick();
    expect(editRequest().value).toBeNull();
    scope.stop();
  });
});

describe("a VLAN created for a network (task 17b, 021/A13)", () => {
  afterEach(() => {
    drafts.setActive(null);
    queryClient.clear();
    setFetch(null);
  });
  const NETWORK = { id: P1, name: "192.0.2.0/24", property: D1 };

  it("creates the VLAN under a local name, then sets `core.vlan` of the network to it", () => {
    const creation = EDITS.vlans.create({
      group: V1,
      vid: "100",
      name: "serveurs",
      status: "active",
    });
    expect(vlanForNetwork(creation, NETWORK, 4)).toEqual([
      {
        op: "create_vlan",
        group: { id: V1 },
        vid: 100,
        name: "serveurs",
        status: "active",
        local: 4,
      },
      {
        op: "assign_property",
        kind: "prefix",
        object: { id: P1 },
        property: { id: D1 },
        values: [{ local: 4 }],
      },
    ]);
  });

  it("says a refusal of the setting on the network, by its operation or its pointer", () => {
    expect(ofAssignment({ operation: 1, pointer: null })).toBe(true);
    expect(ofAssignment({ pointer: "/operations/1/values/0" })).toBe(true);
    expect(ofAssignment({ pointer: "/operations/1" })).toBe(true);
    expect(ofAssignment({ operation: 0, pointer: "/operations/0/vid" })).toBe(false);
    expect(ofAssignment({ pointer: "/operations/10" })).toBe(false);
  });

  it("takes, in a draft, a local name free among its operations; a new draft, the first", () => {
    const creation = EDITS.vlans.create({ group: V1, vid: "100", name: "a", status: "active" });
    const made = vlanForNetworkInDraft(creation, NETWORK);
    expect(made([])[0]).toMatchObject({ local: 0 });
    const [vlan, setting] = made([
      { op: "create_domain", local: 0, slug: "a" },
      { op: "create_vlan", local: 1, vid: 10 } as never,
    ]);
    expect(vlan).toMatchObject({ local: 2 });
    expect(setting).toMatchObject({ values: [{ local: 2 }] });
  });

  it("refuses « Appliquer » without the approval of each edit permission, named", () => {
    const has = (held: string[]) => (p: string) => held.includes(p);
    const edits = [editRight("core.vlan"), ...NETWORK_VLAN_EDITS];
    const editor = ["core.vlan.edit", "core.prefix.edit", "prop.core.vlan.edit"];
    // `core.vlan.approval.self` alone: the network's is missing, by its name.
    const vlanOnly = has([...editor, "core.vlan.approval.self"]);
    expect(lackingRight(vlanOnly, edits, true)).toBe("core.prefix.approval.self");
    // To the draft, the edit permissions only.
    expect(lackingRight(vlanOnly, edits, false)).toBeNull();
    const withPrefix = has([...editor, "core.vlan.approval.self", "core.prefix.approval.self"]);
    expect(lackingRight(withPrefix, edits, true)).toBe("prop.core.vlan.approval.self");
    expect(lackingRight(has([...editor, ...editor.map(approvalOf)]), edits, true)).toBeNull();
    // An edit permission missing is named first, for both actions.
    expect(lackingRight(has(["core.vlan.edit"]), edits, false)).toBe("core.prefix.edit");
  });

  it("says `prop.assignment.invalid_value` of the setting on the field of the network", () => {
    const names = new Set(EDITS.vlans.creation.map((f) => f.name));
    const errors = fieldErrors(
      "vlans",
      names,
      [
        { rule: "core.vlan.vid_duplicate", detail: "VID taken", pointer: "/operations/0/vid" },
        {
          rule: "prop.assignment.invalid_value",
          detail: "not a VLAN",
          pointer: "/operations/1/values/0",
          operation: 1,
        },
      ],
      true,
    );
    expect(errors).toEqual({
      vid: "VID taken (core.vlan.vid_duplicate)",
      network: "not a VLAN (prop.assignment.invalid_value)",
    });
    // Without a network, an operation 1 lands on no field shown.
    expect(
      fieldErrors("vlans", names, [{ rule: "r", detail: "d", pointer: "/operations/1" }], false),
    ).toEqual({});
  });
});

describe("a violation of a creation applied at once", () => {
  it("lands on the field its operation's member stands for", () => {
    expect(fieldOfPointer("prefixes", "/operations/0/prefix")).toBe("value");
    expect(fieldOfPointer("addresses", "/operations/0/address")).toBe("value");
    expect(fieldOfPointer("ranges", "/operations/0/first")).toBe("first");
    // The server names the operation only: the field that names the object created.
    expect(fieldOfPointer("prefixes", "/operations/0")).toBe("value");
    expect(fieldOfPointer("domains", "/operations/0")).toBe("slug");
    expect(fieldOfPointer("prefixes", "/value")).toBe("value");
    expect(fieldOfPointer("prefixes", null)).toBeNull();
  });
});
