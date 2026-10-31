import { startData } from "@kartenn/sdk/host";
import type { Router } from "vue-router";
import { loadSession, type Session } from "../rights/index.ts";

declare module "vue-router" {
  interface RouteMeta {
    /** A route of the frame: its first page waits for the session (`GET /v1/me`). */
    frame?: boolean;
  }
}

/**
 * The first page of the frame waits for `GET /v1/me`, read once (the sidebar and the user
 * menu follow the rights it gives); signed in, the data layer starts then (cache mounted,
 * event stream opened before the first reads of the screen, 022). The catalogue /_ui never
 * reads it. Without a session the frame shows what needs no right: the login page, the
 * return URL and the dialog of an expired session are task 16's.
 */
export function installSession(
  router: Router,
  load: () => Promise<Pick<Session, "me">> = loadSession,
  start: () => unknown = startData,
): void {
  let first: Promise<unknown> | null = null;
  router.beforeEach(async (to) => {
    if (!to.matched.some((record) => record.meta.frame)) return true;
    first ??= load().then((session) => {
      if (session.me) start();
    });
    await first;
    return true;
  });
}
