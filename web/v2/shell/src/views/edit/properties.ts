import { addToActiveDraft } from "@kartenn/sdk/host";
import {
  drafts,
  keys,
  queryClient,
  reads,
  write,
  type components,
  type Draft,
  type Entry,
  type Operation,
  type Page,
} from "@kartenn/sdk";
import type { CellState, GridColumn, GridCommit, RowState } from "@kartenn/ui";
import { computed, onScopeDispose, ref, shallowReactive, shallowRef, watch } from "vue";
import { useQuery } from "../../data/query.ts";
import { useSession } from "../../rights/index.ts";
import { useTime, writesOf } from "../../time/index.ts";
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
import { cellErrorOf, whyWords, type InlineDeps, type QuickAsk, type Refusal } from "./inline.ts";
import { heldWords } from "./say.ts";

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
/** A label of a host name (RFC 1123 §2.1), a non-ASCII letter kept for its A-label. */
const LABEL = /^(?!-)[\p{L}\p{N}-]{1,63}(?<!-)$/u;

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
 * place »): the type and its parameters (length, pattern, bounds, a key of the closed list
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
      if (/[\u0000-\u001f\u007f-\u009f]/.test(typed)) return { key: "edit.check.control" };
      const length = [...typed].length;
      const out = bounded(
        "length",
        length,
        param(declaration, "min_len"),
        param(declaration, "max_len"),
      );
      if (out) return out;
      const pattern = param(declaration, "pattern");
      if (typeof pattern === "string" && pattern !== "") {
        let anchored: RegExp | null = null;
        try {
          anchored = new RegExp(`^(?:${pattern})$`, "u");
        } catch {
          // A pattern this engine does not read is the server's alone.
        }
        if (anchored && !anchored.test(typed))
          return { key: "edit.check.pattern", values: { pattern } };
      }
      return undefined;
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
      const day = y ? new Date(Date.UTC(Number(y), Number(m) - 1, Number(d))) : null;
      const valid = day?.toISOString().slice(0, 10) === typed;
      return valid ? undefined : { key: "edit.check.date" };
    }
    case "fqdn": {
      const name = typed.replace(/\.$/, "");
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
interface ChainNode {
  id: string;
  kind?: string | null;
  href?: string | null;
}

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
  const origin = explanation.origin as ChainNode | null | undefined;
  if (origin) return origin.id === id ? { kind: "own" } : { kind: "inherited", source: origin };
  if (explanation.provenance) return { kind: "computed" };
  // A value whose origin is withheld comes from an object the user may not read.
  const value = explanation.value as unknown;
  return value === null || value === undefined
    ? { kind: "unset" }
    : { kind: "inherited", source: null };
}

/** A row of the tab: a property of the object, its value as read and as its cell holds it. */
export interface PropertyRow extends Record<string, unknown> {
  key: string;
  /** The value as the editor of its cell holds it. */
  value: string;
  /** The value read (or drafted, or under way), as `PropertyValue` shows it. */
  read: unknown;
  masked: boolean;
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
  id: () => string;
  /** What the object is named (`192.0.2.0/24`). */
  name: () => string;
  /** The version of the object read with its record (`If-Match`). */
  version: () => string | null;
  rows: () => readonly PropertyRow[];
  declaration: (key: string) => Declared | undefined;
  declarationById: (id: string) => Declared | undefined;
  /** The label of a property, its key without one. */
  label: (key: string) => string;
  /** The label of a value of the closed list of a property, if any. */
  valueLabel: (key: string, value: string) => string | null;
  /** The properties read again (after a 412). */
  reread: () => void;
}

const fixed = (reason: FixedReason): NotEditable => ({
  kind: "fixed",
  reason,
});

export function usePropertyEdit(deps: PropertyEditDeps) {
  const { t } = deps;
  const session = useSession();
  const time = useTime();
  const present = computed(() => time.value.present);
  const rights = computed(() => session.value.rights);
  const dates = () => ({ validAt: time.value.validAt, asOf: time.value.asOf });

  // The active draft of the tab: what it changes shows in the cells, « en brouillon ».
  const active = ref<string | null>(drafts.active());
  onScopeDispose(drafts.onActive((id) => (active.value = id)));
  const activeDraft = useQuery(() => ({
    ...drafts.get(active.value ?? "-"),
    enabled: active.value !== null,
  }));
  const drafted = computed(() =>
    draftedProperties(
      (activeDraft.data.value as Entry<Draft> | undefined)?.data.operations,
      deps.id(),
      (id) => deps.declarationById(id)?.key,
    ),
  );

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
  /** The origin of the value of `key`, read when not known yet. */
  async function explain(key: string): Promise<Origin | undefined> {
    const known = explained.get(key);
    if (known) return known;
    const read = reads.sub<Explanation>(
      deps.collection().kind,
      deps.id(),
      `properties/${encodeURIComponent(key)}/explanation`,
      null,
      dates(),
    );
    const entry = await queryClient.fetchQuery(read).catch(() => undefined);
    if (!entry) return undefined;
    const origin = originOf(entry.data, deps.id());
    explained.set(key, origin);
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
  const focus = ref<string | null>(null);
  watch(focus, (key) => {
    const declaration = key ? deps.declaration(key) : undefined;
    if (!key || !declaration) return;
    void explain(key);
    if (declaration.type === "enum") void listOf(key);
  });

  // The edits of this session not yet in the properties read.
  const shown = shallowReactive(new Map<string, Shown>());
  // Another object, other dates, another version: what was known of the values goes.
  watch([deps.id, deps.version, () => time.value.validAt, () => time.value.asOf], () => {
    explained.clear();
    for (const [key, entry] of [...shown]) {
      if (entry.version !== deps.version()) shown.delete(key);
    }
    if (focus.value) void explain(focus.value);
  });

  const rowOf = (key: string) => deps.rows().find((row) => row.key === key);

  /** The rows as the grid shows them: the values of the draft, then those under way. */
  const shownRows = computed<PropertyRow[]>(() =>
    deps.rows().map((row) => {
      const entry = shown.get(row.key);
      if (entry && (entry.state === undefined || entry.state === "pending")) {
        return { ...row, value: entry.text, read: entry.raw };
      }
      const value = drafted.value.get(row.key);
      return value === undefined ? row : { ...row, value: propertyText(value), read: value };
    }),
  );

  /** Why the value of `row` offers no editor, or `null` when it does. */
  function why(row: PropertyRow): NotEditable | null {
    if (row.masked) return fixed("masked");
    const declaration = deps.declaration(row.key);
    if (!declaration) return fixed("undeclared");
    if (!typedInPlace(declaration)) return fixed("typed");
    const of = deps.collection();
    if (!PROPERTIES_PATCHED[of.id]) return fixed("unpatched");
    const editable = editability(rights.value, writesOf(time.value), of, `prop.${row.key}`);
    if (!editable.editable) return editable.why;
    return explained.get(row.key)?.kind === "computed" ? fixed("computed") : null;
  }
  const whyOf = (key: string) => {
    const row = rowOf(key);
    return row ? why(row) : null;
  };
  const editable = (row: PropertyRow, column: GridColumn) =>
    column.id === "value" && why(row) === null;
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
  const owned = (key: string): boolean | undefined => {
    const origin = explained.get(key);
    return origin ? origin.kind === "own" : undefined;
  };
  /** The refusal of a text typed for `key`, in words, or nothing. */
  function refusalOf(key: string, text: string): string | undefined {
    const declaration = deps.declaration(key);
    if (!declaration) return undefined;
    const list = choices.get(key) ?? null;
    const refused = checkProperty(declaration, parse(declaration, text), owned(key), list);
    return refused
      ? t(refused.key, {
          field: deps.label(key),
          ...(list ? { values: list.map((k) => deps.valueLabel(key, k) ?? k).join(", ") } : {}),
          ...refused.values,
        })
      : undefined;
  }
  const validate = (row: PropertyRow, column: GridColumn, text: string) =>
    column.id === "value" ? refusalOf(row.key, text) : undefined;

  /**
   * What an edit of the cell of `key` writes, said before it is sent (Q-021-19): over an
   * inherited value, a value of the object's own; of its own, emptied, removed.
   */
  function noteOf(key: string): string | undefined {
    const origin = explained.get(key);
    if (origin?.kind === "own") return t("edit.property.own");
    if (origin?.kind !== "inherited") return undefined;
    if (!origin.source) return t("edit.property.overridesUnreadable");
    // Said once the source is named, never by its identifier.
    if (!sources.has(origin.source.id)) return undefined;
    const source = sources.get(origin.source.id);
    return source ? t("edit.property.overrides", { source }) : t("edit.property.overridesUnnamed");
  }

  function cellState(row: PropertyRow, column: GridColumn): CellState | undefined {
    if (column.id !== "value") return undefined;
    const entry = shown.get(row.key);
    if (entry) return entry.state;
    return drafted.value.has(row.key) ? "draft" : undefined;
  }
  function stateReason(row: PropertyRow, column: GridColumn): string | undefined {
    const entry = column.id === "value" ? shown.get(row.key) : undefined;
    return entry?.state === "review" ? entry.held : undefined;
  }
  function rowState(row: PropertyRow): RowState | undefined {
    if (shown.get(row.key)?.state === "review") return "review";
    return drafted.value.has(row.key) ? "draft" : undefined;
  }
  /** Why the cell of `key` is held in review, said under the grid too. */
  const heldOf = (key: string) => {
    const entry = shown.get(key);
    return entry?.state === "review" ? entry.held : undefined;
  };

  const what = (key: string) => t("edit.what", { field: deps.label(key), name: deps.name() });

  /** Enter: an operation added to the active draft, the cell under way until it is saved. */
  async function toDraft(declaration: Declared, text: string, note = ""): Promise<void> {
    if (!present.value) return;
    const key = declaration.key;
    const raw = propertyValueOf(declaration, text);
    const version = deps.version();
    shown.set(key, { state: "pending", text, raw, version });
    const op = propertyOperation(deps.collection().kind, deps.id(), declaration.id, raw);
    const outcome = await addToActiveDraft([op]);
    if (outcome.kind === "saved") {
      shown.delete(key);
      const said = t("edit.drafted", { what: what(key) });
      deps.say.status(note ? `${note} ${said}` : said);
    } else {
      shown.set(key, { state: cellErrorOf(outcome, deps.sentence), text, raw, version });
    }
  }

  /** The motive asked in line (Ctrl+Enter), `null` when none. */
  const asking = shallowRef<QuickAsk | null>(null);

  async function onCommit(commit: GridCommit): Promise<void> {
    const row = rowOf(commit.row);
    const declaration = row ? deps.declaration(row.key) : undefined;
    if (!row || !declaration || commit.column !== "value" || why(row) !== null) return;
    // Its origin known before it is written: a computed value has no editor.
    await explain(row.key);
    const late = why(row);
    if (late !== null) {
      // Known only now (a computed value): refused, and said.
      deps.say.status(whyWords(late, t) ?? t("edit.notNow"));
      return;
    }
    const text = parse(declaration, commit.value);
    // Checked again now that the origin is known: emptied, a value inherited has none to remove.
    const refused = refusalOf(row.key, commit.value);
    if (refused) {
      const raw = propertyValueOf(declaration, text);
      shown.set(row.key, { state: { error: refused }, text, raw, version: deps.version() });
      deps.say.status(refused);
      return;
    }
    if (commit.now && mayApply(row.key)) {
      const open = asking.value;
      if (open && (open.busy || open.refusal) && open.row !== commit.row) {
        const refused = t("edit.askOpen", { what: what(open.row), value: commit.value });
        shown.set(row.key, {
          state: { error: refused },
          text,
          raw: propertyValueOf(declaration, text),
          version: deps.version(),
        });
        deps.say.status(refused);
        return;
      }
      const list = choices.get(row.key);
      const field: EditField = list
        ? { name: row.key, kind: "choice", options: list }
        : { name: row.key, kind: "text" };
      asking.value = {
        row: row.key,
        column: commit.column,
        collection: deps.collection(),
        field,
        label: deps.label(row.key),
        check: (value) => refusalOf(row.key, value),
        note: noteOf(row.key),
        name: deps.name(),
        value: text,
        version: deps.version(),
        refusal: null,
        current: null,
        busy: false,
      };
      return;
    }
    const note = commit.now ? t("edit.applyForbidden", { permission: applyRight(row.key) }) : "";
    void toDraft(declaration, text, note);
  }

  /** The value of `key` read again (after a 412), as its cell holds it. */
  async function currentOf(key: string): Promise<string | null> {
    const read = reads.sub<Schemas["Resolved"]>(
      deps.collection().kind,
      deps.id(),
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
    const key = ask.row;
    const declaration = deps.declaration(key);
    if (!declaration || whyOf(key) !== null || !mayApply(key)) {
      cancel();
      deps.say.status(t("edit.notNow"));
      return false;
    }
    const typed = parse(declaration, text);
    const raw = propertyValueOf(declaration, typed);
    asking.value = { ...ask, value: typed, busy: true };
    shown.set(key, { state: "pending", text: typed, raw, version: ask.version });
    const of = deps.collection();
    const outcome = await write
      .quick({
        key: keys.object(of.kind, deps.id()),
        patch: propertyPatch(key, raw),
        motive,
        ...(ask.version ? { ifMatch: `"${ask.version}"` } : {}),
      })
      .catch(() => ({ kind: "failed", input: {}, problem: null }) as Refusal);
    if (outcome.kind === "applied") {
      if (deps.version() === ask.version) {
        shown.set(key, { state: undefined, text: typed, raw, version: ask.version });
      } else shown.delete(key);
      asking.value = null;
      deps.say.applied(what(key), outcome.changeset);
      return true;
    }
    if (outcome.kind === "proposed") {
      shown.set(key, {
        state: "review",
        text: typed,
        raw,
        version: ask.version,
        held: heldWords(t, outcome.reason),
      });
      asking.value = null;
      deps.say.proposed(what(key), outcome.changeset, outcome.reason);
      return true;
    }
    if (outcome.kind === "conflict") {
      shown.delete(key);
      const current = outcome.current as Entry<Item> | undefined;
      const version = typeof current?.data.version === "string" ? current.data.version : null;
      deps.reread();
      asking.value = {
        ...ask,
        value: typed,
        busy: false,
        refusal: outcome,
        current: await currentOf(key),
        version: version ?? ask.version,
      };
      return false;
    }
    if (outcome.kind === "invalid") {
      shown.set(key, {
        state: cellErrorOf(outcome, deps.sentence),
        text: typed,
        raw,
        version: ask.version,
      });
    } else shown.delete(key);
    asking.value = { ...ask, value: typed, busy: false, refusal: outcome };
    return false;
  }

  /** The line of the motive closed without applying: the cell back to the value read. */
  function cancel(): void {
    const ask = asking.value;
    if (!ask) return;
    if (typeof shown.get(ask.row)?.state === "object") shown.delete(ask.row);
    asking.value = null;
  }
  watch(present, (now) => {
    if (!now && asking.value) cancel();
  });

  return {
    shownRows,
    cellState,
    stateReason,
    rowState,
    editable,
    validate,
    why,
    whyOf,
    noteOf,
    heldOf,
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
