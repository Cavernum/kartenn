// The Vue adapter of the data layer (ADR 0037, « Résolution de D-1 »): our own small
// composables over the observers of `@tanstack/query-core`, on the one `QueryClient` of
// `@kartenn/sdk`, which stays without a framework (REQ-021-25). Screens read with
// `useQuery(reads.object(…))`; results stay shallow (lesson of the prototype: deep reactive
// wrappers of large reads cost and lose identity). Each composable observes while its
// effect scope lives and stops observing when the scope is disposed.
// Placed in the shell until task 8 moves it to @kartenn/ui with the shared modules.

import { context, events, label, labelDictionary, queryClient } from "@kartenn/sdk";
import type { StreamState } from "@kartenn/sdk";
import {
  InfiniteQueryObserver,
  MutationObserver,
  QueryObserver,
  type DefaultError,
  type DefaultedInfiniteQueryObserverOptions,
  type InfiniteData,
  type InfiniteQueryObserverOptions,
  type InfiniteQueryObserverResult,
  type MutateOptions,
  type MutationObserverOptions,
  type MutationObserverResult,
  type QueryClient,
  type QueryKey,
  type QueryObserverOptions,
  type QueryObserverResult,
} from "@tanstack/query-core";
import {
  hasInjectionContext,
  inject,
  onScopeDispose,
  readonly,
  shallowReactive,
  shallowReadonly,
  shallowRef,
  toRefs,
  toValue,
  watch,
  type App,
  type InjectionKey,
  type MaybeRefOrGetter,
  type Ref,
  type ToRefs,
} from "vue";

const CLIENT: InjectionKey<QueryClient> = Symbol("kartenn.queryClient");

/** Plugs the cache of the SDK into an application of the shell or of a first-party plugin. */
export function installData(app: App): void {
  app.provide(CLIENT, queryClient);
}

/** The `QueryClient` of the application, the SDK's one when none is provided. */
export function useQueryClient(): QueryClient {
  return (hasInjectionContext() ? inject(CLIENT, null) : null) ?? queryClient;
}

/**
 * The fields of an observer result as read-only refs (shallow: the data keeps its
 * identity), set again on each notification of the observer.
 */
function tracked<R extends object>(initial: R): { refs: ToRefs<Readonly<R>>; set: (r: R) => void } {
  const state = shallowReactive({ ...initial }) as R;
  return {
    refs: toRefs(shallowReadonly(state)) as ToRefs<Readonly<R>>,
    set: (result) => Object.assign(state, result),
  };
}

/** Methods of a result, rebound to the observer rather than exposed as refs. */
type Methods = "refetch" | "fetchNextPage" | "fetchPreviousPage" | "promise";

export type UseQueryResult<TData, TError = DefaultError> = ToRefs<
  Readonly<Omit<QueryObserverResult<TData, TError>, Methods>>
> & {
  refetch: QueryObserverResult<TData, TError>["refetch"];
};

/** Observes the query of `options` (reactive) while the scope lives. */
export function useQuery<
  TQueryFnData = unknown,
  TError = DefaultError,
  TData = TQueryFnData,
  TQueryKey extends QueryKey = QueryKey,
>(
  options: MaybeRefOrGetter<
    QueryObserverOptions<TQueryFnData, TError, TData, TQueryFnData, TQueryKey>
  >,
  client: QueryClient = useQueryClient(),
): UseQueryResult<TData, TError> {
  const defaulted = () => client.defaultQueryOptions(toValue(options));
  const observer = new QueryObserver<TQueryFnData, TError, TData, TQueryFnData, TQueryKey>(
    client,
    defaulted(),
  );
  const { refs, set } = tracked(observer.getOptimisticResult(defaulted()));
  const unsubscribe = observer.subscribe(set);
  watch(
    defaulted,
    (next) => {
      observer.setOptions(next);
      set(observer.getOptimisticResult(next));
    },
    { flush: "sync" },
  );
  onScopeDispose(unsubscribe);
  return { ...refs, refetch: (opts) => observer.refetch(opts) } as UseQueryResult<TData, TError>;
}

export type UseInfiniteQueryResult<TData, TError = DefaultError> = ToRefs<
  Readonly<Omit<InfiniteQueryObserverResult<TData, TError>, Methods>>
> &
  Pick<
    InfiniteQueryObserverResult<TData, TError>,
    "refetch" | "fetchNextPage" | "fetchPreviousPage"
  >;

/** Observes the paged query of `options` (reactive) while the scope lives. */
export function useInfiniteQuery<
  TQueryFnData = unknown,
  TError = DefaultError,
  TData = InfiniteData<TQueryFnData>,
  TQueryKey extends QueryKey = QueryKey,
  TPageParam = unknown,
>(
  options: MaybeRefOrGetter<
    InfiniteQueryObserverOptions<TQueryFnData, TError, TData, TQueryKey, TPageParam>
  >,
  client: QueryClient = useQueryClient(),
): UseInfiniteQueryResult<TData, TError> {
  const defaulted = () =>
    client.defaultQueryOptions(toValue(options)) as DefaultedInfiniteQueryObserverOptions<
      TQueryFnData,
      TError,
      TData,
      TQueryKey,
      TPageParam
    >;
  const observer = new InfiniteQueryObserver(client, defaulted());
  const { refs, set } = tracked(observer.getOptimisticResult(defaulted()));
  const unsubscribe = observer.subscribe(set);
  watch(
    defaulted,
    (next) => {
      observer.setOptions(next);
      set(observer.getOptimisticResult(next));
    },
    { flush: "sync" },
  );
  onScopeDispose(unsubscribe);
  return {
    ...refs,
    refetch: (opts) => observer.refetch(opts),
    fetchNextPage: (opts) => observer.fetchNextPage(opts),
    fetchPreviousPage: (opts) => observer.fetchPreviousPage(opts),
  } as UseInfiniteQueryResult<TData, TError>;
}

export type UseMutationResult<TData, TError, TVariables, TContext> = ToRefs<
  Readonly<Omit<MutationObserverResult<TData, TError, TVariables, TContext>, "mutate" | "reset">>
> & {
  /** Runs the mutation; its error is in `error`, never thrown. */
  mutate: (
    variables: TVariables,
    options?: MutateOptions<TData, TError, TVariables, TContext>,
  ) => void;
  /** Runs the mutation; the promise rejects with its error. */
  mutateAsync: (
    variables: TVariables,
    options?: MutateOptions<TData, TError, TVariables, TContext>,
  ) => Promise<TData>;
  reset: () => void;
};

/** A mutation of `options` (reactive), observed while the scope lives. */
export function useMutation<
  TData = unknown,
  TError = DefaultError,
  TVariables = void,
  TContext = unknown,
>(
  options: MaybeRefOrGetter<MutationObserverOptions<TData, TError, TVariables, TContext>>,
  client: QueryClient = useQueryClient(),
): UseMutationResult<TData, TError, TVariables, TContext> {
  const observer = new MutationObserver(client, client.defaultMutationOptions(toValue(options)));
  const { refs, set } = tracked(observer.getCurrentResult());
  const unsubscribe = observer.subscribe(set);
  watch(
    () => client.defaultMutationOptions(toValue(options)),
    (next) => observer.setOptions(next),
    { flush: "sync" },
  );
  onScopeDispose(unsubscribe);
  return {
    ...refs,
    mutate: (variables, opts) => void observer.mutate(variables, opts).catch(() => {}),
    mutateAsync: (variables, opts) => observer.mutate(variables, opts),
    reset: () => observer.reset(),
  } as UseMutationResult<TData, TError, TVariables, TContext>;
}

/** The value of a store of the SDK as a ref, followed while the scope lives. */
function follow<T>(
  read: () => T,
  subscribe: (listener: () => void) => () => void,
): Readonly<Ref<T>> {
  const value = shallowRef(read());
  const stop = subscribe(() => {
    value.value = read();
  });
  onScopeDispose(stop);
  return readonly(value) as Readonly<Ref<T>>;
}

/** The state of the event stream: `open`, `connecting`, `closed` (a banner without it). */
export function useEventsState(): Readonly<Ref<StreamState>> {
  return follow(
    () => events.state(),
    (listener) => events.onState(listener),
  );
}

/** The language of the context. */
export function useLocale(): Readonly<Ref<string>> {
  return follow(
    () => context.get().locale,
    (listener) => context.subscribe(listener),
  );
}

/**
 * The label function of the language of the context, reactive: what renders it renders
 * again when the dictionary arrives or another revision is read (REQ-021-19).
 */
export function useLabel(): (space: string, key: string) => string {
  const locale = useLocale();
  const dictionary = useQuery(() => labelDictionary(locale.value));
  return (space, key) => {
    void dictionary.data.value;
    return label(space, key, locale.value);
  };
}
