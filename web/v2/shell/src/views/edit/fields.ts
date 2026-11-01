import type { components, Operation } from "@kartenn/sdk";
import type { Rights } from "../../rights/permissions.ts";
import type { CollectionId, Item } from "../collections.ts";

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
  kind: "text" | "choice" | "flag" | "reference";
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

/** Why a field never changes by an edit. */
export type FixedReason = "immutable" | "computed" | "creation" | "moved";

/** The values of a form: a text, a key of a closed list, a flag, an identifier. */
export type Values = Record<string, string | boolean>;

const text = (values: Values, name: string) => String(values[name] ?? "");
const ref = (id: string) => ({ id });
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
};
const UPDATE_OF: Readonly<Record<CollectionId, string>> = {
  domains: "rename",
  entities: "rename",
  sites: "rename",
  prefixes: "update_prefix",
  ranges: "update_range",
  addresses: "update_address",
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
/** A column of a grid that stands for a property (`prop.<key>`). */
const PROPERTY_COLUMN = /^prop\.(.+)$/;

/** Why a cell offers no editor. */
export type NotEditable =
  /** A right is missing: the permission, as served. */
  | { kind: "forbidden"; permission: string }
  /** The view is not at the present. */
  | { kind: "past" }
  /** The field never changes by an edit. */
  | { kind: "fixed"; reason: FixedReason }
  /** The field is not an attribute an edit writes (the identifier, a column of a plugin). */
  | { kind: "none" };

export type Editability = { editable: true } | { editable: false; why: NotEditable };

/**
 * Whether the cell `column` of an object of `collection` may be edited in place: the field
 * is one of its patch (or a property of a `prop.<key>` column), the view at the present,
 * and the user holds `X.edit` of the type and, for a property, `prop.<key>.edit`. The
 * server stays the judge (a 403 is still shown as a refusal).
 */
export function editability(
  rights: Rights,
  present: boolean,
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
  if (!present) return { editable: false, why: { kind: "past" } };
  for (const permission of [
    editRight(collection.type),
    ...(property ? [propertyRight(property)] : []),
  ]) {
    if (!rights.has(permission)) return { editable: false, why: { kind: "forbidden", permission } };
  }
  return { editable: true };
}

/** The field of the patch of a collection named `name`, if any. */
export function modificationField(collection: CollectionId, name: string): EditField | undefined {
  return EDITS[collection].modification.find((field) => field.name === name);
}

/** The text a cell or a field holds for a value read (a flag as `true`/`false`). */
export function textOf(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "boolean") return String(value);
  if (typeof value === "object" && value !== null && typeof (value as Item).id === "string") {
    return String((value as Item).id);
  }
  return "";
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

/** The members of the merge patch of `changes` (RFC 7396): a reference as `{ id }`. */
export function patchOf(fields: readonly EditField[], changes: Values): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const field of fields) {
    const value = changes[field.name];
    if (value === undefined) continue;
    out[field.name] = field.kind === "reference" ? ref(String(value)) : value;
  }
  return out;
}
