import { ApiProblem } from "@kartenn/sdk";
import type { ProblemFacts } from "@kartenn/ui";
import { useI18n } from "vue-i18n";
import { useLabel } from "../data/query.ts";

// The one way the shell says a problem of the API (REQ-021-19, 20; design 021 « Filtres,
// libellés, codes », « Erreurs et cas limites »): in plain words, the translation of its
// `type` (space `problem` of the dictionary of the 018, then the catalogs
// `labels.problem.<slug>`, then the answer's own words), and beside it the codes as the API
// serves them, copyable: the `type` (`urn:kartenn:problem:<slug>`), the rule of each
// violation, the `Request-Id`. `ProblemState` (an error state), `RefusalAlert` (an inline
// alert, a write refused) and the login read it.

const PREFIX = "urn:kartenn:problem:";

/** A line of an inline alert: what went wrong and its code. */
export interface ProblemLine {
  message: string;
  code?: string;
}

/** What an error state shows of an error: the problem of the API as it is, else nothing. */
export function problemOf(error: unknown): ProblemFacts | undefined {
  return error instanceof ApiProblem ? error : undefined;
}

/** The slug of the type of a problem (`urn:kartenn:problem:<slug>`), `null` otherwise. */
export function problemSlug(error: unknown): string | null {
  return error instanceof ApiProblem && error.type.startsWith(PREFIX) ? error.code : null;
}

/** The code of a problem as served, `undefined` for `about:blank` (RFC 9457 §4.2.1). */
export function problemCode(error: unknown): string | undefined {
  return error instanceof ApiProblem && error.type !== "about:blank" ? error.type : undefined;
}

/** A sentence ends with its stop: the labels of the dictionary come without one. */
const ended = (text: string) => (/[.!?…]$/.test(text) ? text : `${text}.`);

/**
 * The words of the problems. `dictionary: false` where the dictionary cannot be read (the
 * login, before any session): the catalogs alone.
 */
export function useProblems(options: { dictionary?: boolean } = {}) {
  const { t, te } = useI18n();
  const label =
    options.dictionary === false
      ? (space: string, key: string) =>
          te(`labels.${space}.${key}`) ? t(`labels.${space}.${key}`) : key
      : useLabel();

  /** What happened, in plain words; the server unreached when there is no answer. */
  function sentence(error: unknown): string {
    if (!(error instanceof ApiProblem)) return t("problem.network");
    const slug = problemSlug(error);
    const said = slug === null ? null : label("problem", slug);
    if (said !== null && said !== slug) return ended(said);
    return ended(error.detail || error.title || t("problem.failed"));
  }

  /**
   * The lines of an inline alert: `lead` (the violations, the fields changed), then the
   * problem in plain words with its `type`, then the `Request-Id`, each code copyable.
   * `said`: the title already says the problem — its `type` only, under « Code du
   * problème », and nothing for a network failure.
   */
  function lines(
    error: unknown,
    lead: readonly ProblemLine[] = [],
    { said = false }: { said?: boolean } = {},
  ): ProblemLine[] {
    const out = [...lead];
    const code = problemCode(error);
    if (code) out.push({ message: said ? t("problem.code") : sentence(error), code });
    else if (!said) out.push({ message: sentence(error) });
    const id = error instanceof ApiProblem ? error.requestId : null;
    if (id) out.push({ message: t("problem.requestId"), code: id });
    return out;
  }

  return { sentence, lines };
}
