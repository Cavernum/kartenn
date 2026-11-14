import { lazyCatalog } from "../i18n/index.ts";

// The words of the screens, out of the initial shell (task 22n, budget « Shell initial »):
// read with the chunk of each screen of the frame (`entryScreen`, `recordScreen`) and of
// each panel the frame opens itself (edit, closure, draft), both before it renders, so that
// no screen shows a key in place of its words.

/** Reads the words of the screens once, and merges them into the shell's. */
export const screenWords = lazyCatalog(() => import("./i18n/index.ts"));

/** `load`, which resolves once the words of the screens are read too. */
export const worded =
  <T>(load: () => Promise<T>) =>
  (): Promise<T> =>
    Promise.all([load(), screenWords()]).then(([module]) => module);
