import type { Catalog } from "../../i18n/index.ts";
import en from "./en.json";
import fr from "./fr.json";

// The words of the screens (task 11, under `views` and `edit`), a chunk of their own read
// with the screens and the panels that use them (`views/words.ts`, task 22n). The words the
// frame reads before any screen (labels, problems, `views.retry`, the names of the fields)
// stay in the initial shell, `shared/`.
export default { fr, en } satisfies Catalog;
