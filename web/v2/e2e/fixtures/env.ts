import { fileURLToPath } from "node:url";

/** The instance of the journeys (web/v2/e2e/kartenn.toml). */
export const BASE_URL = "https://127.0.0.1:9443";
/** The development issuer (web/e2e/users.toml), shared with the old journeys. */
export const ISSUER = "https://127.0.0.1:9444";
/** Material written by the servers of the journeys (web/e2e/serve.sh, web/v2/e2e/serve.sh). */
export const STATE = fileURLToPath(new URL("../../../e2e/.state/", import.meta.url));
