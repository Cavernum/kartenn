// @vitest-environment happy-dom
import { ApiProblem, queryClient } from "@kartenn/sdk";
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
  changesOf,
  checkValue,
  draftedValues,
  editability,
  EDITS,
  fieldOfPointer,
  initialValues,
  patchOf,
} from "../src/views/edit/fields.ts";
import { useInlineEdit, type InlineEdit, type InlineSay } from "../src/views/edit/inline.ts";

// In-place editing and the edit panel of a kind (task 021-12; REQ-021-15, 16, 36, 37): the
// fields of each kind typed by the contract, the operations of the draft they give, whether
// a cell may be edited and why not; then a list edited in place against a fake API — Enter
// to the draft, Ctrl+Enter applied with its motive, 202 in review and never applied, 412
// the input kept beside the value read again, no apply without `X.approval.self`.

const PREFIXES = collectionById("prefixes")!;
const P1 = "0b5e3c1a-0000-4000-8000-0000000000a1";
const V1 = "0b5e3c1a-0000-4000-8000-0000000000c1";
const V2 = "0b5e3c1a-0000-4000-8000-0000000000c2";
const D1 = "0b5e3c1a-0000-4000-8000-0000000000d1";

describe("the fields of an edit", () => {
  const author = new Rights(["core.prefix.edit", "prop.core.owner.edit"]);
  const reader = new Rights(["core.prefix.read"]);

  it("let a cell be edited with the right, at the present, when the patch takes it", () => {
    expect(editability(author, true, PREFIXES, "value")).toEqual({ editable: true });
    expect(editability(author, true, PREFIXES, "nature")).toEqual({ editable: true });
    // A reader: no editor, the permission named as served.
    expect(editability(reader, true, PREFIXES, "value")).toEqual({
      editable: false,
      why: { kind: "forbidden", permission: "core.prefix.edit" },
    });
    expect(editability(author, false, PREFIXES, "value")).toEqual({
      editable: false,
      why: { kind: "past" },
    });
    // The domain never changes after creation; a computed value never does.
    expect(editability(author, true, PREFIXES, "domain")).toMatchObject({
      why: { kind: "fixed", reason: "immutable" },
    });
    expect(editability(author, true, PREFIXES, "public")).toMatchObject({
      why: { kind: "fixed", reason: "computed" },
    });
    expect(editability(author, true, PREFIXES, "id")).toMatchObject({ why: { kind: "none" } });
  });

  it("ask `prop.<key>.edit` too for a property", () => {
    // The rule (a list offers no editor until it writes properties: `useInlineEdit.why`).
    expect(editability(author, true, PREFIXES, "prop.core.owner")).toEqual({ editable: true });
    expect(editability(author, true, PREFIXES, "prop.dns.ttl")).toEqual({
      editable: false,
      why: { kind: "forbidden", permission: "prop.dns.ttl.edit" },
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
        edit = useInlineEdit({
          collection: () => PREFIXES,
          rows: () => rows.value,
          t: (key, values) => `${key}${values ? JSON.stringify(values) : ""}`,
          sentence: (error) =>
            error instanceof ApiProblem ? `said:${error.code}` : "said:network",
          say,
        });
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
    const past = await mounted(APPLY, () => lists, "/?valid_at=2026-01-01T00:00:00Z");
    expect(past.edit().why("value")).toEqual({ kind: "past" });
    past.edit().onCommit({ row: P1, column: "value", value: "192.0.2.0/25" });
    await nextTick();
    expect(past.seen.filter((s) => s.method !== "GET")).toEqual([]);
    app?.unmount();
    const now = await mounted(APPLY, () => lists);
    now.edit().onCommit({ row: P1, column: "value", value: "192.0.2.0/25", now: true });
    expect(now.edit().asking.value).not.toBeNull();
    await now.router.push("/?as_of=2026-01-01T00:00:00Z");
    await nextTick();
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
});

describe("the edit panel of a kind", () => {
  it("closes with its page, and off the present (nothing written there)", async () => {
    const path = ref("/addresses");
    const present = ref(true);
    const scope = effectScope();
    scope.run(() =>
      closeEditWithPage(
        () => path.value,
        () => present.value,
      ),
    );
    openEdit({ mode: "create", collection: "addresses" });
    expect(editRequest().value).toMatchObject({ mode: "create", collection: "addresses" });
    present.value = false;
    await nextTick();
    expect(editRequest().value).toBeNull();
    present.value = true;
    openEdit({ mode: "edit", collection: "prefixes", id: P1 });
    path.value = `/prefixes/${P1}`;
    await nextTick();
    expect(editRequest().value).toBeNull();
    scope.stop();
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
