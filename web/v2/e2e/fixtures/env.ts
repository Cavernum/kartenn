import { fileURLToPath } from "node:url";

/** The instance of the journeys (web/v2/e2e/kartenn.toml). */
export const BASE_URL = "https://127.0.0.1:9443";
/** The development issuer (web/e2e/users.toml), shared with the old journeys. */
export const ISSUER = "https://127.0.0.1:9444";
/** Material written by the servers of the journeys (web/e2e/serve.sh, web/v2/e2e/serve.sh). */
export const STATE = fileURLToPath(new URL("../../../e2e/.state/", import.meta.url));

/**
 * The reads a request makes, by path: a batch of properties (`/v1/<c>/properties?id=…`,
 * REQ-006-40, task 021-11p10-c) reads `/v1/<c>/<id>/properties` for each of its `id`; any
 * other request, its path.
 */
export function readsOf(url: string): string[] {
  const { pathname, searchParams } = new URL(url);
  const batch = /^(\/v1\/[a-z-]+)\/properties$/.exec(pathname);
  if (!batch) return [pathname];
  return searchParams.getAll("id").map((id) => `${batch[1]}/${id}/properties`);
}
