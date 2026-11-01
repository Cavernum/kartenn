import type { RouteParams, RouteRecordRaw } from "vue-router";
import { pluginSpaces } from "../plugins/navigation.ts";
import PluginView from "../plugins/PluginView.vue";
import { labelOf, usePlugins, viewPath } from "../plugins/registry.ts";
import { useSession } from "../rights/index.ts";
import LoginView from "../session/LoginView.vue";
import { SEARCHED } from "../shortcuts/search.ts";
import HelpView from "../shortcuts/HelpView.vue";
import AboutView from "./AboutView.vue";
import PendingView from "./PendingView.vue";
import { CORE_SPACES, locate, spaceEntry, type Space, type SpaceEntry } from "./spaces.ts";

// The routes of the frame (design 021 « Routes »), children of its record: one per entry
// of the spaces, each with its breadcrumb (space, then the collection); the record of an
// object the palette finds (`/:collection/:id`); the help of the shortcuts
// (`/help/shortcuts`); the login page and « À propos », open without a session; the views
// of the plugins (`/x/<plugin>/<id>/*`). The views of the entries and the records come with
// their tasks (11 and after), a pending view until then, in the first chunk with the frame.

declare module "vue-router" {
  interface RouteMeta {
    /** Open without a session: the login page, « À propos ». */
    public?: boolean;
  }
}

const session = useSession();
const plugins = usePlugins();

/** The plugin of a route `/x/<plugin>/…`, as the capabilities list it. */
function pluginOf(params: RouteParams) {
  return plugins.value.find((p) => p.plugin.name === String(params.plugin));
}

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
  // Open without a session (task 16): the login page, « À propos ».
  {
    path: "login",
    name: "login",
    component: LoginView,
    meta: { public: true, breadcrumb: (_, t) => [{ label: t("login.title") }] },
  },
  {
    path: "about",
    name: "about",
    component: AboutView,
    meta: { public: true, breadcrumb: (_, t) => [{ label: t("about.title") }] },
  },
  // A view of a plugin under its space (REQ-021-13; design « Routes »): its modules
  // preloaded on the intent of a link to it, with the reads.
  {
    path: "x/:plugin/:view/:rest(.*)*",
    name: "plugin-view",
    component: PluginView,
    meta: {
      modules: (params) => pluginOf(params)?.plugin ?? null,
      breadcrumb: (params, t) => {
        const plugin = String(params.plugin);
        const view = String(params.view);
        const found = locate(viewPath(plugin, view), pluginSpaces(plugins.value));
        // A view without an entry of navigation (or of a plugin not loaded yet): its label,
        // else its plugin and its id.
        const extension = pluginOf(params)?.plugin.extensions.find((e) => e.id === view);
        const own = labelOf(t, {
          ...(extension ?? { id: view, element: "", point: "view" }),
          plugin,
        });
        const label = { label: found ? t(found.entry.label) : own };
        if (!found) return [label];
        const first = spaceEntry(found.space, session.value.rights);
        return [{ label: t(found.space.label), to: first.to }, label];
      },
    },
  },
  {
    path: ":missing(.*)*",
    name: "missing",
    component: PendingView,
    props: { missing: true },
    meta: { breadcrumb: (_, t) => [{ label: t("app.notFound") }] },
  },
];
