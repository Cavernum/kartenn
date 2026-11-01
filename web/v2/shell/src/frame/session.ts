import { startData } from "@kartenn/sdk/host";
import type { Router } from "vue-router";
import { loadSession, useSession, type Session } from "../rights/index.ts";
import { takeReturn } from "../session/index.ts";

declare module "vue-router" {
  interface RouteMeta {
    /** A route of the frame: its first page waits for the session (`GET /v1/me`). */
    frame?: boolean;
  }
}

/**
 * The session on the router (REQ-021-14, REQ-007-28): the first page of the frame waits for
 * `GET /v1/me`, read once (the sidebar and the user menu follow the rights it gives);
 * signed in, the data layer starts then (cache mounted, event stream opened before the
 * first reads of the screen, 022), or after a login in place (a break-glass account).
 * Without a session, a page that needs one sends to `/login` with its URL to come back to;
 * the pages open without one (`meta.public`) show. The first page after a login goes back
 * to the URL kept. The catalogue /_ui never reads the session.
 */
export function installSession(
  router: Router,
  load: () => Promise<Pick<Session, "me">> = loadSession,
  start: () => unknown = startData,
  current: () => Pick<Session, "me"> = () => useSession().value,
): void {
  let first: Promise<Pick<Session, "me">> | null = null;
  let started = false;
  router.beforeEach(async (to) => {
    if (!to.matched.some((record) => record.meta.frame)) return true;
    first ??= load();
    // The session as read first, or as a login in place has read it since.
    const me = current().me ?? (await first).me;
    if (!me) {
      if (to.matched.some((record) => record.meta.public)) return true;
      return { path: "/login", query: { return: to.fullPath } };
    }
    if (!started) {
      started = true;
      start();
    }
    const back = takeReturn();
    return back && back !== to.fullPath ? back : true;
  });
}
