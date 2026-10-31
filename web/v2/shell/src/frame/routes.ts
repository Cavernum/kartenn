import type { RouteRecordRaw } from "vue-router";
import PendingView from "./PendingView.vue";
import { CORE_SPACES, type Space, type SpaceEntry } from "./spaces.ts";

// The routes of the frame (design 021 « Routes »), children of its record: one per entry
// of the spaces, each with its breadcrumb (space, then the collection); their views come
// with their tasks (11 and after), a pending view until then. The routes of an object
// (`/:collection/:id`), of the plugins (`/x/…`) and of the help come with their screens.

// In the first chunk, as the frame: a lazy screen that imports a component of @kartenn/ui
// splits that component out of the chunk of the library, and the build links its sheet
// before that of the library, whose order of the layers then comes second (test/build).
const Pending = PendingView;

/** The record of an entry of a space: its path below the frame, its breadcrumb. */
function entryRoute(space: Space, entry: SpaceEntry): RouteRecordRaw {
  return {
    path: entry.to.slice(1),
    name: entry.id,
    component: Pending,
    meta: {
      breadcrumb: (_, t) => [
        { label: t(space.label), to: space.home },
        { label: t(entry.label), to: entry.to },
      ],
    },
  };
}

export const frameRoutes: RouteRecordRaw[] = [
  ...CORE_SPACES.flatMap((space) => space.entries.map((entry) => entryRoute(space, entry))),
  // The administration opens on its first screen.
  { path: "admin", redirect: "/admin/groups" },
  {
    path: ":missing(.*)*",
    name: "missing",
    component: Pending,
    props: { missing: true },
    meta: { breadcrumb: (_, t) => [{ label: t("app.notFound") }] },
  },
];
