// @vitest-environment happy-dom
import { ApiProblem, NetworkError, queryClient } from "@kartenn/sdk";
import { resetContext, setFetch } from "@kartenn/sdk/host";
import { afterEach, describe, expect, it } from "vitest";
import { createApp, h, type App } from "vue";
import { createShellI18n } from "../src/i18n/index.ts";
import ProblemState from "../src/problems/ProblemState.vue";
import { useProblems } from "../src/problems/index.ts";
import RefusalAlert from "../src/views/edit/RefusalAlert.vue";
import type { Refusal } from "../src/views/edit/inline.ts";

// The one way the shell says a problem (task 021-13; REQ-021-19, 20): its type in plain
// words — the dictionary of the 018 first, then the catalogs, then the answer's words —
// and beside it the codes as served (type, rules, Request-Id), copyable.

let app: App | undefined;
afterEach(() => {
  app?.unmount();
  app = undefined;
  queryClient.clear();
  resetContext();
});

/** The dictionary served: `problem` labelled as the server would. */
function serve(problem: Record<string, string>): string[] {
  const seen: string[] = [];
  setFetch((async (input: RequestInfo | URL) => {
    seen.push(String(input));
    const body = { lang: "fr", default_lang: "fr", revision: "1", spaces: { problem } };
    return new Response(JSON.stringify(body), {
      headers: { "Content-Type": "application/json", ETag: '"1.fr"' },
    });
  }) as typeof fetch);
  return seen;
}

/** A component mounted under the shell's i18n, in French: `setup` gives its render. */
function mount(setup: () => () => ReturnType<typeof h>): HTMLElement {
  const root = document.createElement("div");
  app = createApp({ setup });
  app.use(createShellI18n("fr"));
  app.mount(root);
  return root;
}

const problem = (type: string, init: Partial<ConstructorParameters<typeof ApiProblem>[0]> = {}) =>
  new ApiProblem({ status: 422, type, title: "Unprocessable", requestId: "req-1", ...init });

describe("the words of a problem", () => {
  it("say its type: the dictionary, the catalog, the answer's words, the network", async () => {
    const seen = serve({ "precondition-failed": "La ressource a changé" });
    let words!: ReturnType<typeof useProblems>;
    mount(() => {
      words = useProblems();
      return () => h("div");
    });
    const forbidden = problem("urn:kartenn:problem:forbidden", { status: 403 });
    // The dictionary not yet read: the catalog.
    expect(words.sentence(forbidden)).toBe("Vous n'avez pas le droit de faire cela.");
    await expect
      .poll(() => words.sentence(problem("urn:kartenn:problem:precondition-failed")))
      .toBe("La ressource a changé.");
    expect(seen).toEqual(["/v1/labels?lang=fr"]);
    // A type of no catalog: the answer's own words; none, the request failed.
    expect(words.sentence(problem("urn:other:x", { detail: "Something else" }))).toBe(
      "Something else.",
    );
    expect(words.sentence(problem("about:blank", { title: "" }))).toBe(
      "La requête n'a pas abouti.",
    );
    // No answer is the network's; a bug of the client is never said to be.
    expect(words.sentence(new NetworkError(new TypeError("Failed to fetch")))).toBe(
      "Le serveur n'a pas pu être joint.",
    );
    expect(words.sentence(new TypeError("x is undefined"))).toBe(
      "Une erreur de l'interface a interrompu l'affichage ; rechargez la page.",
    );
  });

  it("the space problem as the server serves it: its sentence shown as it is", async () => {
    // The labels of `ProblemKind::labels()` (kartenn-rest, problem.rs): a sentence and its
    // stop, in the language asked.
    serve({
      forbidden: "Vous n'avez pas le droit de faire cela.",
      "precondition-failed": "L'objet a changé depuis votre lecture.",
      "draft-limit": "Vous avez atteint le nombre maximal de brouillons.",
    });
    let words!: ReturnType<typeof useProblems>;
    mount(() => {
      words = useProblems();
      return () => h("div");
    });
    await expect
      .poll(() => words.sentence(problem("urn:kartenn:problem:draft-limit")))
      .toBe("Vous avez atteint le nombre maximal de brouillons.");
    expect(words.sentence(problem("urn:kartenn:problem:precondition-failed"))).toBe(
      "L'objet a changé depuis votre lecture.",
    );
  });

  it("lines: what it is about, then the type with its words, then the Request-Id", () => {
    serve({});
    let words!: ReturnType<typeof useProblems>;
    mount(() => {
      words = useProblems({ dictionary: false });
      return () => h("div");
    });
    const refused = problem("urn:kartenn:problem:changeset-refused");
    expect(
      words.lines(refused, [{ message: "Non aligné.", code: "core.prefix.unaligned" }]),
    ).toEqual([
      { message: "Non aligné.", code: "core.prefix.unaligned" },
      {
        message: "Le changement est refusé par les règles de cohérence.",
        code: "urn:kartenn:problem:changeset-refused",
      },
      { message: "Request-Id", code: "req-1" },
    ]);
    // The title already says it: the type alone, under its name.
    expect(words.lines(refused, [], { said: true })).toEqual([
      { message: "Code du problème", code: "urn:kartenn:problem:changeset-refused" },
      { message: "Request-Id", code: "req-1" },
    ]);
    // `about:blank` has no code to pass on, the network neither type nor request.
    expect(words.lines(problem("about:blank", { requestId: null }))).toEqual([
      { message: "Unprocessable." },
    ]);
    expect(words.lines(null)).toEqual([{ message: "Le serveur n'a pas pu être joint." }]);
    expect(words.lines(null, [], { said: true })).toEqual([]);
  });
});

describe("the renderings of a problem", () => {
  it("an error state: what failed, why, the codes copyable, « Réessayer »", async () => {
    serve({});
    let retried = 0;
    const root = mount(
      () => () =>
        h(ProblemState, {
          title: "La liste n'a pas pu être lue.",
          error: problem("urn:kartenn:problem:forbidden", { status: 403 }),
          retry: () => (retried += 1),
        }),
    );
    expect(root.textContent).toContain("Vous n'avez pas le droit de faire cela.");
    const copy = [...root.querySelectorAll("button")].map((b) => b.textContent?.trim());
    expect(copy).toContain("Copier type urn:kartenn:problem:forbidden");
    expect(copy).toContain("Copier Request-Id req-1");
    [...root.querySelectorAll("button")].find((b) => b.textContent?.includes("Réessayer"))!.click();
    expect(retried).toBe(1);
    // In the alert, said with the title: why, then what to do with the codes.
    const alert = root.querySelector("[role='alert']")!;
    expect(alert.textContent).toContain("Vous n'avez pas le droit de faire cela.");
    expect(alert.textContent).toContain("Si l'erreur persiste, transmettez ces codes.");
  });

  it("an error state says its why once: the dictionary arriving never rewrites it", async () => {
    const seen = serve({ forbidden: "Libellé du dictionnaire." });
    const root = mount(
      () => () =>
        h(ProblemState, {
          title: "La liste n'a pas pu être lue.",
          error: problem("urn:kartenn:problem:forbidden", { status: 403 }),
        }),
    );
    const said = () => root.querySelector("[role='alert']")!.textContent;
    const first = said();
    expect(first).toContain("Vous n'avez pas le droit de faire cela.");
    await expect.poll(() => seen.length).toBe(1);
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(said()).toBe(first);
  });

  it("no answer: the network said, no code to pass on", () => {
    serve({});
    const root = mount(
      () => () =>
        h(ProblemState, {
          title: "La liste n'a pas pu être lue.",
          error: new NetworkError(new TypeError("Failed to fetch")),
        }),
    );
    expect(root.textContent).toContain("Le serveur n'a pas pu être joint.");
    expect(root.textContent).not.toContain("transmettez ces codes");
    expect(root.querySelector(".k-code-badge")).toBeNull();
  });

  it("a 422 refused: each violation with its rule, the type and the Request-Id, copyable", () => {
    serve({});
    const answer = problem("urn:kartenn:problem:changeset-refused", {
      violations: [
        {
          rule: "core.prefix.unaligned",
          detail: "Non aligné.",
          severity: "error",
          pointer: "/value",
          subjects: [],
        },
        {
          rule: "core.prefix.overlap",
          detail: "Chevauche 192.0.2.0/25.",
          severity: "error",
          subjects: [],
        },
      ],
    });
    const refusal: Refusal = {
      kind: "invalid",
      byField: { value: [answer.violations[0]!] },
      general: [answer.violations[1]!],
      input: {},
      problem: answer,
    };
    const root = mount(() => () => h(RefusalAlert, { refusal }));
    const codes = [...root.querySelectorAll("button code")].map((c) => c.textContent);
    expect(codes).toEqual([
      "core.prefix.unaligned",
      "core.prefix.overlap",
      "urn:kartenn:problem:changeset-refused",
      "req-1",
    ]);
    // The title says it is invalid: the type under its name, not said again.
    expect(root.textContent).toContain("Code du problème");
    expect(root.textContent).not.toContain("Le changement est refusé par les règles");
  });
});
