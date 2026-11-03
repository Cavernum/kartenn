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
  // Absent (`null`, `undefined`): `null`, said by the place that shows it (`AttributeList`,
  // `NoValue`); a value the contract requires stays a text, typed so (no blank cell).
  /** The reader as « vous », anyone else by their identifier. */
  function person(identity: string): string;
  function person(identity: string | null | undefined): string | null;
  function person(identity: string | null | undefined): string | null {
    if (identity === null || identity === undefined) return null;
    return identity === session.value.me?.identity ? t("changesets.you") : identity;
  }
  /** An instant in the language of the moment; unreadable, as served. */
  function when(iso: string): string;
  function when(iso: string | null | undefined): string | null;
  function when(iso: string | null | undefined): string | null {
    if (iso === null || iso === undefined) return null;
    const date = new Date(iso);
    return Number.isNaN(date.getTime()) ? iso : format.value.format(date);
  }
  return {
    state: (state: string) =>
      keyLabel("state", `changeset.${state}`) ?? known(`views.value.state.${state}`),
    kind: (kind: string) =>
      keyLabel("state", `changeset.kind.${kind}`) ?? known(`views.value.kind.${kind}`),
    person,
    when,
  };
}
