import { ref, shallowRef } from "vue";
import { send, type Problem } from "../data/api.ts";
import { outcomeOf, type Outcome, type Submitted } from "./admin.ts";

/**
 * A write of the administration and what became of it: `run` sends it, keeps its outcome
 * or its problem, and answers whether it went through.
 */
export function useAdminWrite() {
  const outcome = shallowRef<Outcome | null>(null);
  const problem = shallowRef<Problem | null>(null);
  const busy = ref(false);

  async function run(
    method: string,
    url: string,
    body?: unknown,
    headers: Record<string, string> = {},
  ): Promise<Outcome | null> {
    busy.value = true;
    // Spec 024 (REQ-024-13): applied at once by who holds apply_own, pending otherwise.
    const applied = /[?&]apply=/.test(url)
      ? url
      : `${url}${url.includes("?") ? "&" : "?"}apply=own`;
    const answer = await send<Submitted>(method, applied, body, headers);
    busy.value = false;
    if (!answer.ok) {
      outcome.value = null;
      problem.value = answer.problem;
      return null;
    }
    problem.value = null;
    outcome.value = outcomeOf(answer);
    return outcome.value;
  }

  return { outcome, problem, busy, run };
}
