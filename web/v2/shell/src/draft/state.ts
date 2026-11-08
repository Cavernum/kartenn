import {
  drafts,
  type Draft,
  type DraftPeriod,
  type DraftSubmitOutcome,
  type Simulated,
  type Violation,
} from "@kartenn/sdk";
import { draftPeriod } from "@kartenn/sdk/host";
import type { SplitAction } from "@kartenn/ui";
import { computed, ref, shallowReactive, toValue, type MaybeRefOrGetter } from "vue";
import { lacking } from "../changesets/model.ts";
import { useQuery } from "../data/query.ts";
import { useSession } from "../rights/index.ts";
import { useTime, writesOf, type Time } from "../time/index.ts";

// A draft of the server as its tab and its page read it (REQ-021-6, 11; spec 024 § A; design
// 021 « Cadre » : pied de l'onglet Brouillon): the draft by its identifier, its simulation —
// a query of the cache waited 400 ms after the last change, so that the list and the
// footer, which both observe it, share one —, the violations by operation, and the actions
// of the split button by the rights and the simulation: « Appliquer » (`approval.self` on
// all it touches), « Soumettre en revue », « Enregistrer le brouillon »; an impossible one
// stays, disabled, with its reason or its permission (REQ-021-37). Under `as_of` the
// draft is read only; under `valid_at` alone it is submitted, its period from that date
// (REQ-021-5).

export type DraftAction = "apply" | "review" | "save";

/**
 * The period of the changeset a draft submits (its dates of effect, `valid_from` and
 * `valid_to`), kept with the draft by the SDK for the tab, a reload included (the server's
 * draft has none): the date of its first addition under `valid_at` alone, or the foot's
 * choice, so that its simulation and its submission read the same, back at the present
 * too. Neither, `valid_from` follows the date read (`valid_at`), else now; `valid_to` is
 * unbounded.
 */
const periodsChanged = ref(0);
draftPeriod.on(() => (periodsChanged.value += 1));
export const draftPeriods = {
  get: (id: string, time: Time): DraftPeriod => {
    void periodsChanged.value;
    return draftPeriod.get(id) ?? (time.validAt ? { valid_from: time.validAt } : {});
  },
  set: (id: string, period: DraftPeriod): void => draftPeriod.set(id, period),
  drop: (id: string): void => draftPeriod.set(id, null),
};

/** The period as sent: its dates set, nothing for the defaults of the server. */
export function periodBody(period: DraftPeriod): DraftPeriod {
  return {
    ...(period.valid_from ? { valid_from: period.valid_from } : {}),
    ...(period.valid_to ? { valid_to: period.valid_to } : {}),
  };
}

/**
 * A submission that got no answer (REQ-006-15): its key, kept for the page's life beyond
 * the foot that sent it — the draft may leave the tab meanwhile (the server applied it and
 * deleted it, its event `draft` come first) — so that « Réessayer » replays it, the same
 * URL, the same key, the same period, and the server answers what it did.
 */
export interface LostSubmission {
  id: string;
  key: string;
  approvalSelf: boolean;
  title: string;
  period: DraftPeriod;
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
  period: DraftPeriod = {},
): Promise<DraftSubmitOutcome> {
  const replay = lostSubmissions.get(id);
  const key = replay?.key ?? crypto.randomUUID();
  const self = replay?.approvalSelf ?? approvalSelf;
  const sent = replay?.period ?? periodBody(period);
  const outcome = await drafts.submit(id, {
    approvalSelf: self,
    idempotencyKey: key,
    period: sent,
  });
  if (outcome.kind === "failed" && outcome.problem === null) {
    lostSubmissions.keep({
      id,
      key,
      approvalSelf: self,
      title: replay?.title ?? title,
      period: sent,
    });
  } else {
    // Accepted, the SDK forgets the draft and its period with it.
    lostSubmissions.drop(id);
  }
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
  /** The draft may be written: not under `as_of` (REQ-021-5). */
  const writable = computed(() => writesOf(time.value).draft);
  /**
   * A submission of the draft awaits its retry (no answer): its period is the one sent,
   * fixed until an answer (REQ-006-15).
   */
  const awaiting = computed(() => {
    const at = toValue(id);
    return at === null ? undefined : lostSubmissions.get(at);
  });
  /** The period of its changeset: as sent and awaiting, else as kept (or its default). */
  const period = computed<DraftPeriod>(() => {
    const at = toValue(id);
    if (at === null) return {};
    return awaiting.value?.period ?? draftPeriods.get(at, time.value);
  });
  const simulation = useQuery(() => ({
    ...drafts.simulate(draft.value ?? { id: "-", version: 0 }, periodBody(period.value)),
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
   * a simulation still to come or with errors, a permission lacking (« Appliquer »), a
   * state known at a past date (`as_of`: nothing written, the draft not saved either).
   */
  function actions(
    words: (key: string, values?: Record<string, unknown>) => string,
  ): SplitAction[] {
    const past = writable.value ? undefined : words("drafts.why.past");
    const blocked = past
      ? past
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
      { id: "save", label: words("drafts.action.save"), icon: "draft", ...refused(past) },
    ];
  }

  /**
   * The action by default: « Appliquer » when possible, else « Soumettre », else save. A
   * simulation still to come does not change it (« Appliquer » waits, maquette E5): the
   * rights and the time choose meanwhile.
   */
  function preferred(list: readonly SplitAction[]): DraftAction {
    if (simulating.value && writable.value) {
      return applyLacking.value === undefined ? "apply" : "review";
    }
    const possible = (id: DraftAction) => list.some((a) => a.id === id && !a.disabled);
    return possible("apply") ? "apply" : possible("review") ? "review" : "save";
  }

  return {
    query,
    draft,
    operations,
    writable,
    awaiting,
    period,
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
