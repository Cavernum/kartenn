import type { PendingReason } from "@kartenn/sdk";
import { injectToasts } from "@kartenn/ui";
import { nextTick, ref, watch, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useSession } from "../../rights/index.ts";
import { useTime } from "../../time/index.ts";
import type { InlineSay } from "./inline.ts";

// The outcome of a write that went through, said by a toast of the frame (REQ-021-16): 200
// or 201 « appliqué » with a link to its changeset; 202 « proposé, en revue » — never
// « appliqué » — with the reason a write that asked to apply stays held, and a link to the
// proposal; each said once, by its toast (and by the cell held, `heldWords`). A sentence
// of the view (« ajouté au brouillon ») goes to `status`. A refusal is said where it was
// asked (the cell, the line of the motive, the panel), never by a toast.

/** The reasons a write that asked to apply is held (024, REQ-024-13), as the catalogs say them. */
const REASONS: Readonly<Record<string, string>> = {
  "changeset.approval.self.missing": "edit.reason.missing",
  "changeset.approval.self": "edit.reason.held",
};

/** Why a write that asked to apply is held: the catalog's sentence, else the server's. */
export function heldWords(
  t: (key: string) => string,
  reason: PendingReason | null,
): string | undefined {
  if (!reason) return undefined;
  const words = REASONS[reason.code];
  return words ? t(words) : (reason.message ?? undefined);
}

/** The sentence of a view's polite status region (`role="status"`), and how to set it. */
export interface StatusLine {
  said: Readonly<Ref<string>>;
  /** Says `text`: emptied first, so that the same sentence twice is said twice. */
  status: (text: string) => void;
  /** Empties it: what it said no longer holds (another object). */
  clear: () => void;
}

/**
 * The status of a view, emptied once what it said may no longer hold: another period
 * (a refusal « revenez au présent » is not left at the present), other rights; the view
 * clears it for another object (`clear`). A sentence still to come is dropped too.
 */
export function useStatus(): StatusLine {
  const said = ref("");
  const time = useTime();
  const session = useSession();
  let turn = 0;
  const clear = () => {
    turn += 1;
    said.value = "";
  };
  watch(() => [time.value.validAt, time.value.asOf, session.value.rights] as const, clear);
  return {
    said,
    status(text) {
      clear();
      const mine = turn;
      void nextTick(() => {
        if (mine === turn) said.value = text;
      });
    },
    clear,
  };
}

export function useSay(status: (text: string) => void = () => {}): InlineSay {
  const { t } = useI18n();
  const router = useRouter();
  const toasts = injectToasts();
  const link = (changeset: string | null, label: string) =>
    changeset
      ? { label, run: () => void router.push(`/changesets/${encodeURIComponent(changeset)}`) }
      : undefined;
  return {
    applied(what, changeset) {
      toasts?.show({
        tone: "ok",
        title: t("edit.applied", { what }),
        action: link(changeset, t("edit.seeChangeset")),
      });
    },
    proposed(what, changeset, reason) {
      toasts?.show({
        tone: "info",
        title: t("edit.proposed", { what }),
        description: heldWords(t, reason),
        code: reason?.code,
        action: link(changeset, t("edit.seeProposal")),
      });
    },
    status,
  };
}
