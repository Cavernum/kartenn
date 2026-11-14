// @vitest-environment happy-dom
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { WIDTH_CLASSES, type WidthClass } from "@kartenn/ui";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, defineComponent, h, nextTick, type App, type ComputedRef } from "vue";
import { createMemoryHistory, createRouter, RouterView } from "vue-router";
import { followWindow, frameWidth, setFrameWidth } from "../src/frame/width.ts";
import { createShellI18n } from "../src/i18n/index.ts";
import { refusalWords, timeOf, useWrites, writesOf, type Writes } from "../src/time/index.ts";
import { whyWords } from "../src/views/edit/inline.ts";

// The gate of writes (task 021-21c; REQ-021-29, Q-021-12; REQ-021-5): one function of the
// period and of the width class of the frame, one refusal, its sentence. The model test of
// the surfaces that follow it (21d to 21f): a surface reads `useWrites()`, refuses at its
// point of action with `refusalWords` (or `edit.why.narrow` where it said `past`).

const VALID_AT = "2026-01-01T00:00:00Z";
const AS_OF = "2026-06-30T12:00:00Z";
const PERIODS = {
  present: {},
  dated: { valid_at: VALID_AT },
  asOf: { as_of: AS_OF },
  both: { valid_at: VALID_AT, as_of: AS_OF },
} as const;

afterEach(() => setFrameWidth("wide"));

/**
 * The surfaces that still read the period alone (`writesOf(time.value)`, without the width):
 * the simulation of a closure (a read), and those of tasks 21d to 21f, which take up
 * `useWrites()` and leave this list. A new one fails here: a write that forgets the width.
 */
const PERIOD_ONLY = [
  "draft/DraftsView.vue",
  "draft/LostSubmissions.vue",
  "draft/state.ts",
  "views/RecordView.vue",
  "views/TopologyView.vue",
  "views/edit/ClosePanel.vue",
];

describe("the readers of the period alone", () => {
  // A path, not a URL: happy-dom's `URL` is not Node's.
  const SRC = join(import.meta.dirname, "../src");
  it("are the ones listed, the others going through useWrites", () => {
    const files = readdirSync(SRC, { recursive: true, encoding: "utf8" }).filter(
      (name) => /\.(ts|vue)$/.test(name) && name !== "time/index.ts",
    );
    const found = files
      .filter((name) =>
        /writesOf\([^,()]*(\([^()]*\))?[^,()]*\)/.test(readFileSync(join(SRC, name), "utf8")),
      )
      .sort();
    expect(found).toEqual(PERIOD_ONLY);
  });
});

describe("the gate of writes, width × period", () => {
  const open = (quickEdit: boolean): Writes => ({ quickEdit, draft: true, refused: null });
  const past: Writes = { quickEdit: false, draft: false, refused: "past" };
  const narrow: Writes = { quickEdit: false, draft: false, refused: "narrow" };

  const expected: Record<WidthClass, Record<keyof typeof PERIODS, Writes>> = {
    compact: { present: narrow, dated: narrow, asOf: past, both: past },
    narrow: { present: narrow, dated: narrow, asOf: past, both: past },
    standard: { present: open(true), dated: open(false), asOf: past, both: past },
    wide: { present: open(true), dated: open(false), asOf: past, both: past },
  };

  for (const { id: width } of WIDTH_CLASSES) {
    for (const [period, query] of Object.entries(PERIODS)) {
      it(`${width} × ${period}`, () => {
        expect(writesOf(timeOf(query), width)).toEqual(
          expected[width][period as keyof typeof PERIODS],
        );
      });
    }
  }

  it("reads the period alone without a width (a read that a write follows)", () => {
    expect(writesOf(timeOf({}))).toEqual(open(true));
    expect(writesOf(timeOf(PERIODS.dated))).toEqual(open(false));
    expect(writesOf(timeOf(PERIODS.asOf))).toEqual(past);
  });

  it("says its refusal in one sentence, the cells' own", () => {
    const { t } = createShellI18n("fr").global;
    const words = (writes: Writes) => refusalWords(writes, t);
    expect(words(narrow)).toBe(
      "Modification à partir de 1024 px de large : élargissez la fenêtre pour modifier.",
    );
    expect(words(past)).toBe(t("edit.why.past"));
    expect(words(open(false))).toBeNull();
    // A cell says the same.
    expect(whyWords({ kind: "narrow" }, t)).toBe(words(narrow));
    expect(createShellI18n("en").global.t("edit.why.narrow")).toBe(
      "Changes from 1024 px wide: widen the window to change it.",
    );
  });
});

describe("the width of the frame", () => {
  /** A window of `width` CSS pixels, its queries answering for it, resized by `resize`. */
  function fakeWindow(width: number) {
    const lists: { query: string; listeners: Set<() => void>; matches: boolean }[] = [];
    const matches = (query: string) => {
      const from = /(\d+)px <= width/.exec(query) ?? /width >= (\d+)px/.exec(query);
      const below = /width < (\d+)px/.exec(query);
      return (!from || width >= Number(from[1])) && (!below || width < Number(below[1]));
    };
    vi.stubGlobal("matchMedia", (query: string) => {
      const list = { query, listeners: new Set<() => void>(), matches: matches(query) };
      lists.push(list);
      return {
        get matches() {
          return list.matches;
        },
        addEventListener: (_: string, f: () => void) => list.listeners.add(f),
        removeEventListener: (_: string, f: () => void) => list.listeners.delete(f),
      };
    });
    return {
      resize(to: number) {
        width = to;
        for (const list of lists) list.matches = matches(list.query);
        for (const list of lists) for (const f of list.listeners) f();
      },
      listening: () => lists.reduce((n, list) => n + list.listeners.size, 0),
    };
  }
  afterEach(() => vi.unstubAllGlobals());

  it("follows the window by the queries of the sheet, until stopped", () => {
    const view = fakeWindow(1440);
    const stop = followWindow();
    expect(frameWidth.value).toBe("wide");
    view.resize(1024);
    expect(frameWidth.value).toBe("standard");
    view.resize(768);
    expect(frameWidth.value).toBe("narrow");
    view.resize(390);
    expect(frameWidth.value).toBe("compact");
    stop();
    expect(view.listening()).toBe(0);
    view.resize(1440);
    expect(frameWidth.value).toBe("compact");
  });

  let app: App | undefined;
  afterEach(() => {
    app?.unmount();
    app = undefined;
  });

  it("closes and opens the gate as the frame narrows and widens (useWrites)", async () => {
    let writes: ComputedRef<Writes> | undefined;
    const Probe = defineComponent({
      setup() {
        writes = useWrites();
        return () => h("p");
      },
    });
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: "/", component: Probe }],
    });
    await router.push("/");
    app = createApp({ render: () => h(RouterView) });
    app.use(router).mount(document.createElement("div"));
    await router.isReady();
    expect(writes!.value.refused).toBeNull();
    setFrameWidth("narrow");
    expect(writes!.value).toEqual({ quickEdit: false, draft: false, refused: "narrow" });
    await router.push({ path: "/", query: { as_of: AS_OF } });
    await nextTick();
    expect(writes!.value.refused).toBe("past");
    await router.push({ path: "/", query: {} });
    setFrameWidth("standard");
    await nextTick();
    expect(writes!.value).toEqual({ quickEdit: true, draft: true, refused: null });
  });
});
