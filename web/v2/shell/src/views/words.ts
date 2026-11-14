import { lazyCatalog } from "../i18n/index.ts";

// The words of the screens, out of the initial shell (task 22n, budget « Shell initial »):
// read with the chunk of each screen of the frame (`entryScreen`, `recordScreen`) and of
// each panel the frame opens itself (`frame/panels.ts`), both before it renders, so that
// no screen shows a key in place of its words.

/** Reads the words of the screens once, and merges them into the shell's. */
export const screenWords = lazyCatalog(() => import("./i18n/index.ts"));

/** The loaders `worded` made, which `test/words.test.ts` looks for on each route. */
const loaders = new WeakSet<object>();

/** `load`, which resolves once the words of the screens are read too. */
export function worded<T>(load: () => Promise<T>): () => Promise<T> {
  const run = (): Promise<T> => Promise.all([load(), screenWords()]).then(([module]) => module);
  loaders.add(run);
  return run;
}

/** Whether `load` awaits the words of the screens (`worded`). */
export const isWorded = (load: unknown): boolean => typeof load === "function" && loaders.has(load);
