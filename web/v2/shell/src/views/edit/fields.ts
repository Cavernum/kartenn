import type { components, Operation } from "@kartenn/sdk";
import type { Rights } from "../../rights/permissions.ts";
import type { Writes } from "../../time/index.ts";
import { anchorOf, type CollectionId, type Item } from "../collections.ts";

// What an edit of an object of the referential writes (REQ-021-15, 36; design 021 « Édition
// en place », « Panneau d'édition par sorte »): by collection, the fields of its creation
// and of its merge patch, typed by the contract (`New*`, `*Patch`: a name that is not a
// member of the schema does not compile), the operation of the draft each gives (W10,
// `Operation`, the version read as `expected`), and whether a cell may be edited — the
// rights (`X.edit`, `prop.<key>.edit`), the present, the contract — and why not.

type Schemas = components["schemas"];

/** A field of the edit panel or a cell of a grid: its member, its kind, its choices. */
export interface EditField {
  name: string;
  /**
   * `integer`: a whole number (a VID), written as one; `anchor`: one object among the
   * entities, sites and domains, written `{<kind>: {id}}` (a group of VLAN, 026 V3).
   */
  kind: "text" | "integer" | "choice" | "flag" | "reference" | "anchor";
  /** The keys of a closed list of the contract, in the order offered. */
  options?: readonly string[];
  /** The collection a reference names (`{ id }`). */
  target?: CollectionId;
  required?: boolean;
  mono?: boolean;
}

/** Every value of a closed list of the contract, checked at compile time. */
function closed<T extends string>() {
  return <const A extends readonly T[]>(
    values: A & ([T] extends [A[number]] ? unknown : "a value of the contract is missing"),
  ): A => values;
}
const NATURES = closed<Schemas["Nature"]>()(["network", "container"]);
const ADDRESS_USAGES = closed<Schemas["AddressUsage"]>()([
  "host",
  "gateway",
  "vip",
  "anycast",
  "reserved",
]);
const RANGE_USAGES = closed<Schemas["RangeUsage"]>()(["static", "dhcp", "reserved"]);
const VLAN_STATUSES = closed<Schemas["VlanStatus"]>()(["active", "reserved", "deprecated"]);
/** The kinds an anchor of a group of VLAN names (026 V3), in the order offered. */
export const ANCHOR_KINDS = ["entity", "site", "domain"] as const;
type AnchorKind = (typeof ANCHOR_KINDS)[number];

/** Fields whose names are members of the schema `T`. */
function of<T>() {
  return <const F extends readonly (EditField & { name: keyof T & string })[]>(fields: F): F =>
    fields;
}

const slug = { name: "slug", kind: "text", required: true, mono: true } as const;
const domain = { name: "domain", kind: "reference", target: "domains", required: true } as const;

/** The edit of a collection: its fields, and the operations of the draft they give. */
export interface CollectionEdit {
  creation: readonly EditField[];
  modification: readonly EditField[];
  /** The field of « Changer de parent… » (REQ-021-32), for a kind whose parent is declared. */
  move?: readonly EditField[];
  /**
   * The read fields a cell shows that never change by an edit, and why: the domain
   * (`core.domain.immutable`), a value computed or fixed at creation, a parent moved by the
   * menu.
   */
  fixed: Readonly<Record<string, FixedReason>>;
  /** The operation that creates the object, from the values of the form. */
  create(values: Values): Operation;
  /** The operation that changes the object `id`, read at `expected`, by `changes`. */
  update(id: string, expected: string | null, changes: Values): Operation;
}

/**
 * Why a field never changes by an edit in place: `reference`, an object chosen in the edit
 * panel (`e`), never typed in a cell.
 */
export type FixedReason = "immutable" | "computed" | "creation" | "moved" | "reference";

/**
 * The values of a form: a text, a whole number as its digits, a key of a closed list, a
 * flag, an identifier, an anchor as `<kind>:<id>`.
 */
export type Values = Record<string, string | boolean>;

const text = (values: Values, name: string) => String(values[name] ?? "");
const ref = (id: string) => ({ id });
/** A whole number of the form (`"105"` → 105). */
const integer = (values: Values, name: string) => Number(text(values, name));
/** The anchor of the form (`entity:<id>`) as the contract writes it (`{entity: {id}}`). */
function anchor(value: string): Schemas["VlanAnchor"] {
  const [kind, id = ""] = value.split(":") as [AnchorKind, string?];
  if (kind === "site") return { site: ref(id) };
  if (kind === "domain") return { domain: ref(id) };
  return { entity: ref(id) };
}
/** The members of `changes` for an integer field among `names`, as numbers. */
function integers(changes: Values, names: readonly string[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const name of names) if (changes[name] !== undefined) out[name] = integer(changes, name);
  return out;
}
/** The members of `changes` among `names`, renamed for the operation (`value` → `prefix`). */
function members(changes: Values, names: Record<string, string>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [field, member] of Object.entries(names)) {
    if (changes[field] !== undefined) out[member] = changes[field];
  }
  return out;
}
const versioned = (expected: string | null) => (expected ? { expected } : {});

const rename =
  (kind: Schemas["NamedKind"]) =>
  (id: string, expected: string | null, changes: Values): Operation => ({
    op: "rename",
    kind,
    id: ref(id),
    slug: text(changes, "slug"),
    ...versioned(expected),
  });

const DOMAIN_FIXED = { domain: "immutable", public: "computed" } as const;
const NODE_FIXED = { parent: "moved" } as const;

export const EDITS: Readonly<Record<CollectionId, CollectionEdit>> = {
  domains: {
    creation: of<Schemas["NewDomain"]>()([slug]),
    modification: of<Schemas["DomainPatch"]>()([slug]),
    fixed: {},
    create: (values) => ({ op: "create_domain", slug: text(values, "slug") }),
    update: rename("domain"),
  },
  entities: {
    creation: of<Schemas["NewNode"]>()([
      slug,
      { name: "parent", kind: "reference", target: "entities" },
    ]),
    modification: of<Schemas["NodePatch"]>()([slug]),
    move: of<Schemas["NodePatch"]>()([{ name: "parent", kind: "reference", target: "entities" }]),
    fixed: NODE_FIXED,
    create: (values) => ({
      op: "create_entity",
      slug: text(values, "slug"),
      ...(text(values, "parent") ? { parent: ref(text(values, "parent")) } : {}),
    }),
    update: rename("entity"),
  },
  sites: {
    creation: of<Schemas["NewNode"]>()([
      slug,
      { name: "parent", kind: "reference", target: "sites" },
    ]),
    modification: of<Schemas["NodePatch"]>()([slug]),
    move: of<Schemas["NodePatch"]>()([{ name: "parent", kind: "reference", target: "sites" }]),
    fixed: NODE_FIXED,
    create: (values) => ({
      op: "create_site",
      slug: text(values, "slug"),
      ...(text(values, "parent") ? { parent: ref(text(values, "parent")) } : {}),
    }),
    update: rename("site"),
  },
  prefixes: {
    creation: of<Schemas["NewPrefix"]>()([
      domain,
      { name: "value", kind: "text", required: true, mono: true },
      { name: "nature", kind: "choice", options: NATURES, required: true },
      { name: "point_to_point", kind: "flag" },
      { name: "anycast", kind: "flag" },
    ]),
    modification: of<Schemas["PrefixPatch"]>()([
      { name: "value", kind: "text", required: true, mono: true },
      { name: "nature", kind: "choice", options: NATURES, required: true },
      { name: "point_to_point", kind: "flag" },
      { name: "anycast", kind: "flag" },
    ]),
    fixed: DOMAIN_FIXED,
    create: (values) => ({
      op: "create_prefix",
      domain: ref(text(values, "domain")),
      prefix: text(values, "value"),
      nature: text(values, "nature") as Schemas["Nature"],
      point_to_point: values.point_to_point === true,
      anycast: values.anycast === true,
    }),
    update: (id, expected, changes) => ({
      op: "update_prefix",
      id: ref(id),
      ...members(changes, {
        value: "prefix",
        nature: "nature",
        point_to_point: "point_to_point",
        anycast: "anycast",
      }),
      ...versioned(expected),
    }),
  },
  ranges: {
    creation: of<Schemas["NewRange"]>()([
      domain,
      { name: "first", kind: "text", required: true, mono: true },
      { name: "last", kind: "text", required: true, mono: true },
      { name: "usage", kind: "choice", options: RANGE_USAGES, required: true },
    ]),
    modification: of<Schemas["RangePatch"]>()([
      { name: "usage", kind: "choice", options: RANGE_USAGES, required: true },
    ]),
    fixed: { ...DOMAIN_FIXED, first: "creation", last: "creation" },
    create: (values) => ({
      op: "create_range",
      domain: ref(text(values, "domain")),
      first: text(values, "first"),
      last: text(values, "last"),
      usage: text(values, "usage") as Schemas["RangeUsage"],
    }),
    update: (id, expected, changes) => ({
      op: "update_range",
      id: ref(id),
      usage: text(changes, "usage") as Schemas["RangeUsage"],
      ...versioned(expected),
    }),
  },
  addresses: {
    creation: of<Schemas["NewAddress"]>()([
      domain,
      { name: "value", kind: "text", required: true, mono: true },
      { name: "usage", kind: "choice", options: ADDRESS_USAGES, required: true },
    ]),
    modification: of<Schemas["AddressPatch"]>()([
      { name: "usage", kind: "choice", options: ADDRESS_USAGES, required: true },
    ]),
    fixed: { ...DOMAIN_FIXED, value: "creation" },
    create: (values) => ({
      op: "create_address",
      domain: ref(text(values, "domain")),
      address: text(values, "value"),
      usage: text(values, "usage") as Schemas["AddressUsage"],
    }),
    update: (id, expected, changes) => ({
      op: "update_address",
      id: ref(id),
      usage: text(changes, "usage") as Schemas["AddressUsage"],
      ...versioned(expected),
    }),
  },
  // The VLAN (026): their operations carry no `expected` (the contract has none for them).
  "vlan-groups": {
    creation: of<Schemas["VlanGroupCreation"]>()([
      slug,
      { name: "anchor", kind: "anchor", required: true },
      { name: "vid_min", kind: "integer", mono: true },
      { name: "vid_max", kind: "integer", mono: true },
    ]),
    modification: of<Schemas["VlanGroupPatch"]>()([
      slug,
      { name: "anchor", kind: "anchor", required: true },
      { name: "vid_min", kind: "integer", required: true, mono: true },
      { name: "vid_max", kind: "integer", required: true, mono: true },
    ]),
    fixed: { anchor: "reference" },
    create: (values) => ({
      op: "create_vlan_group",
      slug: text(values, "slug"),
      anchor: anchor(text(values, "anchor")),
      // Unset, the range of the contract (1 to 4094).
      ...integers(
        values,
        ["vid_min", "vid_max"].filter((name) => text(values, name) !== ""),
      ),
    }),
    update: (id, _expected, changes) => ({
      op: "update_vlan_group",
      id: ref(id),
      ...members(changes, { slug: "slug" }),
      ...(changes.anchor !== undefined ? { anchor: anchor(text(changes, "anchor")) } : {}),
      ...integers(changes, ["vid_min", "vid_max"]),
    }),
  },
  vlans: {
    creation: of<Schemas["VlanCreation"]>()([
      { name: "group", kind: "reference", target: "vlan-groups", required: true },
      { name: "vid", kind: "integer", required: true, mono: true },
      { name: "name", kind: "text", required: true, mono: true },
      { name: "status", kind: "choice", options: VLAN_STATUSES, required: true },
    ]),
    modification: of<Schemas["VlanPatch"]>()([
      { name: "vid", kind: "integer", required: true, mono: true },
      { name: "name", kind: "text", required: true, mono: true },
      { name: "status", kind: "choice", options: VLAN_STATUSES, required: true },
      { name: "group", kind: "reference", target: "vlan-groups", required: true },
    ]),
    fixed: { group: "reference" },
    create: (values) => ({
      op: "create_vlan",
      group: ref(text(values, "group")),
      vid: integer(values, "vid"),
      name: text(values, "name"),
      status: text(values, "status") as Schemas["VlanStatus"],
    }),
    update: (id, _expected, changes) => ({
      op: "update_vlan",
      id: ref(id),
      ...members(changes, { name: "name", status: "status" }),
      ...(changes.group !== undefined ? { group: ref(text(changes, "group")) } : {}),
      ...integers(changes, ["vid"]),
    }),
  },
};

/** The members of a read object that the operations of the draft name otherwise. */
const OPERATION_MEMBERS: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  update_prefix: {
    prefix: "value",
    nature: "nature",
    point_to_point: "point_to_point",
    anycast: "anycast",
  },
  update_range: { usage: "usage" },
  update_address: { usage: "usage" },
  rename: { slug: "slug" },
  update_vlan_group: { slug: "slug", vid_min: "vid_min", vid_max: "vid_max" },
  update_vlan: { vid: "vid", name: "name", status: "status" },
};
const UPDATE_OF: Readonly<Record<CollectionId, string>> = {
  domains: "rename",
  entities: "rename",
  sites: "rename",
  prefixes: "update_prefix",
  ranges: "update_range",
  addresses: "update_address",
  "vlan-groups": "update_vlan_group",
  vlans: "update_vlan",
};
const NAMED: Partial<Record<CollectionId, string>> = {
  domains: "domain",
  entities: "entity",
  sites: "site",
};

/**
 * What the operations of a draft change on the objects of a collection, by identifier:
 * the fields as the grid reads them (`prefix` → `value`), the last operation winning.
 */
export function draftedValues(
  operations: readonly Operation[] | undefined,
  collection: CollectionId,
): Map<string, Values> {
  const out = new Map<string, Values>();
  for (const operation of operations ?? []) {
    const op = operation as Record<string, unknown>;
    if (op.op !== UPDATE_OF[collection]) continue;
    if (op.op === "rename" && op.kind !== NAMED[collection]) continue;
    const id = (op.id as { id?: unknown } | undefined)?.id;
    if (typeof id !== "string") continue;
    const values = { ...out.get(id) };
    for (const [member, field] of Object.entries(OPERATION_MEMBERS[op.op as string] ?? {})) {
      const value = op[member];
      if (typeof value === "string" || typeof value === "boolean") values[field] = value;
      // A whole number (a VID) as the form holds it.
      if (typeof value === "number") values[field] = String(value);
    }
    out.set(id, values);
  }
  return out;
}

/** The members of the creation operations named otherwise than their field. */
const CREATION_MEMBERS: Partial<Record<CollectionId, Readonly<Record<string, string>>>> = {
  prefixes: { prefix: "value" },
  addresses: { address: "value" },
};

/**
 * The field that names the object being created: what the rules of a creation are about
 * (duplicate, alignment, inside the network, overlap, slug taken).
 */
const KEY_FIELD: Readonly<Record<CollectionId, string>> = {
  domains: "slug",
  entities: "slug",
  sites: "slug",
  prefixes: "value",
  ranges: "first",
  addresses: "value",
  "vlan-groups": "slug",
  vlans: "vid",
};

/**
 * The field a violation names by its JSON Pointer: a member of the patch (`/value`); for
 * the one operation of a creation applied at once, its member (`/operations/0/prefix` →
 * `value`), or, the server naming the operation only (`/operations/0`), the field that
 * names the object.
 */
export function fieldOfPointer(
  collection: CollectionId,
  pointer: string | null | undefined,
): string | null {
  const operation = /^\/operations\/0(?:\/([^/]+))?$|^\/operations\/0\/([^/]+)\//.exec(
    pointer ?? "",
  );
  if (operation) {
    const member = operation[1] ?? operation[2];
    return member ? (CREATION_MEMBERS[collection]?.[member] ?? member) : KEY_FIELD[collection];
  }
  return /^\/([^/]+)/.exec(pointer ?? "")?.[1] ?? null;
}

/** The permission that writes the objects of a collection (`core.prefix.edit`, 024). */
export const editRight = (type: string) => `${type}.edit`;
/** The permission that applies one's own changeset at once (`core.prefix.approval.self`). */
export const applyRight = (type: string) => `${type}.approval.self`;
/** The permission that writes a property (`prop.core.owner.edit`, spec 018). */
export const propertyRight = (key: string) => `prop.${key}.edit`;
/**
 * The approval of one's own changeset that goes with an edit permission, which the server
 * asks of each one a changeset applied at once needs (024, `verb_permission`):
 * `core.prefix.edit` → `core.prefix.approval.self`, `prop.core.vlan.edit` →
 * `prop.core.vlan.approval.self`.
 */
export const approvalOf = (edit: string) => edit.replace(/\.edit$/, ".approval.self");
/**
 * The permission an action lacks, named: each of `edits` for both actions; to apply, the
 * approval of each too.
 */
export function lackingRight(
  has: (permission: string) => boolean,
  edits: readonly string[],
  apply: boolean,
): string | null {
  const missing = edits.find((p) => !has(p));
  if (missing) return missing;
  return apply ? (edits.map(approvalOf).find((p) => !has(p)) ?? null) : null;
}
/** A column of a grid that stands for a property (`prop.<key>`). */
const PROPERTY_COLUMN = /^prop\.(.+)$/;

/** Why a cell offers no editor. */
export type NotEditable =
  /** A right is missing: the permission, as served. */
  | { kind: "forbidden"; permission: string }
  /** The view is read as known at a past time (`as_of`): nothing is written. */
  | { kind: "past" }
  /** The view is at a validity date alone: the change goes by the edit panel, to the dated draft. */
  | { kind: "dated" }
  /** The field never changes by an edit. */
  | { kind: "fixed"; reason: FixedReason }
  /** The field is not an attribute an edit writes (the identifier, a column of a plugin). */
  | { kind: "none" };

export type Editability = { editable: true } | { editable: false; why: NotEditable };

/**
 * Whether the cell `column` of an object of `collection` may be edited in place: the field
 * is one of its patch (or a property of a `prop.<key>` column), the view at the present
 * (`writes.quickEdit`; under `valid_at` alone, the dated draft by the edit panel only), and
 * the user holds `X.edit` of the type and, for a property, `prop.<key>.edit`. The server
 * stays the judge (a 403 is still shown as a refusal).
 */
export function editability(
  rights: Rights,
  writes: Writes,
  collection: { id: CollectionId; type: string },
  column: string,
): Editability {
  const edit = EDITS[collection.id];
  const property = PROPERTY_COLUMN.exec(column)?.[1];
  const fixed = edit.fixed[column];
  if (fixed) return { editable: false, why: { kind: "fixed", reason: fixed } };
  if (!property && !edit.modification.some((field) => field.name === column)) {
    return { editable: false, why: { kind: "none" } };
  }
  // Under `as_of` nothing is written, whatever the rights; the dated draft (`valid_at`
  // alone) is said only to whom may write it, a missing right named first.
  if (!writes.draft) return { editable: false, why: { kind: "past" } };
  for (const permission of [
    editRight(collection.type),
    ...(property ? [propertyRight(property)] : []),
  ]) {
    if (!rights.has(permission)) return { editable: false, why: { kind: "forbidden", permission } };
  }
  if (!writes.quickEdit) return { editable: false, why: { kind: "dated" } };
  return { editable: true };
}

/** The field of the patch of a collection named `name`, if any. */
export function modificationField(collection: CollectionId, name: string): EditField | undefined {
  return EDITS[collection].modification.find((field) => field.name === name);
}

/**
 * The text a cell or a field holds for a value read: a flag as `true`/`false`, a number
 * by its digits, a reference by its identifier, an anchor as `<kind>:<id>`.
 */
export function textOf(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "boolean" || typeof value === "number") return String(value);
  if (typeof value === "object" && value !== null && typeof (value as Item).id === "string") {
    return String((value as Item).id);
  }
  const anchored = anchorOf(value);
  return anchored ? `${anchored.kind}:${anchored.id}` : "";
}

/**
 * The local check of a value by the types of the contract (design « Édition en place »):
 * a required text not empty, a key of the closed list; the rules of the domain are the
 * server's (422). The key of the message, or nothing.
 */
export function checkValue(field: EditField, value: string | boolean): string | undefined {
  if (field.kind === "flag") return typeof value === "boolean" ? undefined : "edit.check.flag";
  const text = String(value).trim();
  if (field.required && text === "") return "edit.check.required";
  if (field.kind === "choice" && text !== "" && !field.options?.includes(text)) {
    return "edit.check.choice";
  }
  if (field.kind === "integer" && text !== "" && !/^\d{1,9}$/.test(text)) {
    return "edit.check.integer";
  }
  return undefined;
}

/** The value of a cell's text for its field: a flag from `true`/`false`, a text trimmed. */
export function valueOf(field: EditField, text: string): string | boolean {
  if (field.kind === "flag") return text === "true";
  return text.trim();
}

/**
 * The form of `fields` filled from an object read (a modification), or from the values
 * given (a creation: the first choice of a closed list, a flag off, the rest empty).
 */
export function initialValues(
  fields: readonly EditField[],
  item: Item | null,
  preset: Readonly<Record<string, string>> = {},
): Values {
  const values: Values = {};
  for (const field of fields) {
    const read = item?.[field.name];
    if (field.kind === "flag") values[field.name] = read === true;
    else if (item) values[field.name] = textOf(read);
    else
      values[field.name] =
        preset[field.name] ?? (field.kind === "choice" ? field.options![0]! : "");
  }
  return values;
}

/** What changed between `before` and `values`, among `fields`. */
export function changesOf(fields: readonly EditField[], before: Values, values: Values): Values {
  const out: Values = {};
  for (const field of fields) {
    if (values[field.name] !== before[field.name]) out[field.name] = values[field.name]!;
  }
  return out;
}

/**
 * The members of the merge patch of `changes` (RFC 7396): a reference as `{ id }`, a whole
 * number as a number, an anchor as `{<kind>: {id}}`.
 */
export function patchOf(fields: readonly EditField[], changes: Values): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const field of fields) {
    const value = changes[field.name];
    if (value === undefined) continue;
    out[field.name] =
      field.kind === "reference"
        ? ref(String(value))
        : field.kind === "integer"
          ? Number(value)
          : field.kind === "anchor"
            ? anchor(String(value))
            : value;
  }
  return out;
}
