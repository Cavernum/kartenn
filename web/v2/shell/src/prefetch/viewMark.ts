import { nextTick } from "vue";
import { START_LOCATION, type Router } from "vue-router";

/** The mark of a view drawn after a navigation (REQ-021-22, REQ-021-30; read by `bench-web`). */
export const VIEW_MARK = "k:view";

/**
 * After each navigation of the session but the first, a `k:view` mark, its detail the path
 * reached: once Vue has drawn the screen (`nextTick`), at the first animation frame after —
 * the end of a latency in the protocol of the prototype D-1. A navigation refused or
 * cancelled leaves no mark; the first one waits for the session and is the first screen,
 * not a navigation in cache.
 */
export function installViewMark(router: Router): void {
  router.afterEach((to, from, failure) => {
    if (failure || from === START_LOCATION) return;
    void nextTick(() =>
      requestAnimationFrame(() => performance.mark(VIEW_MARK, { detail: to.fullPath })),
    );
  });
}
