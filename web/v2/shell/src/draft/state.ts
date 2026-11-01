import {
  drafts,
  type Draft,
  type DraftSubmitOutcome,
  type Simulated,
  type Violation,
} from "@kartenn/sdk";
import type { SplitAction } from "@kartenn/ui";
import { computed, shallowReactive, toValue, type MaybeRefOrGetter } from "vue";
import { lacking } from "../changesets/model.ts";
import { useQuery } from "../data/query.ts";
import { useSession } from "../rights/index.ts";
import { useTime } from "../time/index.ts";

// A draft of the server as its tab and its page read it (REQ-021-6, 11; spec 024 § A; design
// 021 « Cadre » : pied de l'onglet Brouillon): the draft by its identifier, its simulation —
// a query of the cache waited 400 ms after the last change, so that the list and the
// footer, which both observe it, share one —, the violations by operation, and the actions
// of the split button by the rights and the simulation: « Appliquer » (`approval.self` on
// all it touches), « Soumettre en revue », « Enregistrer le brouillon »; an impossible one
// stays, disabled, with its reason or its permission (REQ-021-37).

export type DraftAction = "apply" | "review" | "save";

/**
 * A submission that got no answer (REQ-006-15): its key, kept for the page's life beyond
 * the foot that sent it — the draft may leave the tab meanwhile (the server applied it and
 * deleted it, its event `draft` come first) — so that « Réessayer » replays it, the same
 * URL, the same key, and the server answers what it did.
 */
export interface LostSubmission {
  id: string;
  key: string;
  approvalSelf: boolean;
  title: string;
}
const lost = shallowReactive(new Map<string, LostSubmission>());
export const lostSubmissions = {
  get: (id: string): LostSubmission | undefined => lost.get(id),
  keep: (submission: LostSubmission): void => void lost.set(submission.id, submission),
  drop: (id: string): void => void lost.delete(id),
  all: (): LostSubmission[] => [...lost.values()],
};

/**
 * Sends a submission, or replays the one that got no answer under its key: kept lost
 * again without an answer, forgotten on any answer.
 */
export async function submitDraft(
  id: string,
  approvalSelf: boolean,
  title: string,
): Promise<DraftSubmitOutcome> {
  const replay = lostSubmissions.get(id);
  const key = replay?.key ?? crypto.randomUUID();
  const self = replay?.approvalSelf ?? approvalSelf;
  const outcome = await drafts.submit(id, { approvalSelf: self, idempotencyKey: key });
  if (outcome.kind === "failed" && outcome.problem === null) {
    lostSubmissions.keep({ id, key, approvalSelf: self, title: replay?.title ?? title });
  } else lostSubmissions.drop(id);
  return outcome;
}

export function useDraftQueries(id: MaybeRefOrGetter<string | null>) {
  const session = useSession();
  const time = useTime();
  const query = useQuery(() => ({
    ...drafts.get(toValue(id) ?? "-"),
    enabled: toValue(id) !== null,
  }));
  const draft = computed<Draft | undefined>(() =>
    toValue(id) !== null && query.data.value?.data.id === toValue(id)
      ? query.data.value.data
      : undefined,
  );
  const operations = computed(() => draft.value?.operations ?? []);
  const simulation = useQuery(() => ({
    ...drafts.simulate(draft.value ?? { id: "-", version: 0 }),
    enabled: operations.value.length > 0,
  }));
  const simulated = computed<Simulated | undefined>(() =>
    operations.value.length > 0 ? simulation.data.value : undefined,
  );
  /** The simulation of the version shown is still to come (waited, or asked). */
  const simulating = computed(
    () =>
      operations.value.length > 0 && (simulation.isPending.value || simulation.isFetching.value),
  );

  /** The violations of the simulation by the index of their operation; `-1`: of the result. */
  const violations = computed(() => {
    const by = new Map<number, Violation[]>();
    for (const violation of simulated.value?.violations ?? []) {
      const index = typeof violation.operation === "number" ? violation.operation : -1;
      const list = by.get(index) ?? [];
      list.push(violation);
      by.set(index, list);
    }
    return by;
  });
  const errors = computed(
    () => (simulated.value?.violations ?? []).filter((v) => v.severity === "error").length,
  );

  /** The permission `approval.self` the operations ask for that the user lacks, if one. */
  const applyLacking = computed(() =>
    lacking(session.value.rights, operations.value, "approval.self"),
  );

  /**
   * The actions of the split button, each with why it is impossible now: an empty draft,
   * a simulation still to come or with errors, a permission lacking (« Appliquer »), the
   * past (nothing is submitted out of the present).
   */
  function actions(
    words: (key: string, values?: Record<string, unknown>) => string,
  ): SplitAction[] {
    const blocked = !time.value.present
      ? words("drafts.why.past")
      : operations.value.length === 0
        ? words("drafts.why.empty")
        : simulating.value
          ? words("drafts.why.simulating")
          : simulation.isError.value
            ? words("drafts.why.simulationFailed")
            : errors.value > 0
              ? words("drafts.why.violations", { n: errors.value })
              : undefined;
    const refused = (reason: string | undefined, forbidden?: string) =>
      forbidden && !reason
        ? { disabled: true, forbidden }
        : reason
          ? { disabled: true, reason }
          : {};
    return [
      {
        id: "apply",
        label: words("drafts.action.apply"),
        icon: "circle-check",
        ...refused(blocked, applyLacking.value),
      },
      { id: "review", label: words("drafts.action.review"), icon: "send", ...refused(blocked) },
      { id: "save", label: words("drafts.action.save"), icon: "draft" },
    ];
  }

  /**
   * The action by default: « Appliquer » when possible, else « Soumettre », else save. A
   * simulation still to come does not change it (« Appliquer » waits, maquette E5): the
   * rights and the present choose meanwhile.
   */
  function preferred(list: readonly SplitAction[]): DraftAction {
    if (simulating.value && time.value.present) {
      return applyLacking.value === undefined ? "apply" : "review";
    }
    const possible = (id: DraftAction) => list.some((a) => a.id === id && !a.disabled);
    return possible("apply") ? "apply" : possible("review") ? "review" : "save";
  }

  return {
    query,
    draft,
    operations,
    simulation,
    simulated,
    simulating,
    violations,
    errors,
    applyLacking,
    actions,
    preferred,
  };
}
