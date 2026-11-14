import { addToActiveDraft } from "@kartenn/sdk/host";
import {
  drafts,
  keys,
  queryClient,
  reads,
  type components,
  type Draft,
  type Entry,
  type Operation,
  type Page,
} from "@kartenn/sdk";
import type { CellState, GridColumn, GridCommit, ListOption, RowState } from "@kartenn/ui";
import { computed, onScopeDispose, ref, shallowReactive, shallowRef, watch } from "vue";
import { useQuery } from "../../data/query.ts";
import { useSession } from "../../rights/index.ts";
import { useTime, useWrites } from "../../time/index.ts";
import {
  collectionById,
  collectionOfKind,
  nameOf,
  type Collection,
  type CollectionId,
  type Item,
} from "../collections.ts";
import {
  approvalOf,
  editability,
  propertyRight,
  type EditField,
  type FixedReason,
  type NotEditable,
} from "./fields.ts";
import { cellErrorOf, quickWrite, whyWords, type InlineDeps, type QuickAsk } from "./inline.ts";

// The tab « Propriétés » of a record edited in place (task 12h; REQ-021-15, 16, 37; 003,
// Q-021-19): the value of a property of one object, typed in its cell and checked by the
// type of its declaration (REQ-003-8). Enter adds `assign_property` (or, the cell emptied,
// `unassign_property`) to the active draft; Ctrl+Enter applies `properties` of the merge
// patch of the kind under `If-Match`, with its motive, as `useInlineEdit` does a field
// (200, 202 « en revue » and why, 412 the input kept beside the value read again, 422, 403).
// A value inherited is edited too: it becomes the object's own, which overrides the
// inherited one — the cell says so before it is sent, from the explanation of the value
// (origin); emptying a value of the object's own removes it, inheritance resumes. No editor,
// and why, for a value masked, computed (inferred: no assignment), of a type no cell types
// (a reference, a set), of a kind whose patch writes no property, or without
// `prop.<key>.edit` (the property's permission, apart from the object's, REQ-003-17).

type Schemas = components["schemas"];
type Explanation = Schemas["Explanation"];
/** A raw value of an assignment (REQ-003-8) as a cell writes it. */
type Raw = string | number | boolean;
/** A declaration as this module reads it (`/v1/property-declarations`). */
export type Declared = Pick<Schemas["DeclarationItem"], "id" | "key" | "type" | "cardinality"> & {
  readonly params?: Readonly<Record<string, unknown>>;
};

/** Whether the merge patch of a kind takes `properties`, checked against the contract. */
type Takes<P> = "properties" extends keyof P ? true : false;
export const PROPERTIES_PATCHED: Readonly<Record<CollectionId, boolean>> = {
  domains: true satisfies Takes<Schemas["DomainPatch"]>,
  entities: true satisfies Takes<Schemas["NodePatch"]>,
  sites: true satisfies Takes<Schemas["NodePatch"]>,
  prefixes: true satisfies Takes<Schemas["PrefixPatch"]>,
  ranges: true satisfies Takes<Schemas["RangePatch"]>,
  addresses: true satisfies Takes<Schemas["AddressPatch"]>,
  "vlan-groups": false satisfies Takes<Schemas["VlanGroupPatch"]>,
  vlans: false satisfies Takes<Schemas["VlanPatch"]>,
};

/** The declared types a cell takes as text (REQ-003-8); a reference is chosen, not typed. */
const TYPED: ReadonlySet<string> = new Set(["text", "int", "bool", "date", "fqdn", "enum"]);

/** Whether a value of `declaration` is typed in a cell: one value of a type above. */
export const typedInPlace = (declaration: Declared): boolean =>
  declaration.cardinality === "single" && TYPED.has(declaration.type);

/** The text a cell holds for a resolved value: a number by its digits, none for no value. */
export function propertyText(value: unknown): string {
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return "";
}

/** The refusal of a value typed: the key of its message and its values. */
export interface PropertyCheck {
  key: string;
  values?: Record<string, unknown>;
}

const param = (declaration: Declared, name: string) => declaration.params?.[name];
const DATE = /^(\d{4})-(\d{2})-(\d{2})$/;
/** A label of a host name (RFC 1123 §2.1), ASCII by LDH, any non-ASCII left to the server. */
const LABEL = /^(?!-)(?:[A-Za-z0-9-]|[^\u0000-\u007f]){1,63}(?<!-)$/u;
/** The full stops UTS #46 maps to `.` (ideographic, full-width, half-width). */
const SEPARATORS = /[。．｡]/g;

/**
 * A measure out of its declared bounds, said by the bounds there are: `<what>` both,
 * `<what>Min` a lower one alone, `<what>Max` an upper one alone.
 */
function bounded(
  what: "length" | "between",
  measure: number,
  low: unknown,
  high: unknown,
): PropertyCheck | undefined {
  const min = typeof low === "number" ? low : null;
  const max = typeof high === "number" ? high : null;
  if ((min === null || measure >= min) && (max === null || measure <= max)) return undefined;
  if (min !== null && max !== null) return { key: `edit.check.${what}`, values: { min, max } };
  return min !== null
    ? { key: `edit.check.${what}Min`, values: { min } }
    : { key: `edit.check.${what}Max`, values: { max } };
}

/**
 * The local check of a value typed for `declaration` (REQ-003-8; design « Édition en
 * place »), never stricter than the server's: the type and its parameters (length in NFC,
 * bounds, a key of the closed list
 * when `choices` holds the whole list). Emptied, the cell removes a value of the object's
 * own: refused when the value shown is not (`own` false). The rest is the server's (422).
 */
export function checkProperty(
  declaration: Declared,
  text: string,
  own: boolean | undefined,
  choices: readonly string[] | null = null,
): PropertyCheck | undefined {
  const typed = text.trim();
  if (typed === "") return own === false ? { key: "edit.property.nothingToRemove" } : undefined;
  switch (declaration.type) {
    case "text": {
      // Counted as the server counts: in NFC, by code point (`values.rs`, TextType). The
      // pattern is the server's alone: the `regex` crate and JavaScript differ (`\d`, `\w`
      // are Unicode there), a local check could refuse what the server takes.
      const normal = typed.normalize("NFC");
      if (/[\u0000-\u001f\u007f-\u009f]/.test(normal)) return { key: "edit.check.control" };
      return bounded(
        "length",
        [...normal].length,
        param(declaration, "min_len"),
        param(declaration, "max_len"),
      );
    }
    case "int": {
      const value = Number(typed);
      if (!/^-?\d+$/.test(typed) || !Number.isSafeInteger(value)) {
        return { key: "edit.check.integer" };
      }
      return bounded("between", value, param(declaration, "min"), param(declaration, "max"));
    }
    case "bool":
      return typed === "true" || typed === "false" ? undefined : { key: "edit.check.bool" };
    case "date": {
      const [, y, m, d] = DATE.exec(typed) ?? [];
      // `setUTCFullYear`: the years 0 to 99 as they are (`Date.UTC` takes them as 19xx).
      const day = new Date(0);
      if (y) day.setUTCFullYear(Number(y), Number(m) - 1, Number(d));
      const valid = y !== undefined && day.toISOString().slice(0, 10) === typed;
      return valid ? undefined : { key: "edit.check.date" };
    }
    case "fqdn": {
      // Looser than the server (IDNA2008 / UTS #46, `values.rs`): the separators UTS #46
      // maps to a dot, any non-ASCII character left to it; the lengths hold, an A-label being
      // never shorter than its Unicode form.
      const name = typed.replace(SEPARATORS, ".").replace(/\.$/, "");
      const valid = name.length <= 253 && name.split(".").every((label) => LABEL.test(label));
      return valid ? undefined : { key: "edit.check.fqdn" };
    }
    case "enum":
      return choices && !choices.includes(typed)
        ? { key: "edit.check.choice", values: { values: choices.join(", ") } }
        : undefined;
    default:
      return undefined;
  }
}

/**
 * The closed list of a cell of `declaration` (task 021-12g): a `bool`'s `true` and
 * `false`, as its cell writes them; the whole list of an `enum` (`list`) once read, in its
 * words (`words`, else the key in fixed pitch): a part, or none yet, leaves it typed. A
 * value of the object's own (`own`) may be removed: `none` after them, the cell emptied.
 */
export function propertyChoices(
  declaration: Declared,
  list: readonly string[] | null | undefined,
  o: { words: (value: string) => string | null; own: boolean; none: string },
): ListOption[] | undefined {
  const bool = declaration.type === "bool";
  const keys = bool ? ["true", "false"] : declaration.type === "enum" ? list : null;
  // An enum declared without values: nothing to choose, the text editor (the server says).
  if (!keys || keys.length === 0) return undefined;
  const options: ListOption[] = keys.map((value) => {
    const label = bool ? null : o.words(value);
    return { value, label: label ?? value, mono: label === null };
  });
  if (o.own) options.push({ value: "", label: o.none });
  return options;
}

/** The raw value of a text typed for `declaration`; `null`, the cell emptied: removed. */
export function propertyValueOf(declaration: Declared, text: string): Raw | null {
  const typed = text.trim();
  if (typed === "") return null;
  if (declaration.type === "int") return Number(typed);
  if (declaration.type === "bool") return typed === "true";
  return typed;
}

/**
 * The operation of the draft (W10, the contract's): the value assigned to the property of
 * the object, or, `null`, its assignment removed — the object inherits again (REQ-003-12).
 */
export function propertyOperation(
  kind: string,
  object: string,
  declaration: string,
  value: Raw | null,
): Operation {
  const target = {
    kind: kind as Schemas["TargetKind"],
    object: { id: object },
    property: { id: declaration },
  };
  return value === null
    ? { op: "unassign_property", ...target }
    : { op: "assign_property", ...target, values: [value] };
}

/**
 * The operations an edit adds to a draft whose operations are `current` (Q-021-21): its
 * guard on the version read (`expect`, as the server's `PATCH` writes one), unless the draft
 * already guards the object or no version was read, then the operation itself.
 */
export function guarded(
  current: readonly Operation[],
  kind: string,
  id: string,
  version: string | null,
  operation: Operation,
): Operation[] {
  const has = current.some((op) => {
    const o = op as Record<string, unknown>;
    return o.op === "expect" && (o.id as { id?: unknown } | undefined)?.id === id;
  });
  if (has || version === null) return [operation];
  const guard: Operation = {
    op: "expect",
    kind: kind as Schemas["TargetKind"],
    id: { id },
    expected: version,
  };
  return [guard, operation];
}

/** The members of the merge patch (RFC 7396) of the kind: `properties`, `null` removes. */
export function propertyPatch(key: string, value: Raw | null): Record<string, unknown> {
  const properties: Schemas["PrefixPatch"]["properties"] = { [key]: value };
  return { properties };
}

/**
 * What the operations of a draft change in the properties of the object `id`, by key: the
 * value assigned, `null` removed, the last operation winning.
 */
export function draftedProperties(
  operations: readonly Operation[] | undefined,
  id: string,
  keyOf: (declaration: string) => string | undefined,
): Map<string, Raw | null> {
  const out = new Map<string, Raw | null>();
  for (const operation of operations ?? []) {
    const op = operation as Record<string, unknown>;
    if (op.op !== "assign_property" && op.op !== "unassign_property") continue;
    if ((op.object as { id?: unknown } | undefined)?.id !== id) continue;
    const declaration = (op.property as { id?: unknown } | undefined)?.id;
    const key = typeof declaration === "string" ? keyOf(declaration) : undefined;
    if (!key) continue;
    const [first] = (op.values as unknown[] | undefined) ?? [];
    const raw =
      typeof first === "string" || typeof first === "number" || typeof first === "boolean"
        ? first
        : null;
    out.set(key, op.op === "assign_property" ? raw : null);
  }
  return out;
}

/** A node of an explanation, as the server writes it. */
type ChainNode = Schemas["NodeRef"];

/**
 * Where a resolved value comes from (REQ-003-14): the object's own, inherited (from
 * `source`, `null` when the user may not read it), computed (an expected value, no
 * assignment), or none.
 */
export type Origin =
  | { kind: "own" }
  | { kind: "inherited"; source: ChainNode | null }
  | { kind: "computed" }
  | { kind: "unset" };

/** The origin of a value from its explanation, read on the object `id`. */
export function originOf(explanation: Explanation, id: string): Origin {
  const origin = explanation.origin;
  if (origin) return origin.id === id ? { kind: "own" } : { kind: "inherited", source: origin };
  if (explanation.provenance) return { kind: "computed" };
  // A value whose origin is withheld comes from an object the user may not read.
  const value = explanation.value as unknown;
  return value === null || value === undefined
    ? { kind: "unset" }
    : { kind: "inherited", source: null };
}

/** A property of an object as read: its value as an editor holds it, as read, masked. */
export interface PropertyRead {
  /** The value as the editor of its cell holds it. */
  value: string;
  /** The value read (or drafted, or under way), as `PropertyValue` shows it. */
  read: unknown;
  masked: boolean;
}

/** A row of the tab: a property of the object, its value as read and as its cell holds it. */
export interface PropertyRow extends Record<string, unknown>, PropertyRead {
  key: string;
}

/** An object whose properties are edited, as a write needs it. */
export interface PropertyObject {
  id: string;
  /** What it is named (`192.0.2.0/24`). */
  name: string;
  /** Its version as read: what a write expects (`If-Match`, the `expected` of its guard). */
  version: string | null;
}

/** The cell of a property: the object and the key. */
export interface PropertyCell {
  id: string;
  key: string;
}

/** The motive asked for a property: the cell of the grid, and the object and key it holds. */
export interface PropertyAsk extends QuickAsk {
  object: string;
  key: string;
}

/** What a cell shows of an edit of this session. */
interface Shown {
  state: CellState | undefined;
  text: string;
  raw: Raw | null;
  /** The version of the object when it was written: the entry goes once it changes. */
  version: string | null;
  held?: string | undefined;
}

export interface PropertyEditDeps extends Pick<InlineDeps, "t" | "sentence" | "say"> {
  collection: () => Collection;
  /** The objects edited: the record's in its tab, the rows' in a list (task 12i). */
  objects: () => readonly PropertyObject[];
  /** The property `key` of the object `id` as read, `undefined` while it is not. */
  property: (id: string, key: string) => PropertyRead | undefined;
  /** Whether the read of the properties of `id` was refused (it will not come by itself). */
  unreadable?: (id: string) => boolean;
  declaration: (key: string) => Declared | undefined;
  declarationById: (id: string) => Declared | undefined;
  /** The label of a property, its key without one. */
  label: (key: string) => string;
  /** The label of a value of the closed list of a property, if any. */
  valueLabel: (key: string, value: string) => string | null;
  /** The properties of the object `id` read again (after a 412). */
  reread: (id: string) => void;
}

const fixed = (reason: FixedReason): NotEditable => ({
  kind: "fixed",
  reason,
});
const cellKey = (id: string, key: string) => `${id}\n${key}`;

/**
 * The properties of objects edited in place: of one record (its tab, `recordGrid`), or of
 * the rows of a list, one column a property (task 12i, `propertyColumns.ts`). Every rule
 * is the same for both; a cell is an object and a key.
 */
export function usePropertyEdit(deps: PropertyEditDeps) {
  const { t } = deps;
  const session = useSession();
  const time = useTime();
  const present = computed(() => time.value.present);
  /** The gate: the period and the width of the frame (REQ-021-29). */
  const writes = useWrites();
  /** Under 1024 px, the sentence that refuses the text of an editor still open. */
  const narrowed = () => (writes.value.refused === "narrow" ? t("edit.why.narrow") : undefined);
  const rights = computed(() => session.value.rights);
  const dates = () => ({ validAt: time.value.validAt, asOf: time.value.asOf });
  const objectById = computed(() => new Map(deps.objects().map((o) => [o.id, o])));
  const objectOf = (id: string): PropertyObject =>
    objectById.value.get(id) ?? { id, name: id, version: null };

  // The active draft of the tab: what it changes shows in the cells, « en brouillon ».
  const active = ref<string | null>(drafts.active());
  onScopeDispose(drafts.onActive((id) => (active.value = id)));
  const activeDraft = useQuery(() => ({
    ...drafts.get(active.value ?? "-"),
    enabled: active.value !== null,
  }));
  // By object: what the draft changes in the properties of each object edited.
  const drafted = computed(() => {
    const operations = (activeDraft.data.value as Entry<Draft> | undefined)?.data.operations;
    const touched = new Set<string>();
    for (const operation of operations ?? []) {
      const id = ((operation as Record<string, unknown>).object as { id?: unknown } | undefined)
        ?.id;
      if (typeof id === "string" && objectById.value.has(id)) touched.add(id);
    }
    const out = new Map<string, Map<string, Raw | null>>();
    for (const id of touched) {
      const values = draftedProperties(operations, id, (d) => deps.declarationById(d)?.key);
      if (values.size > 0) out.set(id, values);
    }
    return out;
  });
  const isDrafted = (id: string, key: string) => drafted.value.get(id)?.has(key) === true;

  // The origin of the values (their explanation, the cache's, as the popover reads it) and
  // the name of the object an inherited one comes from; the whole list of a closed list.
  const explained = shallowReactive(new Map<string, Origin>());
  // The name of a source, `null` when it could not be read (the sentence then names none).
  const sources = shallowReactive(new Map<string, string | null>());
  const choices = shallowReactive(new Map<string, readonly string[] | null>());
  async function sourceName(node: ChainNode): Promise<void> {
    if (sources.has(node.id)) return;
    const of =
      collectionOfKind(node.kind) ??
      collectionById(/\/v1\/([a-z-]+)\//.exec(node.href ?? "")?.[1] ?? "");
    const entry = of
      ? ((await queryClient
          .fetchQuery(reads.object(of.kind, node.id, dates()))
          .catch(() => undefined)) as Entry<Item> | undefined)
      : undefined;
    sources.set(node.id, entry ? nameOf(entry.data) : null);
  }
  /** The origin of the value of `key` of the object `id`, read when not known yet. */
  async function explain(id: string, key: string): Promise<Origin | undefined> {
    const known = explained.get(cellKey(id, key));
    if (known) return known;
    const read = reads.sub<Explanation>(
      deps.collection().kind,
      id,
      `properties/${encodeURIComponent(key)}/explanation`,
      null,
      dates(),
    );
    const entry = await queryClient.fetchQuery(read).catch(() => undefined);
    if (!entry) return undefined;
    const origin = originOf(entry.data, id);
    explained.set(cellKey(id, key), origin);
    if (origin.kind === "inherited" && origin.source) void sourceName(origin.source);
    return origin;
  }
  async function listOf(key: string): Promise<void> {
    if (choices.has(key)) return;
    const read = reads.list("/v1/property-declarations/{key}/values", null, null, { key });
    const pages = await queryClient.fetchInfiniteQuery(read).catch(() => undefined);
    const all = pages?.pages as Entry<Page<{ key: string }>>[] | undefined;
    // Only the whole list refuses a key; a part of it leaves the check to the server.
    const whole = all && !all.at(-1)?.data.links.next;
    choices.set(
      key,
      whole ? all.flatMap((page) => page.data.items.map((value) => value.key)) : null,
    );
  }
  /** The cell of the focus: its origin and, for a closed list, its values are read. */
  const focus = ref<PropertyCell | null>(null);
  watch(focus, (cell) => {
    const declaration = cell ? deps.declaration(cell.key) : undefined;
    if (!cell || !declaration) return;
    void explain(cell.id, cell.key);
    if (declaration.type === "enum") void listOf(cell.key);
  });

  // The edits of this session not yet in the properties read.
  const shown = shallowReactive(new Map<string, Shown>());
  // Other dates: what was known of the values goes.
  watch([() => time.value.validAt, () => time.value.asOf], () => {
    explained.clear();
    if (focus.value) void explain(focus.value.id, focus.value.key);
  });
  // Another version of an object (or the object gone): what was known of its values goes.
  let versions = new Map<string, string | null>();
  watch(objectById, (byId) => {
    for (const [at, entry] of [...shown]) {
      if (byId.get(at.split("\n")[0]!)?.version !== entry.version) shown.delete(at);
    }
    for (const at of [...explained.keys()]) {
      const id = at.split("\n")[0]!;
      if (!byId.has(id) || versions.get(id) !== byId.get(id)!.version) explained.delete(at);
    }
    versions = new Map([...byId].map(([id, object]) => [id, object.version]));
    if (focus.value && !explained.has(cellKey(focus.value.id, focus.value.key))) {
      void explain(focus.value.id, focus.value.key);
    }
  });

  /** The property as the grid shows it: the value of the draft, then the one under way. */
  function shownValue(id: string, key: string): PropertyRead | undefined {
    const read = deps.property(id, key);
    const entry = shown.get(cellKey(id, key));
    if (entry && (entry.state === undefined || entry.state === "pending")) {
      return { value: entry.text, read: entry.raw, masked: false };
    }
    const values = drafted.value.get(id);
    if (!values?.has(key)) return read;
    const value = values.get(key)!;
    return { value: propertyText(value), read: value, masked: false };
  }

  /** Why the value of `key` of the object `id` offers no editor, or `null` when it does. */
  function why(id: string, key: string): NotEditable | null {
    const read = deps.property(id, key);
    if (read?.masked) return fixed("masked");
    const declaration = deps.declaration(key);
    if (!declaration) return fixed("undeclared");
    if (!typedInPlace(declaration)) return fixed("typed");
    const of = deps.collection();
    if (!PROPERTIES_PATCHED[of.id]) return fixed("unpatched");
    // A value not read (a row of a list): never typed over blind; its read refused, said so.
    // Said after the right and before the gate (`rankedRefusal`), as a computed value.
    const own = !read
      ? fixed(deps.unreadable?.(id) ? "unreadable" : "unread")
      : explained.get(cellKey(id, key))?.kind === "computed"
        ? fixed("computed")
        : null;
    const editable = editability(rights.value, writes.value, of, `prop.${key}`, own);
    return editable.editable ? null : editable.why;
  }
  /** The permission Ctrl+Enter needs: the approval of the property's own (024). */
  const applyRight = (key: string) => approvalOf(propertyRight(key));
  const mayApply = (key: string) => rights.value.has(applyRight(key));

  /** A text typed as the property takes it: the words of a value of its list, its key. */
  function parse(declaration: Declared, text: string): string {
    if (declaration.type !== "enum") return text;
    const typed = text.trim().toLocaleLowerCase();
    const keys = choices.get(declaration.key) ?? [];
    return (
      keys.find(
        (key) =>
          key === text.trim() ||
          deps.valueLabel(declaration.key, key)?.toLocaleLowerCase() === typed,
      ) ?? text
    );
  }
  const owned = (id: string, key: string): boolean | undefined => {
    const origin = explained.get(cellKey(id, key));
    return origin ? origin.kind === "own" : undefined;
  };
  /** The refusal of a text typed for `key` of the object `id`, in words, or nothing. */
  function refusalOf(id: string, key: string, text: string): string | undefined {
    const declaration = deps.declaration(key);
    if (!declaration) return undefined;
    const list = choices.get(key) ?? null;
    const refused = checkProperty(declaration, parse(declaration, text), owned(id, key), list);
    return refused
      ? t(refused.key, {
          field: deps.label(key),
          ...(list ? { values: list.map((k) => deps.valueLabel(key, k) ?? k).join(", ") } : {}),
          ...refused.values,
        })
      : undefined;
  }
  /** The closed list of the cell of `key` of the object `id` (`propertyChoices`). */
  const choicesOf = (id: string, key: string): ListOption[] | undefined => {
    const declaration = deps.declaration(key);
    if (!declaration) return undefined;
    return propertyChoices(declaration, choices.get(key), {
      words: (value) => deps.valueLabel(key, value),
      own: owned(id, key) === true,
      none: t("edit.choice.none"),
    });
  };
  /**
   * What an edit of the cell of `key` writes, said before it is sent (Q-021-19): over an
   * inherited value, a value of the object's own; of its own, emptied, removed.
   */
  function noteOf(id: string, key: string): string | undefined {
    const origin = explained.get(cellKey(id, key));
    if (origin?.kind === "own") return t("edit.property.own");
    if (origin?.kind !== "inherited") return undefined;
    if (!origin.source) return t("edit.property.overridesUnreadable");
    // Said once the source is named, never by its identifier.
    if (!sources.has(origin.source.id)) return undefined;
    const source = sources.get(origin.source.id);
    return source ? t("edit.property.overrides", { source }) : t("edit.property.overridesUnnamed");
  }

  function cellState(id: string, key: string): CellState | undefined {
    const entry = shown.get(cellKey(id, key));
    if (entry) return entry.state;
    return isDrafted(id, key) ? "draft" : undefined;
  }
  /** Why the cell of `key` of `id` is held in review (its 202), said under the grid too. */
  const heldOf = (id: string, key: string) => {
    const entry = shown.get(cellKey(id, key));
    return entry?.state === "review" ? entry.held : undefined;
  };
  /**
   * The state of the row of the object `id`: in review when one of its values is held, in
   * the draft when the draft changes one; of the value of `key` alone (a row of the tab).
   */
  function rowState(id: string, key?: string): RowState | undefined {
    const keys = key === undefined ? null : [key];
    for (const [at, entry] of shown) {
      const [of, k] = at.split("\n");
      if (entry.state === "review" && of === id && (!keys || keys.includes(k!))) return "review";
    }
    if (key !== undefined) return isDrafted(id, key) ? "draft" : undefined;
    return drafted.value.has(id) ? "draft" : undefined;
  }
  /** Why the row of the object `id` is in review: the reasons of its values held, once each. */
  function rowReasons(id: string): string[] {
    const held = new Set<string>();
    for (const [at, entry] of shown) {
      if (entry.state === "review" && entry.held && at.startsWith(`${id}\n`)) held.add(entry.held);
    }
    return [...held];
  }

  const what = (id: string, key: string) =>
    t("edit.what", { field: deps.label(key), name: objectOf(id).name });

  /** Enter: an operation added to the active draft, the cell under way until it is saved. */
  async function toDraft(
    id: string,
    declaration: Declared,
    text: string,
    note = "",
  ): Promise<void> {
    if (!present.value) return;
    const key = declaration.key;
    const at = cellKey(id, key);
    const raw = propertyValueOf(declaration, text);
    const version = objectOf(id).version;
    shown.set(at, { state: "pending", text, raw, version });
    const kind = deps.collection().kind;
    const op = propertyOperation(kind, id, declaration.id, raw);
    // Made from the draft it goes to (read again after a 412): its guard only once.
    const outcome = await addToActiveDraft((current) => guarded(current, kind, id, version, op));
    if (outcome.kind === "saved") {
      shown.delete(at);
      const said = t("edit.drafted", { what: what(id, key) });
      deps.say.status(note ? `${note} ${said}` : said);
    } else {
      shown.set(at, { state: cellErrorOf(outcome, deps.sentence), text, raw, version });
    }
  }

  /** The motive asked in line (Ctrl+Enter), `null` when none. */
  const asking = shallowRef<PropertyAsk | null>(null);

  /** A commit of the grid on the cell of `key` of the object `id`. */
  async function onCommit(commit: GridCommit, { id, key }: PropertyCell): Promise<void> {
    const declaration = deps.declaration(key);
    if (!declaration || why(id, key) !== null) return;
    // Its origin known before it is written: a computed value has no editor.
    await explain(id, key);
    const late = why(id, key);
    if (late !== null) {
      // Known only now (a computed value): refused, and said.
      deps.say.status(whyWords(late, t) ?? t("edit.notNow"));
      return;
    }
    const at = cellKey(id, key);
    const version = objectOf(id).version;
    const text = parse(declaration, commit.value);
    const raw = propertyValueOf(declaration, text);
    // Checked again now that the origin is known: emptied, a value inherited has none to remove.
    const refused = refusalOf(id, key, commit.value);
    if (refused) {
      shown.set(at, { state: { error: refused }, text, raw, version });
      deps.say.status(refused);
      return;
    }
    if (commit.now && mayApply(key)) {
      const open = asking.value;
      if (
        open &&
        (open.busy || open.refusal) &&
        (open.row !== commit.row || open.column !== commit.column)
      ) {
        const refused = t("edit.askOpen", {
          what: what(open.object, open.key),
          value: commit.value,
        });
        shown.set(at, { state: { error: refused }, text, raw, version });
        deps.say.status(refused);
        return;
      }
      const list = choices.get(key);
      const field: EditField = list
        ? { name: key, kind: "choice", options: list }
        : { name: key, kind: "text" };
      asking.value = {
        row: commit.row,
        column: commit.column,
        object: id,
        key,
        collection: deps.collection(),
        field,
        label: deps.label(key),
        check: (value) => refusalOf(id, key, value),
        note: noteOf(id, key),
        name: objectOf(id).name,
        value: text,
        version,
        refusal: null,
        current: null,
        busy: false,
      };
      return;
    }
    const note = commit.now ? t("edit.applyForbidden", { permission: applyRight(key) }) : "";
    void toDraft(id, declaration, text, note);
  }

  /** The value of `key` of `id` read again (after a 412), as its cell holds it. */
  async function currentOf(id: string, key: string): Promise<string | null> {
    const read = reads.sub<Schemas["Resolved"]>(
      deps.collection().kind,
      id,
      "properties",
      null,
      dates(),
    );
    const entry = await queryClient.fetchQuery({ ...read, staleTime: 0 }).catch(() => undefined);
    return entry ? propertyText((entry.data.properties as Record<string, unknown>)[key]) : null;
  }

  /** Ctrl+Enter, the motive given: `properties` of the patch applied now, by its answer. */
  async function confirm(motive: string, text: string): Promise<boolean> {
    const ask = asking.value;
    if (!ask || ask.busy) return false;
    const { object: id, key } = ask;
    // The frame narrowed meanwhile: the line and its motive kept, the width said.
    const width = narrowed();
    if (width) {
      deps.say.status(width);
      return false;
    }
    const declaration = deps.declaration(key);
    if (!declaration || why(id, key) !== null || !mayApply(key)) {
      cancel();
      deps.say.status(t("edit.notNow"));
      return false;
    }
    const typed = parse(declaration, text);
    const raw = propertyValueOf(declaration, typed);
    const at = cellKey(id, key);
    const set = (state: CellState | undefined, held?: string) =>
      void shown.set(at, {
        state,
        text: typed,
        raw,
        version: ask.version,
        ...(held ? { held } : {}),
      });
    return quickWrite({
      asking,
      ask,
      text: typed,
      request: {
        key: keys.object(deps.collection().kind, id),
        patch: propertyPatch(key, raw),
        motive,
        ...(ask.version ? { ifMatch: `"${ask.version}"` } : {}),
      },
      what: what(id, key),
      cell: {
        pending: () => set("pending"),
        applied: () => (objectOf(id).version === ask.version ? set(undefined) : shown.delete(at)),
        held: (reason) => set("review", reason),
        invalid: (error) => set(error),
        clear: () => void shown.delete(at),
      },
      reread: () => deps.reread(id),
      current: async (refusal) => {
        const entry = (refusal.kind === "conflict" ? refusal.current : undefined) as
          Entry<Item> | undefined;
        const version = typeof entry?.data.version === "string" ? entry.data.version : null;
        return { text: await currentOf(id, key), version };
      },
      t,
      sentence: deps.sentence,
      say: deps.say,
    });
  }

  /** The line of the motive closed without applying: the cell back to the value read. */
  function cancel(): void {
    const ask = asking.value;
    if (!ask) return;
    const at = cellKey(ask.object, ask.key);
    if (typeof shown.get(at)?.state === "object") shown.delete(at);
    asking.value = null;
  }
  watch(present, (now) => {
    if (!now && asking.value) cancel();
  });

  return {
    shownValue,
    cellState,
    heldOf,
    rowState,
    rowReasons,
    // An editor open when the frame narrowed: kept with its text, the width said (`inline.ts`).
    validate: (id: string, key: string, text: string) => narrowed() ?? refusalOf(id, key, text),
    choices: choicesOf,
    why,
    noteOf,
    mayApply,
    applyRight,
    focus,
    asking,
    onCommit,
    confirm,
    cancel,
  };
}

export type PropertyEdit = ReturnType<typeof usePropertyEdit>;

/**
 * The grid of the tab « Propriétés » of a record (task 12h): its rows are the properties of
 * the one object `id`, its column `value` edits them.
 */
export function recordGrid(
  edit: PropertyEdit,
  id: () => string,
  rows: () => readonly PropertyRow[],
) {
  const value = (column: GridColumn) => column.id === "value";
  const has = (key: string) => rows().some((row) => row.key === key);
  return {
    shownRows: computed<PropertyRow[]>(() =>
      rows().map((row) => {
        const shown = edit.shownValue(id(), row.key);
        return shown ? { ...row, ...shown, masked: row.masked } : row;
      }),
    ),
    editable: (row: PropertyRow, column: GridColumn) =>
      value(column) && edit.why(id(), row.key) === null,
    validate: (row: PropertyRow, column: GridColumn, text: string) =>
      value(column) ? edit.validate(id(), row.key, text) : undefined,
    options: (row: PropertyRow, column: GridColumn) =>
      value(column) ? edit.choices(id(), row.key) : undefined,
    cellState: (row: PropertyRow, column: GridColumn) =>
      value(column) ? edit.cellState(id(), row.key) : undefined,
    stateReason: (row: PropertyRow, column: GridColumn) =>
      value(column) ? edit.heldOf(id(), row.key) : undefined,
    rowState: (row: PropertyRow) => edit.rowState(id(), row.key),
    /** Why a row is in review: the reason of its value held, said by its first cell. */
    rowReason: (row: PropertyRow) => edit.heldOf(id(), row.key),
    why: (row: PropertyRow) => edit.why(id(), row.key),
    whyOf: (key: string) => (has(key) ? edit.why(id(), key) : null),
    noteOf: (key: string) => edit.noteOf(id(), key),
    heldOf: (key: string) => edit.heldOf(id(), key),
    mayApply: edit.mayApply,
    applyRight: edit.applyRight,
    /** The key of the cell of the focus: its origin and its list are read. */
    focus: computed<string | null>({
      get: () => edit.focus.value?.key ?? null,
      set: (key) => (edit.focus.value = key === null ? null : { id: id(), key }),
    }),
    asking: edit.asking,
    onCommit: (commit: GridCommit) =>
      commit.column === "value" ? edit.onCommit(commit, { id: id(), key: commit.row }) : undefined,
    confirm: edit.confirm,
    cancel: edit.cancel,
  };
}
