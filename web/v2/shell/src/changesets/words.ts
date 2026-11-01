import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useSession } from "../rights/index.ts";
import { useKeyLabel } from "../views/data.ts";

// The words of the changesets and the drafts (REQ-021-10, 11, 19): a state and a kind by
// the dictionary (space `state` of the 018: `changeset.<state>`, `changeset.kind.<kind>`), else the catalogs of the
// shell, else the key; the reader as « vous »; an instant in the language of the moment.

export function useChangesetWords() {
  const { t, te, locale } = useI18n();
  const keyLabel = useKeyLabel();
  const session = useSession();
  const known = (key: string) => (te(key) ? t(key) : null);
  const format = computed(
    () => new Intl.DateTimeFormat(locale.value, { dateStyle: "short", timeStyle: "short" }),
  );
  return {
    state: (state: string) =>
      keyLabel("state", `changeset.${state}`) ?? known(`views.value.state.${state}`),
    kind: (kind: string) =>
      keyLabel("state", `changeset.kind.${kind}`) ?? known(`views.value.kind.${kind}`),
    /** The reader as « vous », anyone else by their identifier. */
    person: (identity: string | null | undefined) =>
      !identity ? "—" : identity === session.value.me?.identity ? t("changesets.you") : identity,
    when: (iso: string | null | undefined) => {
      if (!iso) return "—";
      const date = new Date(iso);
      return Number.isNaN(date.getTime()) ? iso : format.value.format(date);
    },
  };
}
