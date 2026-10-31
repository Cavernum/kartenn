import type { RouteRecordRaw } from "vue-router";
import { useSession } from "../rights/index.ts";
import PendingView from "./PendingView.vue";
import { CORE_SPACES, spaceEntry, type Space, type SpaceEntry } from "./spaces.ts";

// The routes of the frame (design 021 « Routes »), children of its record: one per entry
// of the spaces, each with its breadcrumb (space, then the collection); their views come
// with their tasks (11 and after), a pending view until then, in the first chunk with the
// frame. The routes of an object (`/:collection/:id`), of the plugins (`/x/…`) and of the
// help come with their screens.

const session = useSession();

/** The record of an entry of a space: its path below the frame, its breadcrumb. */
function entryRoute(space: Space, entry: SpaceEntry): RouteRecordRaw {
  return {
    path: entry.to.slice(1),
    name: entry.id,
    component: PendingView,
    meta: {
      // The space leads to its first entry the user may see.
      breadcrumb: (_, t) => [
        { label: t(space.label), to: spaceEntry(space, session.value.rights).to },
        { label: t(entry.label), to: entry.to },
      ],
    },
  };
}

const admin = CORE_SPACES.find((space) => space.id === "admin")!;

export const frameRoutes: RouteRecordRaw[] = [
  ...CORE_SPACES.flatMap((space) => space.entries.map((entry) => entryRoute(space, entry))),
  // The administration opens on its first screen the user may see.
  { path: "admin", redirect: () => spaceEntry(admin, session.value.rights).to },
  {
    path: ":missing(.*)*",
    name: "missing",
    component: PendingView,
    props: { missing: true },
    meta: { breadcrumb: (_, t) => [{ label: t("app.notFound") }] },
  },
];
