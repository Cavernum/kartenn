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
  type Refusal as SdkRefusal,
} from "@kartenn/sdk";
import type { CellRef, CellState, GridColumn, GridCommit, RowState } from "@kartenn/ui";
import { computed, onScopeDispose, ref, shallowReactive, shallowRef, watch } from "vue";
import { useQuery } from "../../data/query.ts";
import { problemCode } from "../../problems/index.ts";
import { useSession } from "../../rights/index.ts";
import { useTime } from "../../time/index.ts";
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
} from "./fields.ts";

// In-place editing of a list of the referential (REQ-021-15, 16; design 021 « Édition en
// place »): the cells the user may edit (rights, present, contract), checked by the types
// of the contract; Enter adds an operation to the active draft (W10, the version read as
// `expected`) and the cell shows « en brouillon » from the draft itself; Ctrl+Enter asks
// for a motive in line (prefilled) and applies by `write.quick` (`?approval=self`) — the
// value shown before the answer (present only), then: 200 applied, 202 « en revue » (never
// applied), 412 the object read again with the input kept beside its current value, 422
// the violations on the cell with their codes, 403 a refusal, the input kept. Without
// `X.approval.self`, Ctrl+Enter goes to the draft and says why.

/** A refused try, as the line of the motive shows it (the SDK's, whatever its input). */
export type Refusal = SdkRefusal<unknown>;

/** The motive asked in line for an edit applied now (Ctrl+Enter). */
export interface QuickAsk {
  row: string;
  column: string;
  field: EditField;
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
  /** The version of the row when it was written: the entry goes once the row changes. */
  version: string | null;
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
  collection: () => Collection;
  rows: () => readonly Item[];
  /** The words: `t` of the shell. */
  t: (key: string, values?: Record<string, unknown>) => string;
  /** A problem in plain words (`useProblems().sentence`). */
  sentence: (error: unknown) => string;
  say: InlineSay;
}

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

export function useInlineEdit({ collection, rows, t, sentence, say }: InlineDeps) {
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
  const drafted = computed(() =>
    draftedValues(
      (activeDraft.data.value as Entry<Draft> | undefined)?.data.operations,
      collection().id,
    ),
  );

  // The edits of this session not yet in the rows read: under way, applied, in review, refused.
  const shown = shallowReactive(new Map<string, Shown>());
  const rowById = computed(() => new Map(rows().map((row) => [String(row.id), row])));
  // An entry goes once its row is read at another version: applied, the review decided, or
  // what a refusal or a write under way was about is no longer the object read.
  watch(rowById, (byId) => {
    for (const [key, entry] of [...shown]) {
      const row = byId.get(key.split("\n")[0]!);
      if (!row || versionOf(row) !== entry.version) shown.delete(key);
    }
  });

  /**
   * The rows as the grid shows them: the values of the draft, then those of this session
   * under way or applied; a value in review (202) is not the object's: the cell keeps the
   * value read, marked « en revue » (design « Édition en place »).
   */
  const shownRows = computed<Item[]>(() =>
    rows().map((row) => {
      const id = String(row.id);
      const fromDraft = drafted.value.get(id);
      let out: Item | null = fromDraft ? { ...row, ...fromDraft } : null;
      for (const column of EDITS[collection().id].modification) {
        const entry = shown.get(keyOf(id, column.name));
        if (entry && (entry.state === undefined || entry.state === "pending")) {
          out ??= { ...row };
          out[column.name] = entry.value;
        }
      }
      return out ?? row;
    }),
  );

  const columnOfRow = (row: Item, column: string) => keyOf(String(row.id), column);

  function cellState(row: Item, column: GridColumn): CellState | undefined {
    const entry = shown.get(columnOfRow(row, column.id));
    if (entry?.state !== undefined) return entry.state;
    if (entry) return undefined;
    return drafted.value.get(String(row.id))?.[column.id] !== undefined ? "draft" : undefined;
  }
  function rowState(row: Item): RowState | undefined {
    const id = String(row.id);
    for (const [key, entry] of shown) {
      if (entry.state === "review" && key.startsWith(`${id}\n`)) return "review";
    }
    return drafted.value.has(id) ? "draft" : undefined;
  }

  /** Why the cell of `column` offers no editor, or `null` when it does. */
  function why(column: string): NotEditable | null {
    const editable = editability(rights.value, present.value, collection(), column);
    if (!editable.editable) return editable.why;
    // A column this list cannot write yet (a property): no editor rather than a lost input.
    return modificationField(collection().id, column) ? null : { kind: "none" };
  }
  const editable = (_row: Item, column: GridColumn) => why(column.id) === null;
  /** Whether Ctrl+Enter applies (`X.approval.self`), else it goes to the draft. */
  const canApply = computed(() => rights.value.has(applyRight(collection().type)));

  function validate(_row: Item, column: GridColumn, value: string): string | undefined {
    const field = modificationField(collection().id, column.id);
    if (!field) return undefined;
    const refused = checkValue(field, valueOf(field, value));
    return refused
      ? t(refused, { values: (field.options ?? []).join(", "), field: column.label })
      : undefined;
  }

  const fieldWords = (column: string) => t(`views.field.${column}`);
  const what = (column: string, name: string) =>
    t("edit.what", { field: fieldWords(column), name });

  /** Enter: an operation added to the active draft, the cell under way until it is saved. */
  async function toDraft(row: Item, field: EditField, text: string, note = ""): Promise<void> {
    // Nothing written off the present, the draft included (REQ-021-5, 15).
    if (!present.value) return;
    const id = String(row.id);
    const key = keyOf(id, field.name);
    const value = valueOf(field, text);
    const version = versionOf(row);
    shown.set(key, { state: "pending", value, version });
    const op = EDITS[collection().id].update(id, version, { [field.name]: value });
    const outcome = await addToActiveDraft([op]);
    if (outcome.kind === "saved") {
      shown.delete(key);
      const drafted = t("edit.drafted", { what: what(field.name, nameOf(row)) });
      say.status(note ? `${note} ${drafted}` : drafted);
    } else {
      shown.set(key, { state: cellErrorOf(outcome, sentence), value, version });
    }
  }

  /** The motive asked in line (Ctrl+Enter), `null` when none. */
  const asking = shallowRef<QuickAsk | null>(null);

  function onCommit(commit: GridCommit): void {
    const row = rowById.value.get(commit.row);
    const field = modificationField(collection().id, commit.column);
    if (!row || !field || why(commit.column) !== null) return;
    if (commit.now && canApply.value) {
      // A write under way, or a refusal still open (a 412 and its input), is never replaced:
      // the new input is said refused, with its value, on its cell and in the status.
      const open = asking.value;
      if (
        open &&
        (open.busy || open.refusal) &&
        (open.row !== commit.row || open.column !== commit.column)
      ) {
        const refused = t("edit.askOpen", {
          what: what(open.column, open.name),
          value: commit.value,
        });
        shown.set(keyOf(commit.row, commit.column), {
          state: { error: refused },
          value: commit.value,
          version: versionOf(row),
        });
        say.status(refused);
        return;
      }
      asking.value = {
        row: commit.row,
        column: commit.column,
        field,
        name: nameOf(row),
        value: commit.value,
        version: versionOf(row),
        refusal: null,
        current: null,
        busy: false,
      };
      return;
    }
    // Without the right to apply, one sentence says both: why not now, and where it went.
    const note = commit.now
      ? t("edit.applyForbidden", { permission: applyRight(collection().type) })
      : "";
    void toDraft(row, field, commit.value, note);
  }

  /** The value read again after a 412: the cache's answer, else the object read now. */
  async function currentOf(id: string, refusal: Refusal): Promise<Item | undefined> {
    if (refusal.kind === "conflict" && refusal.current) return refusal.current.data as Item;
    const entry = await queryClient
      .fetchQuery({ ...reads.object(collection().kind, id), staleTime: 0 })
      .catch(() => undefined);
    return (entry as Entry<Item> | undefined)?.data;
  }

  /** Ctrl+Enter, the motive given: the write applied now, rendered by its answer. */
  async function confirm(motive: string, text: string): Promise<boolean> {
    const ask = asking.value;
    if (!ask || ask.busy) return false;
    // Checked again when it is written: still at the present, still with the rights.
    if (why(ask.column) !== null || !canApply.value) {
      cancel();
      say.status(t("edit.notNow"));
      return false;
    }
    const field = ask.field;
    const value = valueOf(field, text);
    const key = keyOf(ask.row, ask.column);
    asking.value = { ...ask, value: text, busy: true };
    // Optimism at the present (D-7): the value shown before the answer, under way.
    shown.set(key, { state: "pending", value, version: ask.version });
    const outcome = await write
      .quick({
        key: keys.object(collection().kind, ask.row),
        patch: patchOf([field], { [field.name]: value }),
        motive,
        ...(ask.version ? { ifMatch: `"${ask.version}"` } : {}),
      })
      .catch(() => ({ kind: "failed", input: {}, problem: null }) as Refusal);
    const words = what(ask.column, ask.name);
    if (outcome.kind === "applied") {
      // Shown until the list is read again, unless it already is.
      if (versionOf(rowById.value.get(ask.row)) === ask.version) {
        shown.set(key, { state: undefined, value, version: ask.version });
      } else shown.delete(key);
      asking.value = null;
      say.applied(words, outcome.changeset);
      return true;
    }
    if (outcome.kind === "proposed") {
      shown.set(key, { state: "review", value, version: ask.version });
      asking.value = null;
      say.proposed(words, outcome.changeset, outcome.reason);
      return true;
    }
    if (outcome.kind === "conflict") {
      shown.delete(key);
      const current = await currentOf(ask.row, outcome);
      // The list reads the object again; the line keeps the input beside its new value.
      void queryClient.invalidateQueries({ queryKey: keys.allOn(collection().path) });
      asking.value = {
        ...ask,
        value: text,
        busy: false,
        refusal: outcome,
        current: current ? textOf(current[field.name]) : null,
        version: versionOf(current) ?? ask.version,
      };
      return false;
    }
    if (outcome.kind === "invalid") {
      shown.set(key, { state: cellErrorOf(outcome, sentence), value, version: ask.version });
    } else shown.delete(key);
    asking.value = { ...ask, value: text, busy: false, refusal: outcome };
    return false;
  }

  /** The line of the motive closed without applying: the cell back to the value read. */
  function cancel(): void {
    const ask = asking.value;
    if (!ask) return;
    const key = keyOf(ask.row, ask.column);
    if (typeof shown.get(key)?.state === "object") shown.delete(key);
    asking.value = null;
  }

  // Off the present (the date pill), the line of the motive closes: nothing is written there.
  watch(present, (now) => {
    if (!now && asking.value) cancel();
  });

  /** The row and the column of a cell, as the grid keys it. */
  const cellOf = (ask: QuickAsk): CellRef => ({ row: ask.row, column: ask.column });

  return {
    shownRows,
    cellState,
    rowState,
    editable,
    validate,
    why,
    canApply,
    present,
    asking,
    onCommit,
    confirm,
    cancel,
    cellOf,
  };
}

export type InlineEdit = ReturnType<typeof useInlineEdit>;
