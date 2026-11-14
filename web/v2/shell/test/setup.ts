import { screenWords } from "../src/views/words.ts";

// Before each file of tests: the words of the screens, which the shell reads with the route
// of each screen (`views/words.ts`, task 22n), where a test mounts a screen without it. Read
// before the i18n of a test is created, they are merged into it at its creation.
await screenWords();
