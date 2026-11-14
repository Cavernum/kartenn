import { screenWords } from "../src/views/words.ts";

// Before each file of tests: the words of the screens, which the shell reads with the route
// of each screen (`views/words.ts`, task 22n), where a test mounts a screen without it. Read
// before the i18n of a test is created, they are merged into it at its creation.
await screenWords();

// The `IntersectionObserver` of happy-dom never calls back, so that a cell reading only in
// view (`data/inView.ts`) would never read: without one, every element is in view, as on
// the screen of a test. A test of the view itself stubs its own (`gate.test.ts`).
(globalThis as { IntersectionObserver?: unknown }).IntersectionObserver = undefined;
