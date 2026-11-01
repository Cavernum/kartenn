import { injectToasts } from "@kartenn/ui";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import type { InlineSay } from "./inline.ts";

// The outcome of a write that went through, said by a toast of the frame (REQ-021-16): 200
// or 201 « appliqué » with a link to its changeset; 202 « proposé, en revue » — never
// « appliqué » — with the reason a write that asked to apply stays held, and a link to the
// proposal; each said once, by its toast. A sentence of the view (« ajouté au brouillon »)
// goes to `status`. A refusal is said where it was asked (the cell, the line of the
// motive, the panel), never by a toast.

/** The reasons a write that asked to apply is held (024, REQ-024-13), as the catalogs say them. */
const REASONS: Readonly<Record<string, string>> = {
  "changeset.approval.self.missing": "edit.reason.missing",
  "changeset.approval.self": "edit.reason.held",
};

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
      const words = reason ? REASONS[reason.code] : undefined;
      toasts?.show({
        tone: "info",
        title: t("edit.proposed", { what }),
        description: words ? t(words) : (reason?.message ?? undefined),
        code: reason?.code,
        action: link(changeset, t("edit.seeProposal")),
      });
    },
    status,
  };
}
