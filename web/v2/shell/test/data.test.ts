// @vitest-environment happy-dom
import { queryClient } from "@kartenn/sdk";
import { resetContext, setContext, setFetch } from "@kartenn/sdk/host";
import { useQueryClient } from "@tanstack/vue-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import { computed, createApp, effectScope } from "vue";
import { installData, useEventsState, useLabel } from "../src/data/query.ts";

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
  it("gives vue-query the one QueryClient of the SDK", () => {
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
    // vue-query observes the query: it subscribes only on a client with `isRestoring`.
    expect(
      queryClient
        .getQueryCache()
        .find({ queryKey: ["v1", "labels", "fr"] })
        ?.getObserversCount(),
    ).toBe(1);
    expect((fetch.mock.calls as unknown[][])[0]?.[0]).toBe("/v1/labels?lang=fr");
    stop();
  });

  it("follows the state of the event stream", () => {
    const { value: state, stop } = inApp(() => useEventsState());
    expect(state.value).toBe("closed");
    stop();
  });
});
