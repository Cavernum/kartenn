import { computed, type ComputedRef } from "vue";
import { useI18n } from "vue-i18n";
import { useTime, useWrites, type WriteRefusal } from "../time/index.ts";
import { whyWords } from "../views/edit/inline.ts";
import type { Refusal } from "./model.ts";

// The gate of writes as the administration reads it (REQ-021-29, Q-021-12; REQ-021-5): it
// writes at the present only, its drafts never dated (`valid_at` alone refused too, which
// leaves only the draft to a write elsewhere), and nothing under 1024 px wide. The gate
// only refuses: the rights and the rules of the scope (025) are judged after it, by the
// models of the screens, and nothing it lets through is offered for that.

export interface AdminGate {
  /** Why nothing of the administration is written now; `null` when it may be. */
  refused: ComputedRef<WriteRefusal | null>;
  /** Its sentence (`edit.why.past`, `edit.why.narrow`); `undefined` when it may be. */
  why: ComputedRef<string | undefined>;
}

/** The sentence of a refusal of the administration, as a key refused says it (`e`). */
export function refusalSentence(
  refusal: Refusal | undefined,
  t: (key: string) => string,
): string | undefined {
  if (!refusal) return undefined;
  return "forbidden" in refusal
    ? whyWords({ kind: "forbidden", permission: refusal.forbidden }, t)
    : refusal.reason;
}

export function useAdminGate(): AdminGate {
  const { t } = useI18n();
  const time = useTime();
  const writes = useWrites();
  const refused = computed<WriteRefusal | null>(() =>
    time.value.present ? writes.value.refused : "past",
  );
  const why = computed(() => (refused.value ? t(`edit.why.${refused.value}`) : undefined));
  return { refused, why };
}
