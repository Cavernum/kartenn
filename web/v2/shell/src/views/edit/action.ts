import { ApiProblem, type Submission, type WriteOutcome } from "@kartenn/sdk";
import { injectToasts, type SplitAction } from "@kartenn/ui";
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useSession } from "../../rights/index.ts";
import { useTime } from "../../time/index.ts";
import { CONFLICT_RULE } from "./close.ts";
import { lackingRight } from "./fields.ts";
import type { Refusal } from "./inline.ts";
import { closeEdit } from "./panel.ts";
import { useSay } from "./say.ts";

// The write of a panel of the right side (task 021-12b; REQ-021-36, 37; REQ-006-15): the
// split button's action — to the draft by default, « Appliquer » (`?approval=self`) with the
// approval of each edit permission —, the motive prefilled until the user writes one, the
// idempotency key of a write applied at once, and its outcome said: « ajouté au brouillon »
// by the status and a toast, applied or held by a toast, the panel then closed; a refusal
// kept in the panel. Shared by the edit panel and the close panel.

/** The outcome of a write of a panel: to the draft, applied at once, or rejected outright. */
export type PanelOutcome = WriteOutcome<unknown> | { kind: "saved" } | Refusal;

export interface WriteActionOptions {
  /** The edit permissions the write needs; to apply, the approval of each too. */
  rights: () => readonly string[];
  /** The motive prefilled, until the user writes one of their own. */
  motive: () => string;
  /** The words of the panel: its two actions, and what is said off the present. */
  words: { draft: string; apply: string; past: string };
  /** A refusal of the server, kept in the panel: what the panel does with it besides. */
  refused?: (refusal: Refusal) => void;
}

/**
 * Whether the object changed since its read: a 412 of a patch, or a changeset refused for an
 * operation written on a version no longer current (`changeset.operation.conflict`).
 */
export const isConflict = (refusal: Refusal | null): boolean =>
  refusal?.kind === "conflict" ||
  (refusal?.problem?.violations.some((v) => v.rule === CONFLICT_RULE) ?? false);

/** A write rejected outright: the problem kept, its code and `Request-Id` said with it. */
export const failed = (error: unknown): Refusal => ({
  kind: "failed",
  input: {},
  problem: error instanceof ApiProblem ? error : null,
});

export function useWriteAction(options: WriteActionOptions) {
  const { t } = useI18n();
  const session = useSession();
  const time = useTime();
  const toasts = injectToasts();
  const say = useSay((text) => toasts?.show({ tone: "ok", title: text }));

  /** The action of the split button: to the draft by default. */
  const action = ref("draft");
  /** The permission an action lacks, named: an edit permission, or to apply its approval. */
  const lacking = (id: string) =>
    lackingRight((p) => session.value.rights.has(p), options.rights(), id === "apply");
  /**
   * Under a validity date alone (REQ-021-5, 007/W9): the dated draft only, nothing applied at
   * once — what a write applies takes effect now, not at the date read.
   */
  const dated = computed(() => time.value.asOf === null && time.value.validAt !== null);
  /** Why `id` is impossible at the date read, if it is: « Appliquer » under `valid_at`. */
  const datedReason = (id: string) => (id === "apply" && dated.value ? t("edit.why.dated") : null);
  const canApply = computed(() => lacking("apply") === null && !dated.value);
  const actions = computed<SplitAction[]>(() =>
    [
      { id: "draft", label: t(options.words.draft), icon: "draft" as const },
      { id: "apply", label: t(options.words.apply), icon: "circle-check" as const },
    ].map((a) => {
      const permission = lacking(a.id);
      if (permission) return { ...a, forbidden: permission };
      const reason = datedReason(a.id);
      return reason ? { ...a, disabled: true, reason } : a;
    }),
  );
  /** « Appliquer » chosen and permitted: the motive is asked. */
  const applying = computed(() => action.value === "apply" && canApply.value);

  const motive = ref("");
  let motiveTyped = false;
  watch(
    options.motive,
    (text) => {
      if (!motiveTyped) motive.value = text;
    },
    { immediate: true },
  );
  function setMotive(text: string): void {
    motive.value = text;
    motiveTyped = true;
  }

  /**
   * The key of `submission`, the very body a write applied at once sends (REQ-006-15): the
   * same body again — a try again after a lost answer — replays under the same key; another
   * one (a value typed, a cascade, a new motive, the version read again after a conflict)
   * takes a new key. Compared when the key is taken, so that no change is missed.
   */
  let keyed = { of: "", key: "" };
  function idempotencyKey(submission: Submission): string {
    const of = JSON.stringify(submission);
    if (of !== keyed.of) keyed = { of, key: crypto.randomUUID() };
    return keyed.key;
  }

  const busy = ref(false);
  /** The sentence of the status of the panel. */
  const notice = ref<string | null>(null);
  /** The sentence the page shows, once rendered. */
  let shown: string | null = null;
  watch(notice, (text) => (shown = text), { flush: "post" });
  const refusal = ref<Refusal | null>(null);

  /**
   * Whether `id` may run now: not while a write is in flight; a refused action — run at the
   * keyboard too — says its permission by the status, never silence; nothing is written
   * under `as_of`, the draft included; under `valid_at` alone, the dated draft only. The
   * sentence and the refusal before are cleared.
   */
  function begin(id: string): boolean {
    if (busy.value) return false;
    const missing = lacking(id);
    if (missing !== null) {
      tell(t("edit.unsent", { permission: missing }));
      return false;
    }
    const off = time.value.asOf !== null ? t(options.words.past) : datedReason(id);
    if (off !== null) {
      refusal.value = null;
      tell(off);
      return false;
    }
    notice.value = null;
    refusal.value = null;
    return true;
  }
  /**
   * A sentence of the status; the same as the one shown, emptied first so that it is said
   * again (cleared and set again in one tick, the page would not change).
   */
  function tell(text: string): void {
    if (shown !== text) {
      notice.value = text;
      return;
    }
    notice.value = null;
    void nextTick(() => (notice.value = text));
  }

  /** The outcome said, the panel closed once the write went through; a refusal kept. */
  function settle(outcome: PanelOutcome, what: string): void {
    busy.value = false;
    if (outcome.kind === "saved") {
      say.status(t("edit.drafted", { what }));
      closeEdit();
    } else if (outcome.kind === "applied") {
      say.applied(what, outcome.changeset);
      closeEdit();
    } else if (outcome.kind === "proposed") {
      say.proposed(what, outcome.changeset, outcome.reason);
      closeEdit();
    } else {
      refusal.value = outcome;
      options.refused?.(outcome);
    }
  }

  /** Writes `what` by `send`, the button busy meanwhile; a rejection settles as a refusal. */
  async function write(what: string, send: () => Promise<PanelOutcome>): Promise<void> {
    busy.value = true;
    let outcome: PanelOutcome;
    try {
      outcome = await send();
    } catch (error) {
      outcome = failed(error);
    }
    settle(outcome, what);
  }

  return {
    action,
    actions,
    applying,
    motive,
    setMotive,
    idempotencyKey,
    busy,
    notice,
    refusal,
    tell,
    begin,
    settle,
    write,
  };
}
