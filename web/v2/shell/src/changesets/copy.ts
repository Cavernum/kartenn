import { copyText, injectToasts } from "@kartenn/ui";
import { useI18n } from "vue-i18n";

// « Copier l'identifiant » of a changeset or a draft (4.1.3): the outcome said by a toast;
// refused by the browser, the identifier given in it, copyable by hand.

export function useCopyId(): (id: string) => Promise<void> {
  const { t } = useI18n();
  const toasts = injectToasts();
  return async (id) => {
    const outcome = await copyText(id);
    toasts?.show(
      outcome === "copied"
        ? { tone: "ok", title: t("changesets.copied") }
        : { tone: "warn", title: t("changesets.copyFailed"), code: id },
    );
  };
}
