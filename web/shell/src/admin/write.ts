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
    const answer = await send<Submitted>(method, url, body, headers);
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
