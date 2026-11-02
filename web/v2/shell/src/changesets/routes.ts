import { drafts, reads } from "@kartenn/sdk";
import type { ScreenRoute } from "../views/routes.ts";

// The screens of the space `changes` (task 14; design 021 « Routes »): the changesets, « À
// instruire », the drafts of the user, and the records of a changeset and of a draft, each
// in a chunk of its own with the reads its route declares (`meta.queries`), prefetched on
// the intent of a link (REQ-021-22). The imports (`/imports`) are those of the
// administration (`admin/routes.ts`, task 15c).

/** The screen of an entry of the space `changes`, by its id. */
export function changesScreen(id: string): ScreenRoute | undefined {
  switch (id) {
    case "changesets":
      return {
        component: () => import("./ChangesetsView.vue"),
        queries: () => [reads.list("/v1/changesets")],
      };
    case "proposals":
      return {
        component: () => import("./ProposalsView.vue"),
        queries: () => [reads.list("/v1/proposals")],
      };
    case "drafts":
      return { component: () => import("../draft/DraftsView.vue"), queries: () => [drafts.list()] };
    default:
      return undefined;
  }
}

/** The record of a changeset or of a draft (`/changesets/:id`, `/drafts/:id`). */
export function changesRecord(id: string): ScreenRoute | undefined {
  switch (id) {
    case "changesets":
      return {
        component: () => import("./ChangesetView.vue"),
        props: (route) => ({ id: String(route.params.id) }),
        queries: (params) => [
          reads.object("changeset", String(params.id)),
          reads.subPages("changeset", String(params.id), "diff"),
        ],
      };
    case "drafts":
      return {
        component: () => import("../draft/DraftView.vue"),
        props: (route) => ({ id: String(route.params.id) }),
        queries: (params) => [drafts.get(String(params.id))],
      };
    default:
      return undefined;
  }
}
