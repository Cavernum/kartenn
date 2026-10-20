import { fileURLToPath } from "node:url";

/** The instance of the journeys (web/e2e/kartenn.toml). */
export const BASE_URL = "https://127.0.0.1:9443";
/** The development issuer (web/e2e/users.toml), also the issuer of the bindings. */
export const ISSUER = "https://127.0.0.1:9444";
/** Material written by serve.sh: certificates, secrets, break-glass proofs. */
export const STATE = fileURLToPath(new URL("../.state/", import.meta.url));
