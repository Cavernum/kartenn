// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it, vi } from "vitest";
import { computed, createApp, effectScope, nextTick, ref } from "vue";
import {
  installData,
  useEventsState,
  useInfiniteQuery,
  useLabel,
  useMutation,
  useQuery,
  useQueryClient,
} from "../src/data/query.ts";

// The Vue adapter of the data layer (ADR 0037, « Résolution de D-1 »; spec 021, P1).

afterEach(() => {
  setFetch(null);
  resetContext();
  queryClient.clear();
});

/** Runs composables as a component of an application with the adapter would. */
function inApp<T>(run: () => T): { value: T; stop: () => void } {
  const app = createApp({});
  installData(app);
  const scope = effectScope();
  const value = app.runWithContext(() => scope.run(run))!;
  return { value, stop: () => scope.stop() };
}

describe("Vue adapter of @kartenn/sdk", () => {
  it("gives the composables the one QueryClient of the SDK", () => {
    const { value } = inApp(() => useQueryClient());
    expect(value).toBe(queryClient);
  });

  it("renders a label again once the dictionary of the language is read", async () => {
    const fetch = vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            lang: "fr",
            default_lang: "fr",
            revision: "r1",
            spaces: { kind: { "core.prefix": "Préfixe" } },
          }),
          { headers: { "Content-Type": "application/json", ETag: '"r1.fr"' } },
        ),
    );
    setFetch(fetch as unknown as typeof globalThis.fetch);
    setContext({ locale: "fr" });
    const { value: shown, stop } = inApp(() => {
      const t = useLabel();
      return computed(() => t("kind", "core.prefix"));
    });
    expect(shown.value).toBe("core.prefix");
    await vi.waitFor(() => expect(shown.value).toBe("Préfixe"));
    // The adapter observes the query while the scope lives, and only then.
    expect(
      queryClient
        .getQueryCache()
        .find({ queryKey: ["v1", "labels", "fr"] })
        ?.getObserversCount(),
    ).toBe(1);
    expect((fetch.mock.calls as unknown[][])[0]?.[0]).toBe("/v1/labels?lang=fr");
    stop();
    expect(
      queryClient
        .getQueryCache()
        .find({ queryKey: ["v1", "labels", "fr"] })
        ?.getObserversCount(),
    ).toBe(0);
  });

  it("follows the state of the event stream", () => {
    const { value: state, stop } = inApp(() => useEventsState());
    expect(state.value).toBe("closed");
    stop();
  });
});

describe("composables over the observers of query-core", () => {
  const observers = (key: readonly unknown[]) =>
    queryClient.getQueryCache().find({ queryKey: key })?.getObserversCount() ?? 0;

  it("useQuery fetches, follows the cache and stops observing with its scope", async () => {
    const { value: query, stop } = inApp(() =>
      useQuery({ queryKey: ["test", "a"], queryFn: async () => "a1", staleTime: Infinity }),
    );
    expect(query.isPending.value).toBe(true);
    await vi.waitFor(() => expect(query.data.value).toBe("a1"));
    expect(query.status.value).toBe("success");
    expect(observers(["test", "a"])).toBe(1);
    queryClient.setQueryData(["test", "a"], "a2");
    await vi.waitFor(() => expect(query.data.value).toBe("a2"));
    stop();
    expect(observers(["test", "a"])).toBe(0);
    queryClient.setQueryData(["test", "a"], "a3");
    await new Promise((resolve) => setTimeout(resolve, 10));
    expect(query.data.value).toBe("a2");
  });

  it("useQuery follows reactive options to another key", async () => {
    const id = ref("x");
    const { value: query, stop } = inApp(() =>
      useQuery(() => ({
        queryKey: ["test", id.value],
        queryFn: async () => `body of ${id.value}`,
        staleTime: Infinity,
      })),
    );
    await vi.waitFor(() => expect(query.data.value).toBe("body of x"));
    id.value = "y";
    await nextTick();
    expect(observers(["test", "x"])).toBe(0);
    expect(observers(["test", "y"])).toBe(1);
    await vi.waitFor(() => expect(query.data.value).toBe("body of y"));
    stop();
    expect(observers(["test", "y"])).toBe(0);
  });

  it("useQuery reports the error of its query", async () => {
    const { value: query, stop } = inApp(() =>
      useQuery({
        queryKey: ["test", "failing"],
        queryFn: async (): Promise<string> => {
          throw new Error("down");
        },
        retry: false,
      }),
    );
    await vi.waitFor(() => expect(query.isError.value).toBe(true));
    expect(query.error.value?.message).toBe("down");
    stop();
  });

  it("useInfiniteQuery reads the next page on demand", async () => {
    const { value: list, stop } = inApp(() =>
      useInfiniteQuery({
        queryKey: ["test", "pages"],
        queryFn: async ({ pageParam }: { pageParam: number }) => ({ page: pageParam }),
        initialPageParam: 0,
        getNextPageParam: (last: { page: number }) => (last.page < 1 ? last.page + 1 : undefined),
      }),
    );
    await vi.waitFor(() => expect(list.data.value?.pages).toEqual([{ page: 0 }]));
    expect(list.hasNextPage.value).toBe(true);
    await list.fetchNextPage();
    await vi.waitFor(() => expect(list.data.value?.pages).toEqual([{ page: 0 }, { page: 1 }]));
    expect(list.hasNextPage.value).toBe(false);
    expect(observers(["test", "pages"])).toBe(1);
    stop();
    expect(observers(["test", "pages"])).toBe(0);
  });

  it("useMutation runs, keeps its error without throwing, and resets", async () => {
    const { value: mutation, stop } = inApp(() =>
      useMutation({
        mutationFn: async (n: number) => {
          if (n < 0) throw new Error("negative");
          return n * 2;
        },
      }),
    );
    expect(mutation.isIdle.value).toBe(true);
    await expect(mutation.mutateAsync(2)).resolves.toBe(4);
    await vi.waitFor(() => expect(mutation.data.value).toBe(4));
    mutation.mutate(-1);
    await vi.waitFor(() => expect(mutation.error.value?.message).toBe("negative"));
    mutation.reset();
    await vi.waitFor(() => expect(mutation.isIdle.value).toBe(true));
    stop();
  });
});
