import {
  QueriesObserver,
  type DefaultError,
  type QueryClient,
  type QueryObserverOptions,
  type QueryObserverResult,
} from "@tanstack/query-core";
import { onScopeDispose, readonly, shallowRef, watch, type Ref } from "vue";
import { useQueryClient } from "./query.ts";

// The composable of several queries at once (`query.ts`, ADR 0037), apart: only the views
// read with it, never the frame, and its observer stays out of the initial shell (task
// 22n, budget « Shell initial »).

/**
 * Observes the queries of `options` (reactive: their number changes with it) while the
 * scope lives: their results in the same order (the cells of a column read row by row).
 */
export function useQueries<O extends object>(
  options: () => readonly O[],
  client: QueryClient = useQueryClient(),
): Readonly<Ref<readonly QueryObserverResult<unknown, DefaultError>[]>> {
  // Options of the SDK's reads, typed by their own keys: the observer takes any key.
  const defaulted = () =>
    options().map((one) => client.defaultQueryOptions(one as unknown as QueryObserverOptions));
  const observer = new QueriesObserver(client, defaulted());
  const results = shallowRef<readonly QueryObserverResult<unknown, DefaultError>[]>(
    observer.getCurrentResult(),
  );
  const unsubscribe = observer.subscribe((now) => (results.value = now));
  watch(
    defaulted,
    (next) => {
      observer.setQueries(next);
      results.value = observer.getCurrentResult();
    },
    { flush: "sync" },
  );
  onScopeDispose(unsubscribe);
  return readonly(results) as Readonly<Ref<readonly QueryObserverResult<unknown, DefaultError>[]>>;
}
