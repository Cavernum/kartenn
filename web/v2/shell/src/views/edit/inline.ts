import { addToActiveDraft } from "@kartenn/sdk/host";
import {
  drafts,
  keys,
  queryClient,
  reads,
  write,
  type Draft,
  type Entry,
  type PendingReason,
  type QuickWrite,
  type Refusal as SdkRefusal,
} from "@kartenn/sdk";
import type { CellRef, CellState, GridColumn, GridCommit, ListOption, RowState } from "@kartenn/ui";
import {
  computed,
  onScopeDispose,
  ref,
  shallowReactive,
  shallowRef,
  watch,
  type ShallowRef,
} from "vue";
import { useQuery } from "../../data/query.ts";
import { problemCode } from "../../problems/index.ts";
import { useSession } from "../../rights/index.ts";
import { useTime, writesOf } from "../../time/index.ts";
import { nameOf, type Collection, type Item } from "../collections.ts";
import {
  applyRight,
  checkValue,
  draftedValues,
  editability,
  EDITS,
  modificationField,
  patchOf,
  textOf,
  valueOf,
  type EditField,
  type NotEditable,
  type Values,
} from "./fields.ts";
import { heldWords } from "./say.ts";

// In-place editing of a list of the referential (REQ-021-15, 16; design 021 « Édition en
// place »): the cells the user may edit (rights, present, contract), checked by the types
// of the contract; Enter adds an operation to the active draft (W10, the version read as
// `expected`) and the cell shows « en brouillon » from the draft itself; Ctrl+Enter asks
// for a motive in line (prefilled) and applies by `write.quick` (`?approval=self`) — the
// value shown before the answer (present only), then: 200 applied, 202 « en revue » (never
// applied; why, the description of the cell: `stateReason`), 412 the object read again
// with the input kept beside its current value, 422 the violations on the cell with their
// codes, 403 a refusal, the input kept. Without
// `X.approval.self`, Ctrl+Enter goes to the draft and says why. Rows of several kinds (the
// nodes of the topography, task 12a) give the collection of each row and the field each of
// their columns edits; a closed list is chosen in the cell (`choices`, task 12g), and a word
// of it typed (a host's editor of text) stands for its key.

/** A refused try, as the line of the motive shows it (the SDK's, whatever its input). */
export type Refusal = SdkRefusal<unknown>;

/** The motive asked in line for an edit applied now (Ctrl+Enter). */
export interface QuickAsk {
  row: string;
  /** The column of the grid, where the focus goes back. */
  column: string;
  /**
   * The grid it was asked from, when several show the same rows (the tree of the topography
   * and the content of its node, task 12f2): the line shows under that one alone.
   */
  grid?: string | undefined;
  /** The collection of the row, and the field the column edits. */
  collection: Collection;
  field: EditField;
  /** The words of the field, `views.field.<name>` by default (a property: its label). */
  label?: string;
  /** The local check of the value typed, `checkValue` of the field by default. */
  check?: (value: string) => string | undefined;
  /** Said before applying: a value of the object's own over an inherited one (Q-021-19). */
  note?: string | undefined;
  /** What the object is named (`192.0.2.0/24`). */
  name: string;
  /** The value typed: kept whatever the answer. */
  value: string;
  /** The version the write expects (`If-Match`): the one read, the current one after a 412. */
  version: string | null;
  /** The last refusal, and after a 412 the value read again. */
  refusal: Refusal | null;
  current: string | null;
  busy: boolean;
}

/** What a cell shows of an edit of this session: its state and the value it shows. */
interface Shown {
  state: CellState | undefined;
  value: string | boolean;
  /**
   * The version of the row when it was written: the entry goes once the row changes — the
   * row of the grid it was written from (`grid`), whose page gave that version.
   */
  version: string | null;
  grid?: string | undefined;
  /** Held in review (202): why, as the cell says it (REQ-021-15). */
  held?: string | undefined;
}

/** What the view says of an outcome (the toasts of the frame, a polite status). */
export interface InlineSay {
  /** Applied (200, 201), with its changeset. */
  applied(what: string, changeset: string | null): void;
  /** Held (202), with its changeset and why. */
  proposed(what: string, changeset: string | null, reason: PendingReason | null): void;
  /** A sentence for the polite status of the view. */
  status(text: string): void;
}

export interface InlineDeps {
  /** The collection of the rows: of `row` when they mix kinds, the default one without. */
  collection: (row?: Item) => Collection;
  rows: () => readonly Item[];
  /**
   * The row `id` as the grid `grid` shows it, when several grids show the same objects
   * (its version is the one of the page that grid read); among `rows` by default.
   */
  rowOf?: ((id: string, grid?: string) => Item | undefined) | undefined;
  /** The field the column `column` of `row` edits, none for a column of no field. */
  field?: (row: Item, column: string) => string | undefined;
  /** The words of a key of a closed list: typed in a cell, they stand for the key. */
  words?: (field: string, key: string) => string | null;
  /** After a 412, the rows read again: the lists of the collection by default. */
  reread?: (row: string, collection: Collection) => void;
  /** The words: `t` of the shell. */
  t: (key: string, values?: Record<string, unknown>) => string;
  /** A problem in plain words (`useProblems().sentence`). */
  sentence: (error: unknown) => string;
  say: InlineSay;
}

/** After a 412, the lists of the collection read again (`InlineDeps.reread` by default). */
export const rereadLists = (_row: string, of: Collection): void =>
  void queryClient.invalidateQueries({ queryKey: keys.allOn(of.path) });

const keyOf = (row: string, column: string) => `${row}\n${column}`;
const versionOf = (item: Item | undefined) =>
  typeof item?.version === "string" ? item.version : null;

/**
 * The message and the code of a refusal, as a cell shows them: the first violation and its
 * rule, else the problem in plain words and its `type` (`problems/`).
 */
export function cellErrorOf(
  refusal: Refusal,
  sentence: InlineDeps["sentence"],
): { error: string; code?: string } {
  if (refusal.kind === "invalid") {
    const first = [...Object.values(refusal.byField).flat(), ...refusal.general][0];
    if (first) return { error: first.detail, code: first.rule };
  }
  const code = problemCode(refusal.problem);
  const error = sentence(refusal.problem);
  return code ? { error, code } : { error };
}

/**
 * Why a cell offers no editor, as one sentence (its description); none for no field.
 * `dated`: the host's own sentence under a validity date, when « Modifier » there does not
 * change the row of the cell (the record of a content tab, the node chosen beside it).
 */
export function whyWords(
  why: NotEditable | null,
  t: (key: string) => string,
  dated?: string,
): string | undefined {
  if (!why || why.kind === "none") return undefined;
  if (why.kind === "forbidden") return `${t("edit.why.forbidden")} ${why.permission}`;
  if (why.kind === "past") return t("edit.why.past");
  if (why.kind === "dated") return dated ?? t("edit.why.inPlaceDated");
  return t(`edit.why.${why.reason}`);
}

/** What a cell shows of a write applied now, as `quickWrite` renders its answer. */
export interface QuickCell {
  /** Under way: the value shown before the answer (D-7, at the present only). */
  pending(): void;
  /** Applied (200). */
  applied(): void;
  /** Held in review (202), and why. */
  held(reason: string | undefined): void;
  /** Refused by its violations (422), on the cell. */
  invalid(error: { error: string; code?: string }): void;
  /** Back to the value read (412, 403, a failure). */
  clear(): void;
}

/** A write applied now (Ctrl+Enter), the motive given: of a field, or of a property. */
export interface QuickRun<A extends QuickAsk> {
  /** The line of the motive, and what it asks. */
  asking: ShallowRef<A | null>;
  ask: A;
  /** The value as typed, kept in the line whatever the answer. */
  text: string;
  request: QuickWrite;
  /** What the write is said as (`edit.what`). */
  what: string;
  cell: QuickCell;
  /** After a 412: the rows read again, then the value and version read now. */
  reread: () => void;
  current: (refusal: Refusal) => Promise<{ text: string | null; version: string | null }>;
  t: InlineDeps["t"];
  sentence: InlineDeps["sentence"];
  say: InlineSay;
}

/**
 * The write applied now under `If-Match` (`write.quick`), rendered by its answer, the same
 * for a field of a list (`useInlineEdit`) and a property (`usePropertyEdit`): 200 applied,
 * 202 « en revue » and why, the line closed; 412 the input kept beside the value read
 * again, 422 its violations on the cell, else the refusal on the line, which stays.
 */
export async function quickWrite<A extends QuickAsk>(run: QuickRun<A>): Promise<boolean> {
  const { asking, ask, text, cell } = run;
  asking.value = { ...ask, value: text, busy: true };
  cell.pending();
  const outcome = await write
    .quick(run.request)
    .catch(() => ({ kind: "failed", input: {}, problem: null }) as Refusal);
  if (outcome.kind === "applied") {
    cell.applied();
    asking.value = null;
    run.say.applied(run.what, outcome.changeset);
    return true;
  }
  if (outcome.kind === "proposed") {
    cell.held(heldWords(run.t, outcome.reason));
    asking.value = null;
    run.say.proposed(run.what, outcome.changeset, outcome.reason);
    return true;
  }
  if (outcome.kind === "conflict") {
    cell.clear();
    // The rows read again; the line keeps the input beside its new value.
    run.reread();
    const current = await run.current(outcome);
    asking.value = {
      ...ask,
      value: text,
      busy: false,
      refusal: outcome,
      current: current.text,
      version: current.version ?? ask.version,
    };
    return false;
  }
  if (outcome.kind === "invalid") cell.invalid(cellErrorOf(outcome, run.sentence));
  else cell.clear();
  asking.value = { ...ask, value: text, busy: false, refusal: outcome };
  return false;
}

export function useInlineEdit({
  collection,
  rows,
  rowOf,
  field: fieldOfColumn = (_row, column) => column,
  words,
  reread = rereadLists,
  t,
  sentence,
  say,
}: InlineDeps) {
  const session = useSession();
  const time = useTime();
  const present = computed(() => time.value.present);
  const rights = computed(() => session.value.rights);

  // The active draft of the tab: what it changes shows in the cells, « en brouillon ».
  const active = ref<string | null>(drafts.active());
  onScopeDispose(drafts.onActive((id) => (active.value = id)));
  const activeDraft = useQuery(() => ({
    ...drafts.get(active.value ?? "-"),
    enabled: active.value !== null,
  }));
  // Of every collection the rows are of (identifiers are unique across them).
  const drafted = computed(() => {
    const operations = (activeDraft.data.value as Entry<Draft> | undefined)?.data.operations;
    const ids = new Set(rows().map((row) => collection(row).id));
    if (ids.size === 0) ids.add(collection().id);
    const out = new Map<string, Values>();
    for (const id of ids) {
      for (const [row, values] of draftedValues(operations, id)) out.set(row, values);
    }
    return out;
  });
  /** The field the column of a row edits (`detail` of a prefix: `nature`), if any. */
  const fieldName = (row: Item | undefined, column: string) =>
    row ? fieldOfColumn(row, column) : column;

  /**
   * A text typed in a cell as the field takes it: for a closed list given its words, the
   * key whose words were typed (« Conteneur » → `container`), else the text itself.
   */
  function parse(field: EditField, text: string): string {
    if (field.kind !== "choice" || !words || field.options?.includes(text)) return text;
    const typed = text.trim().toLocaleLowerCase();
    return (
      field.options?.find(
        (key) => key === typed || words(field.name, key)?.toLocaleLowerCase() === typed,
      ) ?? text
    );
  }
  /** The choices of a closed list as the refusal of a cell lists them. */
  const choicesOf = (field: EditField) =>
    (field.options ?? []).map((key) => words?.(field.name, key) ?? key).join(", ");

  // The edits of this session not yet in the rows read: under way, applied, in review, refused.
  const shown = shallowReactive(new Map<string, Shown>());
  const rowById = computed(() => new Map(rows().map((row) => [String(row.id), row])));
  /** The row `id` as the grid `grid` shows it (its version is that grid's page's). */
  const rowIn = (id: string, grid?: string): Item | undefined =>
    rowOf?.(id, grid) ?? rowById.value.get(id);
  // An entry goes once its row is read at another version — the row of the grid it was
  // written from, at the version of that grid's page: applied, the review decided, or what
  // a refusal or a write under way was about is no longer the object read. Any row read
  // again changes `rowById`, whichever grid read it.
  watch(rowById, () => {
    for (const [key, entry] of [...shown]) {
      const row = rowIn(key.split("\n")[0]!, entry.grid);
      if (!row || versionOf(row) !== entry.version) shown.delete(key);
    }
  });

  /**
   * A row as a grid shows it: the values of the draft, then those of this session under way
   * or applied; a value in review (202) is not the object's: the cell keeps the value read,
   * marked « en revue » (design « Édition en place »). Any row of these objects, as a grid
   * reads it (the content of a node beside the tree, task 12f2).
   */
  function shownRow<R extends Item>(row: R): R {
    const id = String(row.id);
    const fromDraft = drafted.value.get(id);
    let mine: Values | null = null;
    for (const column of EDITS[collection(row).id].modification) {
      const entry = shown.get(keyOf(id, column.name));
      if (entry && (entry.state === undefined || entry.state === "pending")) {
        (mine ??= {})[column.name] = entry.value;
      }
    }
    return fromDraft || mine ? { ...row, ...fromDraft, ...mine } : row;
  }
  /** The rows as the grid shows them (`shownRow`). */
  const shownRows = computed<Item[]>(() => rows().map(shownRow));

  function cellState(row: Item, column: GridColumn): CellState | undefined {
    const name = fieldName(row, column.id);
    if (name === undefined) return undefined;
    const entry = shown.get(keyOf(String(row.id), name));
    if (entry?.state !== undefined) return entry.state;
    if (entry) return undefined;
    return drafted.value.get(String(row.id))?.[name] !== undefined ? "draft" : undefined;
  }
  /** Why the cell of `column` is held in review (the reason of its 202), if it is. */
  function heldOf(row: Item, column: string): string | undefined {
    const name = fieldName(row, column);
    if (name === undefined) return undefined;
    const entry = shown.get(keyOf(String(row.id), name));
    return entry?.state === "review" ? entry.held : undefined;
  }
  /** Why a cell is held in review: its description and tooltip (`heldOf`). */
  const stateReason = (row: Item, column: GridColumn): string | undefined => heldOf(row, column.id);
  function rowState(row: Item): RowState | undefined {
    const id = String(row.id);
    for (const [key, entry] of shown) {
      if (entry.state === "review" && key.startsWith(`${id}\n`)) return "review";
    }
    return drafted.value.has(id) ? "draft" : undefined;
  }
  /**
   * Why a row is in review: the reasons of its writes held, each once, whichever column
   * shows them (its first cell says them, beside its hidden word).
   */
  function rowReason(row: Item): string[] | undefined {
    const id = String(row.id);
    const held = new Set<string>();
    for (const [key, entry] of shown) {
      if (entry.state === "review" && entry.held && key.startsWith(`${id}\n`)) held.add(entry.held);
    }
    return held.size ? [...held] : undefined;
  }

  /** Why the field `name` of an object of `of` offers no editor, or `null` when it does. */
  function whyField(of: Collection, name: string | undefined): NotEditable | null {
    if (name === undefined) return { kind: "none" };
    const editable = editability(rights.value, writesOf(time.value), of, name);
    if (!editable.editable) return editable.why;
    // A column this list cannot write yet (a property): no editor rather than a lost input.
    return modificationField(of.id, name) ? null : { kind: "none" };
  }
  /** Why the cell of `column` (of `row`, when the rows mix kinds) offers no editor. */
  const why = (column: string, row?: Item): NotEditable | null =>
    whyField(collection(row), fieldName(row, column));
  const editable = (row: Item, column: GridColumn) => why(column.id, row) === null;
  /** Whether Ctrl+Enter applies (`X.approval.self`) to `row`, else it goes to the draft. */
  const mayApply = (row?: Item) => rights.value.has(applyRight(collection(row).type));
  const canApply = computed(() => mayApply());
  /** The field a cell of `row` edits, if any. */
  const fieldOf = (row: Item, column: string) => {
    const name = fieldName(row, column);
    return name === undefined ? undefined : modificationField(collection(row).id, name);
  };

  /**
   * The closed list a cell of `row` chooses among (task 12g): the keys of a nature or a
   * usage, held as the cell holds them, shown in their words (a key without words, in
   * fixed pitch); a flag's `true` and `false`, as its cell writes them; none for a field
   * typed in.
   */
  function choices(row: Item, column: GridColumn): ListOption[] | undefined {
    const field = fieldOf(row, column.id);
    if (field?.kind === "flag")
      return ["true", "false"].map((value) => ({ value, label: value, mono: true }));
    if (field?.kind !== "choice") return undefined;
    return (field.options ?? []).map((key) => {
      const label = words?.(field.name, key) ?? null;
      return { value: key, label: label ?? key, mono: label === null };
    });
  }

  function validate(row: Item, column: GridColumn, value: string): string | undefined {
    const field = fieldOf(row, column.id);
    if (!field) return undefined;
    const refused = checkValue(field, valueOf(field, parse(field, value)));
    return refused ? t(refused, { values: choicesOf(field), field: column.label }) : undefined;
  }

  const fieldWords = (column: string) => t(`views.field.${column}`);
  const what = (column: string, name: string) =>
    t("edit.what", { field: fieldWords(column), name });

  /** Enter: an operation added to the active draft, the cell under way until it is saved. */
  async function toDraft(
    row: Item,
    field: EditField,
    text: string,
    note: string,
    grid: string | undefined,
  ): Promise<void> {
    // Nothing written off the present, the draft included (REQ-021-5, 15).
    if (!present.value) return;
    const id = String(row.id);
    const key = keyOf(id, field.name);
    const value = valueOf(field, text);
    // `expected`: the version served with the value shown (the same page, the same read).
    const version = versionOf(row);
    shown.set(key, { state: "pending", value, version, grid });
    const op = EDITS[collection(row).id].update(id, version, { [field.name]: value });
    const outcome = await addToActiveDraft([op]);
    if (outcome.kind === "saved") {
      shown.delete(key);
      const drafted = t("edit.drafted", { what: what(field.name, nameOf(row)) });
      say.status(note ? `${note} ${drafted}` : drafted);
    } else {
      shown.set(key, { state: cellErrorOf(outcome, sentence), value, version, grid });
    }
  }

  /** The motive asked in line (Ctrl+Enter), `null` when none. */
  const asking = shallowRef<QuickAsk | null>(null);

  /** A cell committed; `grid`: the one it was typed in, when several show these rows. */
  function onCommit(commit: GridCommit, grid?: string): void {
    // The row of the grid it was typed in: its version is that of the value shown there.
    const row = rowIn(commit.row, grid);
    const field = row ? fieldOf(row, commit.column) : undefined;
    if (!row || !field || why(commit.column, row) !== null) return;
    const text = parse(field, commit.value);
    if (commit.now && mayApply(row)) {
      // A write under way, or a refusal still open (a 412 and its input), is never replaced:
      // the new input is said refused, with its value, on its cell and in the status.
      const open = asking.value;
      if (
        open &&
        (open.busy || open.refusal) &&
        (open.row !== commit.row || open.column !== commit.column)
      ) {
        const refused = t("edit.askOpen", {
          what: what(open.field.name, open.name),
          value: commit.value,
        });
        shown.set(keyOf(commit.row, field.name), {
          state: { error: refused },
          value: text,
          version: versionOf(row),
          grid,
        });
        say.status(refused);
        return;
      }
      asking.value = {
        row: commit.row,
        column: commit.column,
        grid,
        collection: collection(row),
        field,
        name: nameOf(row),
        value: text,
        version: versionOf(row),
        refusal: null,
        current: null,
        busy: false,
      };
      return;
    }
    // Without the right to apply, one sentence says both: why not now, and where it went.
    const note = commit.now
      ? t("edit.applyForbidden", { permission: applyRight(collection(row).type) })
      : "";
    void toDraft(row, field, text, note, grid);
  }

  /**
   * The object read again after a 412, the cache's answer else read now, and its version:
   * the one its body serves with that value (one snapshot, 11d), never another read.
   */
  async function currentOf(
    of: Collection,
    id: string,
    refusal: Refusal,
  ): Promise<{ data: Item; version: string | null } | undefined> {
    const entry =
      refusal.kind === "conflict" && refusal.current
        ? (refusal.current as Entry<Item>)
        : ((await queryClient
            .fetchQuery({ ...reads.object(of.kind, id), staleTime: 0 })
            .catch(() => undefined)) as Entry<Item> | undefined);
    return entry ? { data: entry.data, version: versionOf(entry.data) } : undefined;
  }

  /** Ctrl+Enter, the motive given: the write applied now, rendered by its answer. */
  async function confirm(motive: string, text: string): Promise<boolean> {
    const ask = asking.value;
    if (!ask || ask.busy) return false;
    const of = ask.collection;
    // Checked again when it is written: still at the present, still with the rights.
    if (whyField(of, ask.field.name) !== null || !rights.value.has(applyRight(of.type))) {
      cancel();
      say.status(t("edit.notNow"));
      return false;
    }
    const field = ask.field;
    const value = valueOf(field, text);
    const key = keyOf(ask.row, field.name);
    const set = (state: CellState | undefined, held?: string) =>
      void shown.set(key, {
        state,
        value,
        version: ask.version,
        grid: ask.grid,
        ...(held ? { held } : {}),
      });
    return quickWrite({
      asking,
      ask,
      text,
      request: {
        key: keys.object(of.kind, ask.row),
        patch: patchOf([field], { [field.name]: value }),
        motive,
        ...(ask.version ? { ifMatch: `"${ask.version}"` } : {}),
      },
      what: what(field.name, ask.name),
      cell: {
        pending: () => set("pending"),
        // Shown until the list is read again (the grid's it was asked from), unless it already is.
        applied: () =>
          versionOf(rowIn(ask.row, ask.grid)) === ask.version ? set(undefined) : shown.delete(key),
        held: (reason) => set("review", reason),
        invalid: (error) => set(error),
        clear: () => void shown.delete(key),
      },
      reread: () => reread(ask.row, of),
      current: async (refusal) => {
        const current = await currentOf(of, ask.row, refusal);
        return {
          text: current ? textOf(current.data[field.name]) : null,
          version: current?.version ?? null,
        };
      },
      t,
      sentence,
      say,
    });
  }

  /** The line of the motive closed without applying: the cell back to the value read. */
  function cancel(): void {
    const ask = asking.value;
    if (!ask) return;
    const key = keyOf(ask.row, ask.field.name);
    if (typeof shown.get(key)?.state === "object") shown.delete(key);
    asking.value = null;
  }

  // Off the present (the date pill), the line of the motive closes: nothing is written there.
  watch(present, (now) => {
    if (!now && asking.value) cancel();
  });

  /** The row and the column of a cell, as the grid keys it. */
  const cellOf = (ask: QuickAsk): CellRef => ({ row: ask.row, column: ask.column });
  /** The motive asked from the grid `grid` (none: from a grid that gave none), or `null`. */
  const askOf = (grid?: string): QuickAsk | null =>
    asking.value?.grid === grid ? asking.value : null;

  return {
    shownRows,
    shownRow,
    cellState,
    stateReason,
    heldOf,
    rowState,
    rowReason,
    editable,
    choices,
    validate,
    why,
    canApply,
    mayApply,
    present,
    asking,
    onCommit,
    confirm,
    cancel,
    cellOf,
    askOf,
  };
}

export type InlineEdit = ReturnType<typeof useInlineEdit>;
