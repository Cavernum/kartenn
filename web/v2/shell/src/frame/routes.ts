import type { RouteRecordRaw } from "vue-router";
import { useSession } from "../rights/index.ts";
import { SEARCHED } from "../shortcuts/search.ts";
import HelpView from "../shortcuts/HelpView.vue";
import PendingView from "./PendingView.vue";
import { CORE_SPACES, spaceEntry, type Space, type SpaceEntry } from "./spaces.ts";

// The routes of the frame (design 021 « Routes »), children of its record: one per entry
// of the spaces, each with its breadcrumb (space, then the collection); the record of an
// object the palette finds (`/:collection/:id`); the help of the shortcuts
// (`/help/shortcuts`). The views of the entries and the records come with their tasks (11
// and after), a pending view until then, in the first chunk with the frame. The routes of
// the plugins (`/x/…`) come with their host (task 16).

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

/** The record of an object of a collection (`/prefixes/:id`): its crumb, its identifier. */
function recordRoute(space: Space, entry: SpaceEntry): RouteRecordRaw {
  return {
    path: `${entry.to.slice(1)}/:id`,
    name: `${entry.id}-record`,
    component: PendingView,
    meta: {
      breadcrumb: (params, t) => [
        { label: t(space.label), to: spaceEntry(space, session.value.rights).to },
        { label: t(entry.label), to: entry.to },
        { label: String(params.id), mono: true },
      ],
    },
  };
}

/** The collections whose objects have a record: those the palette searches. */
const RECORDS = new Set(SEARCHED.map((searched) => searched.path.slice(3)));

const admin = CORE_SPACES.find((space) => space.id === "admin")!;

export const frameRoutes: RouteRecordRaw[] = [
  ...CORE_SPACES.flatMap((space) => space.entries.map((entry) => entryRoute(space, entry))),
  ...CORE_SPACES.flatMap((space) =>
    space.entries
      .filter((entry) => RECORDS.has(entry.to))
      .map((entry) => recordRoute(space, entry)),
  ),
  // The administration opens on its first screen the user may see.
  { path: "admin", redirect: () => spaceEntry(admin, session.value.rights).to },
  {
    path: "help/shortcuts",
    name: "help-shortcuts",
    component: HelpView,
    meta: { breadcrumb: (_, t) => [{ label: t("shortcuts.title") }] },
  },
  {
    path: ":missing(.*)*",
    name: "missing",
    component: PendingView,
    props: { missing: true },
    meta: { breadcrumb: (_, t) => [{ label: t("app.notFound") }] },
  },
];
