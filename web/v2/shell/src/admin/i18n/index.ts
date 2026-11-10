import type { Catalog } from "../../i18n/index.ts";
import en from "./en.json";
import fr from "./fr.json";

// The words of the administration (task 15a, under `admin`), a chunk of their own read with
// the screens of the space (`admin/routes.ts`, task 22h): no other screen uses them.
export default { fr, en } satisfies Catalog;
