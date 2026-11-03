<script setup lang="ts">
// The frame of the shell (spec 021, REQ-021-1, 2, 4, 5, 6; design « Cadre », revised by the
// 024: M8, M15), after the validated mock-up (/_ui/ecrans, EcranCadre.vue): fixed slots
// laid by its grid — sidebar of spaces (collapsible, kept), status bar (breadcrumb, date
// pill, draft counter, user menu), band out of the present, work surface, right panel
// (Détail, Brouillon n) beside it — and the toasts at the bottom of the window. Each slot
// scrolls inside itself; the page never does: the bar and the band never cover the focus
// (2.4.11). The surface is an isolated stacking context. The frame gives the library its
// link (`KLink`: router and prefetch on intent) and the place of its floating layers.
//
// Adaptation to the width (spec 020, « Adaptation à la largeur »), as the mock-up: the frame
// alone reads the window, in the four classes of `WIDTH_CLASSES` (copied in the media
// queries below). Wide and standard: sidebar as kept, the panel beside the surface. Narrow:
// the sidebar a rail, the panel covers the surface. Compact: one column, the sidebar in a
// drawer opened from the bar, each panel a sheet over the window.
//
// The shortcuts (task 10; design « Raccourcis »): the registry of the shell and the plugins'
// shortcuts, one `keydown` listener on the document, the help `?` in a dialog, the palette
// Ctrl+K (objects by value and by name, recent ones, commands of the view and of the
// plugins, pages).
//
// The plugins and the session (task 16): the views of the loaded plugins under their
// spaces and their `g` letters (a letter taken refused, said in « À propos »); « À propos »,
// login and logout in the user menu; « Session expirée » on a 401 in use, and the end of
// the session said before it comes, in a toast.
//
// The edit panel of a kind (task 12) in the Détail tab, when a view asks for it.
//
// The draft (task 14): the active draft of the tab in the Brouillon tab, its operations and
// simulation, its foot (motive, split button) in the foot of the panel, each in a chunk of
// its own; a draft that left the tab said by a toast; the counter of « À instruire » in the
// sidebar, read again on the event `changeset`.
//
// Left to the next tasks: the other counters of the sidebar (anomalies, inference).
import { drafts as draftStore, reads, type DraftList } from "@kartenn/sdk";
import { commandRegistry, draftHost, setDraftTitle, shortcutRegistry } from "@kartenn/sdk/host";
import {
  Breadcrumb,
  CommandPalette,
  DatePill,
  Dialog,
  DraftCounter,
  Drawer,
  IconButton,
  Menu,
  SidePanel,
  SidebarNav,
  TimeBand,
  ToastRegion,
  WIDTH_CLASSES,
  createToasts,
  provideLink,
  providePortalTarget,
  provideToasts,
  t as uiT,
  widthQuery,
  type DraftSummary,
  type MenuItem,
  type PaletteGroup,
  type Shortcut,
  type TabItem,
  type WidthClass,
} from "@kartenn/ui";
import {
  computed,
  defineAsyncComponent,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  watch,
} from "vue";
import { useI18n } from "vue-i18n";
import { RouterView, useRoute, useRouter } from "vue-router";
import { useInfiniteQuery, useQuery } from "../data/query.ts";
import { changeLocale, LOCALE_NAMES, LOCALES, type Locale } from "../i18n/index.ts";
import { pluginLetters, pluginSpaces, registerLetters } from "../plugins/navigation.ts";
import { refuseLetters, usePlugins } from "../plugins/registry.ts";
import KLink from "../prefetch/KLink.vue";
import { useSession } from "../rights/index.ts";
import { watchSessionEnd } from "../session/warning.ts";
import { logout } from "../session/index.ts";
import SessionDialog from "../session/SessionDialog.vue";
import {
  checkPeriod,
  goToPeriod,
  probePath,
  refusalOnField,
  toPresent,
  useTime,
  type Period,
} from "../time/index.ts";
import { problemOf, useProblems } from "../problems/index.ts";
import { provideShortcuts } from "../shortcuts/context.ts";
import {
  objectEntry,
  paletteGroups as groupsOfPalette,
  pluginEntries,
  type PaletteEntry,
} from "../shortcuts/palette.ts";
import { recentObjects, rememberObject, type FoundObject } from "../shortcuts/recent.ts";
import { createKeyListener, createShortcutRegistry } from "../shortcuts/registry.ts";
import { readableType, searchObjects } from "../shortcuts/search.ts";
import { NAVIGATION, pluginShortcut, shellShortcuts } from "../shortcuts/shell.ts";
import ShortcutHelp from "../shortcuts/ShortcutHelp.vue";
import { runnable } from "../shortcuts/view.ts";
import { closeEditWithPage, editRequest } from "../views/edit/panel.ts";
import { crumbsOf } from "./breadcrumb.ts";
import { detailRequests } from "./detail.ts";
import { keepPanel, readPanel, useSettings, THEMES, DENSITIES } from "./settings.ts";
import { EDITS, locate, pageCount, sidebarGroups, SPACE_ORDER } from "./spaces.ts";

provideLink(KLink);
providePortalTarget("#k-layers");
const toasts = createToasts();
provideToasts(toasts);

const i18n = useI18n();
const { t } = i18n;
const route = useRoute();
const router = useRouter();
const session = useSession();
const settings = useSettings();
const time = useTime();

/** The zone the dates are shown and typed in: the browser's (design 007 « Temps »). */
const ZONE = Intl.DateTimeFormat().resolvedOptions().timeZone;
const PANEL = "k-panel";

// The sidebar: the spaces the rights show, the views of the loaded plugins under the space
// of each (task 16), the entry and space of the current page.
const plugins = usePlugins();
const spaces = computed(() => pluginSpaces(plugins.value));
// « À instruire » counts the proposals of its first page (REQ-021-10), read again on the
// event `changeset`, never by polling.
const proposals = useInfiniteQuery(() => ({
  ...reads.list("/v1/proposals"),
  enabled: session.value.me !== null && session.value.rights.has("core.changeset.read"),
}));
const badges = computed(() => {
  const first = proposals.data.value?.pages[0]?.data;
  return { proposals: pageCount(first?.items.length ?? 0, !!first?.links?.next) };
});
const groups = computed(() => sidebarGroups(session.value.rights, t, spaces.value, badges.value));
const located = computed(() => locate(route.path, spaces.value));
/** Every space but the current one starts closed: the sidebar stays short. */
const closed = computed(() => SPACE_ORDER.filter((id) => id !== located.value?.space.id));

// The breadcrumb, and the title of the document after its last crumb (2.4.2).
const crumbs = computed(() => crumbsOf(route, t));
/** What the status region of the frame says: the page reached, after a navigation only. */
const announced = ref("");
watch(
  crumbs,
  (now) => {
    const last = now.at(-1)?.label;
    document.title = last ? `${last} — ${t("app.name")}` : t("app.name");
  },
  { immediate: true },
);
watch(
  () => route.path,
  () => (announced.value = crumbs.value.at(-1)?.label ?? ""),
);

/** The skip link moves the focus without a history entry. */
function toSurface(): void {
  document.getElementById("k-surface")?.focus();
}

/**
 * A surface that scrolls is reached by Tab and scrolled by the arrows (2.1.1), as the body
 * of a `Popover`: a tab stop while it overflows, in height or in width, whatever its page
 * holds (a view of text alone, a plugin's long list; task 029-5). Measured on the surface
 * and the pages it holds, the ones the router puts in it watched for.
 */
const surface = ref<HTMLElement | null>(null);
const surfaceScrolls = ref(false);
let surfaceSizes: ResizeObserver | undefined;
let surfacePages: MutationObserver | undefined;
onMounted(() => {
  const el = surface.value;
  if (!el || typeof ResizeObserver === "undefined") return;
  const measure = () =>
    (surfaceScrolls.value = el.scrollHeight > el.clientHeight || el.scrollWidth > el.clientWidth);
  surfaceSizes = new ResizeObserver(measure);
  const observe = () => {
    surfaceSizes?.disconnect();
    for (const part of [el, ...el.children]) surfaceSizes?.observe(part);
    measure();
  };
  observe();
  surfacePages = new MutationObserver(observe);
  surfacePages.observe(el, { childList: true });
});
onBeforeUnmount(() => {
  surfaceSizes?.disconnect();
  surfacePages?.disconnect();
});

// The width class of the window, from the same queries as the sheet.
const queries =
  typeof window === "undefined"
    ? []
    : WIDTH_CLASSES.map((c) => ({ id: c.id, list: window.matchMedia(widthQuery(c.id)) }));
const readWidth = (): WidthClass => queries.find((q) => q.list.matches)?.id ?? "wide";
const width = ref<WidthClass>(readWidth());
const onQuery = () => (width.value = readWidth());
for (const q of queries) q.list.addEventListener("change", onQuery);
onBeforeUnmount(() => {
  for (const q of queries) q.list.removeEventListener("change", onQuery);
});

/** Narrow: the sidebar is a rail whatever is kept; wider, it takes the state kept again. */
const collapsed = computed({
  get: () => settings.sidebarCollapsed.value || width.value === "narrow",
  set: (value) => {
    if (width.value !== "narrow") settings.sidebarCollapsed.value = value;
  },
});
/** The drawer of the sidebar (compact class). */
const navOpen = ref(false);
/** The palette asked from the drawer: it opens once the drawer has given the focus back. */
let paletteAfterNav = false;
watch(width, (now) => {
  if (now === "compact") return;
  navOpen.value = false;
  // Out of the compact class, no drawer gives the focus back: no palette waits for it.
  paletteAfterNav = false;
});

// The right panel, as kept (open, tab, width).
const kept = readPanel();
const panelOpen = ref(kept.open);
const panelTab = ref(kept.tab === "draft" ? "draft" : "detail");
/** Its width: the one kept, else the standard width of `SidePanel` (`--k-size-panel-lg`). */
const panelWidth = ref<number>(kept.width ?? 420);
watch([panelOpen, panelTab, panelWidth], ([open, tab, w]) => keepPanel({ open, tab, width: w }));

// The edit panel of a kind (task 12, REQ-021-36): asked by a view, shown in the Détail tab,
// the panel opened on it; its code in its own chunk, read when first asked for.
const EditPanel = defineAsyncComponent(() => import("../views/edit/EditPanel.vue"));
// « Clore… » of a record (021/A31, task 11g): the closure in the same place, its own panel.
const ClosePanel = defineAsyncComponent(() => import("../views/edit/ClosePanel.vue"));
const editing = editRequest();
watch(editing, (request) => {
  if (!request) return;
  panelOpen.value = true;
  panelTab.value = "detail";
});
// The object a route of the administration chose in its list (`meta.detail`, task 15a): in
// the Détail tab, the panel opened on it when another object is chosen, or the same one
// again (`revealDetail`).
const routeDetail = computed(() => route.meta.detail ?? null);
const detailId = computed(() =>
  routeDetail.value && route.params.id ? String(route.params.id) : null,
);
watch(
  [detailId, detailRequests()],
  ([id]) => {
    if (id === null) return;
    panelOpen.value = true;
    panelTab.value = "detail";
  },
  { immediate: true },
);
// Another page, or another date off the present: the edit asked by the page left goes with it.
closeEditWithPage(
  () => route.path,
  () => time.value,
);

// The drafts of the user (spec 024 § A): the list for the menu of the counter, the active
// one of the tab for its count.
const drafting = computed(() => session.value.me !== null && session.value.rights.hasAny(EDITS));
const draftList = useQuery(() => ({ ...draftStore.list(), enabled: drafting.value }));
const activeDraft = ref<string | null>(draftStore.active());
const stopActive = draftStore.onActive((id) => (activeDraft.value = id));
onBeforeUnmount(stopActive);
const userDrafts = computed<DraftList["items"]>(() => draftList.data.value?.data.items ?? []);
const draftCount = computed(
  () => userDrafts.value.find((d) => d.id === activeDraft.value)?.operation_count ?? 0,
);
const draftMenu = computed<DraftSummary[] | undefined>(() =>
  userDrafts.value.length === 0
    ? undefined
    : userDrafts.value.map((d) => ({
        id: d.id,
        label: d.title,
        count: d.operation_count,
        ...(d.id === activeDraft.value ? { current: true } : {}),
      })),
);
const tabs = computed<TabItem[]>(() => [
  { value: "detail", label: t("app.detail") },
  { value: "draft", label: t("app.draft"), count: draftCount.value },
]);
/** The counter opens the panel on the draft, or closes it, whatever its tab. */
function toggleDraft(): void {
  if (panelOpen.value) panelOpen.value = false;
  else {
    panelOpen.value = true;
    panelTab.value = "draft";
  }
}

// The draft tab (task 14, REQ-021-6, 11): its operations and its foot, each read when the
// tab first shows them; the title of a draft the edits create, in the language of the
// shell; a draft that left the tab (deleted, submitted, taken over elsewhere) said once.
// The draft on its page (`/drafts/:id`) is not shown a second time in the panel.
const DraftOperations = defineAsyncComponent(() => import("../draft/DraftOperations.vue"));
const DraftSubmit = defineAsyncComponent(() => import("../draft/DraftSubmit.vue"));
const draftOnPage = computed(
  () =>
    activeDraft.value !== null && route.path === `/drafts/${encodeURIComponent(activeDraft.value)}`,
);
const stamp = computed(
  () => new Intl.DateTimeFormat(i18n.locale.value, { dateStyle: "short", timeStyle: "short" }),
);
setDraftTitle(() => t("drafts.newTitle", { when: stamp.value.format(new Date()) }));
// A plugin's `draft.add` opens the draft, as its contract promises (the shell's own edits
// go through `addToActiveDraft` and say themselves where they were made).
draftHost.onAdd(() => openDraft());
onBeforeUnmount(() => {
  setDraftTitle(null);
  draftHost.onAdd(null);
});
onBeforeUnmount(
  draftStore.onGone((gone) => {
    // A write refused 404 says it where it was asked (its codes there); the stream, here.
    if (gone.active && gone.cause === "event") {
      toasts.show({ tone: "info", title: t("drafts.gone") });
    }
  }),
);

// The dated context: the date pill applies a period once the API reads at it (its refusal
// back on the field, a new object each time), the band goes back to the present and gives
// the focus to the pill (2.4.3).
const pill = ref<{ focus(): void } | null>(null);
const refusal = shallowRef<{ errors: Partial<Record<keyof Period, string>>; problem: unknown }>();
const { sentence } = useProblems();
/** The period asked last: the answer to an earlier one is dropped. */
let asked = 0;
async function applyPeriod(period: Period): Promise<void> {
  const now = time.value;
  const ask = ++asked;
  if (period.validAt === now.validAt && period.asOf === now.asOf) return;
  const path = probePath((permission) => session.value.rights.has(permission));
  const refused = await checkPeriod(period, path);
  if (ask !== asked) return;
  if (refused) {
    refusal.value = { errors: refusalOnField(refused, t, sentence), problem: refused.problem };
    return;
  }
  refusal.value = undefined;
  await goToPeriod(router, period);
}
async function backToPresent(): Promise<void> {
  asked++;
  await toPresent(router);
  await nextTick();
  pill.value?.focus();
}

// The user menu: language, theme, density, motion, single-key shortcuts.
const userName = computed(() => session.value.me?.name ?? session.value.me?.identity ?? null);
const userMenu = computed<MenuItem[]>(() => {
  const locale = i18n.locale.value as Locale;
  const themeLabel = { system: "themeSystem", light: "themeLight", dark: "themeDark" } as const;
  const densityLabel = { compact: "densityCompact", comfortable: "densityComfortable" } as const;
  return [
    { kind: "label", label: userName.value ?? t("user.anonymous") },
    { kind: "separator" },
    { kind: "label", label: t("user.language") },
    ...LOCALES.map((l) => ({
      kind: "radio" as const,
      id: `locale:${l}`,
      label: LOCALE_NAMES[l],
      checked: l === locale,
    })),
    { kind: "label", label: t("user.theme") },
    ...THEMES.map((theme) => ({
      kind: "radio" as const,
      id: `theme:${theme}`,
      label: t(`user.${themeLabel[theme]}`),
      checked: theme === settings.theme.value,
    })),
    { kind: "label", label: t("user.density") },
    ...DENSITIES.map((density) => ({
      kind: "radio" as const,
      id: `density:${density}`,
      label: t(`user.${densityLabel[density]}`),
      checked: density === settings.density.value,
    })),
    { kind: "separator" },
    {
      kind: "checkbox",
      id: "motion",
      label: t("user.reducedMotion"),
      checked: settings.motion.value === "reduce",
    },
    {
      kind: "checkbox",
      id: "single-keys",
      label: t("user.singleKeys"),
      checked: settings.singleKeys.value,
    },
    { id: "help", label: t("palette.help"), icon: "keyboard", ...hint(["?"]) },
    { id: "about", label: t("about.title"), icon: "info" },
    { kind: "separator" },
    session.value.me
      ? { id: "logout", label: t("user.logout"), icon: "log-out" }
      : { id: "login", label: t("user.login"), icon: "user" },
  ];
});
function onUserMenu(id: string): void {
  const [kind, value] = id.split(":") as [string, string | undefined];
  if (kind === "locale") changeLocale(i18n, value as Locale);
  else if (kind === "theme") settings.theme.value = value as (typeof THEMES)[number];
  else if (kind === "density") settings.density.value = value as (typeof DENSITIES)[number];
  else if (kind === "motion")
    settings.motion.value = settings.motion.value === "reduce" ? "system" : "reduce";
  else if (kind === "single-keys") settings.singleKeys.value = !settings.singleKeys.value;
  else if (kind === "help") helpOpen.value = true;
  else if (kind === "about") void router.push("/about");
  else if (kind === "login")
    void router.push({ path: "/login", query: { return: route.fullPath } });
  else if (kind === "logout") logout();
}

/** The hint of a single-key shortcut, shown only while those are on (WCAG 2.1.4). */
function hint(keys: Shortcut): { shortcut?: Shortcut } {
  return settings.singleKeys.value ? { shortcut: keys } : {};
}
const atPresent = computed(() => time.value.validAt === null && time.value.asOf === null);

// The palette (REQ-021-3), opened by Ctrl+K or « Rechercher » of the sidebar (from the
// drawer of the compact class, once it has closed and given the focus back to its button).
// A text searches the objects by value and by name, 150 ms after the last key; a new text
// or the palette closed aborts the search under way. The objects found stay until the
// next answer replaces them: Enter never changes its target with the network.
const paletteOpen = ref(false);
const paletteQuery = ref("");
const found = shallowRef<FoundObject[]>([]);
const searching = ref(false);
const recent = shallowRef<FoundObject[]>([]);
let searchTimer: ReturnType<typeof setTimeout> | undefined;
let search: AbortController | null = null;
const SEARCH_WAIT = 150;
const has = (permission: string) => session.value.rights.has(permission);
/** The identity of the user: the key of their recent objects (none signed out). */
const owner = () => session.value.me?.identity ?? null;
function stopSearch(): void {
  clearTimeout(searchTimer);
  search?.abort();
  search = null;
}
watch(paletteQuery, (text) => {
  stopSearch();
  searching.value = text.trim() !== "";
  if (!searching.value) return;
  const controller = new AbortController();
  search = controller;
  searchTimer = setTimeout(async () => {
    const result = await searchObjects(text, has, controller.signal);
    if (controller.signal.aborted) return;
    found.value = result;
    searching.value = false;
  }, SEARCH_WAIT);
});
watch(paletteOpen, (open) => {
  if (open) return;
  stopSearch();
  searching.value = false;
  found.value = [];
});
onBeforeUnmount(stopSearch);

// The commands of the palette: those of the view (`c`, `e`), the draft, the present, the
// frame, the help, then the plugins' (absent without their permission).
const pluginCommands = shallowRef(commandRegistry.list());
onBeforeUnmount(commandRegistry.subscribe(() => (pluginCommands.value = commandRegistry.list())));
const commands = computed<PaletteEntry[]>(() => {
  const list: PaletteEntry[] = [];
  const create = runnable("create", time.value);
  if (create)
    list.push({
      id: "cmd:create",
      label: create.label(),
      icon: "plus",
      ...hint(["c"]),
      run: create.run,
    });
  const edit = runnable("edit", time.value);
  if (edit)
    list.push({
      id: "cmd:edit",
      label: edit.label(),
      icon: "pencil",
      ...hint(["e"]),
      run: edit.run,
    });
  if (drafting.value) {
    list.push({ id: "cmd:draft", label: t("palette.openDraft"), icon: "draft", run: openDraft });
  }
  if (!atPresent.value) {
    list.push({
      id: "cmd:present",
      label: t("palette.present"),
      icon: "rotate-ccw-clock",
      run: () => void backToPresent(),
    });
  }
  if (width.value !== "narrow") {
    const sidebarLabel =
      width.value === "compact" || collapsed.value
        ? "palette.expandSidebar"
        : "palette.collapseSidebar";
    list.push({
      id: "cmd:sidebar",
      label: t(sidebarLabel),
      icon: "panel-left",
      ...hint(["["]),
      run: toggleSidebar,
    });
  }
  list.push({
    id: "cmd:panel",
    label: t(panelOpen.value ? "palette.closePanel" : "palette.openPanel"),
    icon: "move-horizontal",
    ...hint(["]"]),
    run: togglePanel,
  });
  list.push({
    id: "cmd:help",
    label: t("palette.help"),
    icon: "keyboard",
    ...hint(["?"]),
    run: () => (helpOpen.value = true),
  });
  list.push(...pluginEntries(pluginCommands.value, (p) => session.value.rights.has(p)));
  return list;
});
/** The `g` letter of each entry of the sidebar: the shell's, and those granted to plugins. */
const LETTERS = new Map(NAVIGATION.map(({ letter, entry }) => [entry, letter]));
const grantedLetters = shallowRef<ReadonlyMap<string, string>>(new Map());
const pages = computed<PaletteEntry[]>(() =>
  groups.value.flatMap((group) =>
    group.entries.map((entry) => {
      const letter = LETTERS.get(entry.id) ?? grantedLetters.value.get(entry.id);
      return {
        id: `page:${entry.id}`,
        label: entry.label,
        icon: entry.icon,
        keywords: [group.label],
        ...(letter ? hint([["g"], [letter]]) : {}),
        run: () => void router.push(entry.to),
      };
    }),
  ),
);
const paletteGroups = computed<PaletteGroup[]>(() =>
  groupsOfPalette({
    query: paletteQuery.value,
    objects: found.value,
    recent: recent.value,
    commands: commands.value,
    pages: pages.value,
    labels: {
      objects: t("palette.objects"),
      recent: t("palette.recent"),
      commands: t("palette.commands"),
      navigation: t("palette.navigation"),
    },
    typeLabel: (type) => t(`palette.type.${type}`),
  }),
);
/** What had the focus when the palette opened: where it goes back before a command runs. */
let paletteOpener: HTMLElement | null = null;
/**
 * Ctrl+K, « Rechercher »: the palette, on the recent objects. Not over another modal layer
 * (a dialog, a confirmation): two modal layers would fight for the focus, and the one
 * below would lose its way back (2.4.3). The drawer of the compact class gives way to it.
 */
function openPalette(): void {
  if (paletteOpen.value) return;
  const at = document.activeElement;
  // The palette itself, closing, may still hold the focus during its exit.
  if (!navOpen.value && at?.closest("[aria-modal='true']:not(.k-palette)")) return;
  dropPending();
  paletteQuery.value = "";
  recent.value = recentObjects(owner(), (type) => readableType(type, has));
  if (!navOpen.value) {
    paletteOpener =
      at instanceof HTMLElement && at !== document.body && !at.closest(".k-palette") ? at : null;
    paletteOpen.value = true;
    return;
  }
  paletteAfterNav = true;
  navOpen.value = false;
}
function onNavFocus(event: FocusEvent): void {
  if (!paletteAfterNav) return;
  paletteAfterNav = false;
  paletteOpener = event.target instanceof HTMLElement ? event.target : null;
  paletteOpen.value = true;
}
/** The wait for the focus back on the opener, past the exit of the palette. */
const FOCUS_BACK_WAIT = 1000;
/** The command waiting for the focus back on the opener, and how to drop it. */
let dropPending: () => void = () => {};
/**
 * Runs `run` once the palette has given the focus back to what opened it, so that a layer
 * the command opens (the help) gives it back there in turn; at once without an opener. The
 * command never runs on an unrelated focus: one elsewhere, the palette opened again, or no
 * focus back in time drops it.
 */
function afterPalette(run: () => void): void {
  dropPending();
  const opener = paletteOpener;
  paletteOpener = null;
  if (!opener?.isConnected) {
    paletteOpen.value = false;
    void nextTick(run);
    return;
  }
  const back = (event: FocusEvent) => {
    if (event.target instanceof Element && event.target.closest(".k-palette")) return;
    dropPending();
    if (event.target === opener) run();
  };
  const timer = setTimeout(() => dropPending(), FOCUS_BACK_WAIT);
  dropPending = () => {
    clearTimeout(timer);
    document.removeEventListener("focusin", back, true);
    dropPending = () => {};
  };
  document.addEventListener("focusin", back, true);
  paletteOpen.value = false;
}
onBeforeUnmount(() => dropPending());
function onPaletteSelect(id: string): void {
  const object = [...found.value, ...recent.value].find((o) => objectEntry(o) === id);
  if (object) {
    rememberObject(owner(), object);
    paletteOpen.value = false;
    void router.push(object.to);
    return;
  }
  const entry = [...commands.value, ...pages.value].find((e) => e.id === id);
  if (entry) afterPalette(entry.run);
  else paletteOpen.value = false;
}

/** The draft in front in the right panel. */
function openDraft(): void {
  panelOpen.value = true;
  panelTab.value = "draft";
}
function togglePanel(): void {
  panelOpen.value = !panelOpen.value;
}
/** `[`: the sidebar collapsed or expanded; its drawer in the compact class; a rail when narrow. */
function toggleSidebar(): boolean {
  if (width.value === "narrow") return false;
  if (width.value === "compact") navOpen.value = !navOpen.value;
  else collapsed.value = !collapsed.value;
  return true;
}

// The help of the shortcuts `?` (and the page /help/shortcuts).
const helpOpen = ref(false);

// The shortcuts (REQ-021-7, 8): the shell's, then those of the plugin's view (the keys of
// the shell refused to them since `main.ts`). One listener on the document.
const registry = createShortcutRegistry();
for (const shortcut of shellShortcuts({
  t,
  rights: () => session.value.rights,
  navigate: (to) => void router.push(to),
  palette: openPalette,
  help: () => {
    helpOpen.value = true;
  },
  filter: () => playView("filter"),
  create: () => playView("create"),
  edit: () => playView("edit"),
  sidebar: toggleSidebar,
  panel: togglePanel,
})) {
  registry.register(shortcut);
}
/** An action of the view behind `/`, `c`, `e`; `false` when it has none or cannot now. */
function playView(id: "filter" | "create" | "edit"): boolean {
  const action = runnable(id, time.value);
  action?.run();
  return action !== null;
}
const shellList = shallowRef(registry.list());
onBeforeUnmount(registry.subscribe(() => (shellList.value = registry.list())));

// The `g` letters of the views of the loaded plugins (design « API des plugins »), in the
// order of `ui_plugins`, registered again at each change of the plugins: a letter taken (by
// the shell or a plugin before) is refused, said in the console and in « À propos », and
// the plugin stays loaded.
let dropLetters = () => {};
watch(
  plugins,
  (loaded) => {
    dropLetters();
    const letters = registerLetters(registry, pluginLetters(loaded), {
      t,
      rights: () => session.value.rights,
      navigate: (to) => void router.push(to),
    });
    dropLetters = letters.drop;
    grantedLetters.value = letters.granted;
    refuseLetters(letters.refused);
  },
  { immediate: true },
);
onBeforeUnmount(() => dropLetters());

const pluginShortcuts = shallowRef(shortcutRegistry.list());
onBeforeUnmount(
  shortcutRegistry.subscribe(() => (pluginShortcuts.value = shortcutRegistry.list())),
);
const shortcuts = computed(() => [
  ...shellList.value,
  ...pluginShortcuts.value.map(pluginShortcut),
]);
provideShortcuts(shortcuts);
const onKeydown = createKeyListener({
  shortcuts: () => shortcuts.value,
  singleKeys: () => settings.singleKeys.value,
});
onMounted(() => document.addEventListener("keydown", onKeydown));
onBeforeUnmount(() => document.removeEventListener("keydown", onKeydown));

// A navigation closes the drawer of the sidebar (the compact class).
watch(
  () => route.fullPath,
  () => (navOpen.value = false),
);

const userLabel = computed(() =>
  userName.value ? t("user.menu", { name: userName.value }) : t("user.anonymousMenu"),
);

// The end of the session, said before it comes (REQ-021-14): a warning that stays until
// the next activity, with « Rester connecté » unless the end is the absolute one.
onBeforeUnmount(
  watchSessionEnd({
    toasts,
    t,
    locale: () => i18n.locale.value,
    end: () => session.value.me?.session?.expires_at,
    absoluteEnd: () => session.value.me?.session?.absolute_expires_at,
    sentence,
  }),
);
</script>

<template>
  <div
    class="k-frame"
    :data-sidebar="collapsed ? 'collapsed' : 'expanded'"
    :data-width="width"
    data-k-modal-scope
  >
    <!-- First stop of the page: past the sidebar and the bar to the surface (WCAG 2.4.1). -->
    <a class="k-frame__skip" href="#k-surface" @click.prevent="toSurface">{{ t("app.skip") }}</a>
    <SidebarNav
      v-model:collapsed="collapsed"
      :brand="t('app.name')"
      :groups="groups"
      :active="located?.entry.id"
      :closed="closed"
      :single-keys="settings.singleKeys.value"
      @palette="openPalette"
    />
    <Drawer
      v-if="width === 'compact'"
      v-model:open="navOpen"
      :title="uiT('sidebar')"
      side="start"
      size="sm"
    >
      <SidebarNav
        :brand="t('app.name')"
        :groups="groups"
        :active="located?.entry.id"
        :closed="closed"
        @palette="openPalette"
      />
    </Drawer>
    <div class="k-frame__column">
      <!-- The banner: the status bar, and under it the band of the dated context. -->
      <header class="k-frame__head">
        <div class="k-frame__bar">
          <IconButton
            class="k-frame__nav"
            icon="menu"
            variant="ghost"
            :label="uiT('sidebar')"
            aria-haspopup="dialog"
            :aria-expanded="navOpen ? 'true' : 'false'"
            :shortcut="settings.singleKeys.value ? ['['] : undefined"
            @click="navOpen = true"
            @focus="onNavFocus"
          />
          <Breadcrumb :items="crumbs" class="k-frame__crumbs" />
          <div class="k-frame__bar-end">
            <DatePill
              ref="pill"
              :valid-at="time.validAt"
              :as-of="time.asOf"
              :time-zone="ZONE"
              :errors="refusal?.errors"
              :problem="problemOf(refusal?.problem)"
              @apply="applyPeriod"
            />
            <DraftCounter
              :count="draftCount"
              :expanded="panelOpen"
              :controls="panelOpen ? PANEL : undefined"
              :drafts="draftMenu"
              @toggle="toggleDraft"
              @select="draftStore.setActive"
            />
            <Menu
              :label="userLabel"
              icon="user"
              icon-only
              variant="ghost"
              align="end"
              :items="userMenu"
              @select="onUserMenu"
            />
          </div>
        </div>
        <TimeBand
          :valid-at="time.validAt"
          :as-of="time.asOf"
          :time-zone="ZONE"
          @present="backToPresent"
        />
      </header>
      <div class="k-frame__body">
        <main
          id="k-surface"
          ref="surface"
          class="k-frame__surface"
          :tabindex="surfaceScrolls ? 0 : -1"
        >
          <RouterView />
        </main>
        <SidePanel
          :id="PANEL"
          v-model:open="panelOpen"
          v-model:tab="panelTab"
          v-model:width="panelWidth"
          :label="t('app.rightPanel')"
          :tabs="tabs"
          :cover="width === 'narrow'"
          :sheet="width === 'compact'"
          :rail="width !== 'compact'"
          :shortcut="settings.singleKeys.value ? [']'] : undefined"
        >
          <template #detail>
            <ClosePanel v-if="editing?.mode === 'close'" :key="editing.serial" :request="editing" />
            <EditPanel v-else-if="editing" :key="editing.serial" :request="editing" />
            <component
              :is="routeDetail"
              v-else-if="routeDetail && detailId"
              :id="detailId"
              :key="detailId"
            />
            <p v-else class="k-frame__muted">{{ t("app.noSelection") }}</p>
          </template>
          <template #draft>
            <p v-if="draftOnPage" class="k-frame__muted">{{ t("drafts.onPage") }}</p>
            <DraftOperations v-else :id="activeDraft" />
          </template>
          <template #footer="{ tab: shown }">
            <DraftSubmit
              v-if="shown === 'draft' && activeDraft && !draftOnPage"
              :id="activeDraft"
            />
          </template>
        </SidePanel>
      </div>
    </div>
    <CommandPalette
      v-model:open="paletteOpen"
      v-model:query="paletteQuery"
      :groups="paletteGroups"
      :loading="searching"
      @select="onPaletteSelect"
    />
    <Dialog v-model:open="helpOpen" size="lg" :title="t('shortcuts.title')">
      <ShortcutHelp :shortcuts="shortcuts" :single-keys="settings.singleKeys.value" />
    </Dialog>
    <SessionDialog />
    <ToastRegion />
    <!-- The page reached, said once its title is set (4.1.3): the focus stays where it is. -->
    <p class="k-visually-hidden" role="status">{{ announced }}</p>
    <div id="k-layers" class="k-frame__layers"></div>
  </div>
</template>

<style>
@layer components {
  /* The slots: sidebar, then a column of bar, band and body, at the height of the window
   * (dynamic where supported). The window never scrolls. The geometry is that of the
   * skeleton of index.html (boot.css), which the frame replaces without a shift. */
  .k-frame {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    block-size: var(--k-size-viewport);
    min-block-size: var(--k-size-viewport);
    overflow: hidden;
    color: var(--k-text);
    background-color: var(--k-bg);
  }

  /* The skip link: out of the grid, shown over the corner while it has the focus. */
  .k-frame__skip {
    position: absolute;
    inset-block-start: var(--k-space-1);
    inset-inline-start: var(--k-space-1);
    z-index: var(--k-z-overlay);
    padding: var(--k-space-1) var(--k-space-2);
    color: var(--k-text);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
    translate: 0 calc(-100% - var(--k-space-2));
  }

  .k-frame__skip:focus {
    translate: 0 0;
  }

  .k-frame__surface:focus {
    outline: none;
  }

  /* A tab stop while it scrolls: its ring, inside its edge (the body clips what passes it). */
  .k-frame__surface[tabindex="0"]:focus-visible {
    outline: var(--k-focus-width) solid var(--k-focus-ring);
    outline-offset: calc(-1 * var(--k-focus-width));
  }

  /* The button of the drawer of the sidebar: the compact class only. */
  .k-frame__nav {
    display: none;
  }

  /* In the drawer, the sidebar takes its body; no collapse there. */
  .k-drawer .k-sidebar {
    inline-size: 100%;
    padding: 0;
    border-inline-end: 0;
  }

  .k-drawer .k-sidebar__collapse {
    display: none;
  }

  .k-frame > .k-sidebar {
    min-block-size: 0;
    overflow: auto;
  }

  .k-frame__column {
    position: relative;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    grid-template-columns: minmax(0, 1fr);
    min-inline-size: 0;
    min-block-size: 0;
  }

  .k-frame__head {
    min-inline-size: 0;
  }

  .k-frame__bar {
    display: flex;
    gap: var(--k-space-4);
    align-items: center;
    justify-content: space-between;
    min-inline-size: 0;
    min-block-size: var(--k-size-status-bar);
    padding-inline: var(--k-space-4) var(--k-space-2);
    background-color: var(--k-surface);
    border-block-end: var(--k-border-width) solid var(--k-border-strong);
  }

  /* The breadcrumb gives way to the controls at the end of the bar. */
  /* Clipped, but not the focus ring of a crumb (2.4.7). */
  .k-frame__crumbs {
    min-inline-size: 0;
    overflow: clip;
    overflow-clip-margin: var(--k-space-1);
  }

  .k-frame__bar-end {
    display: flex;
    flex: none;
    gap: var(--k-space-2);
    align-items: center;
  }

  /* The surface, then the right panel beside it or the strip of the closed panel, in the
   * `auto` track; the panel bounds its width by this container and covers it from here at
   * the narrow class. */
  .k-frame__body {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    min-block-size: 0;
    container-type: inline-size;
  }

  .k-frame__surface {
    display: grid;
    gap: var(--k-space-3);
    align-content: start;
    min-inline-size: 0;
    min-block-size: 0;
    padding: var(--k-space-3);
    overflow: auto;
    isolation: isolate;
    container: k-surface / inline-size;
  }

  .k-frame__layers {
    display: contents;
  }

  .k-frame__muted {
    color: var(--k-text-muted);
  }

  /* The width classes (WIDTH_CLASSES of @kartenn/ui, equal by test): compact under 600 px,
   * narrow from 600, standard from 1024, wide from 1280. */

  /* Under wide: the zone of the date pill stays in its name only, room for the path. */
  @media (width < 1280px) {
    .k-frame__bar .k-date-pill__zone {
      position: absolute;
      inline-size: var(--k-space-px);
      block-size: var(--k-space-px);
      overflow: hidden;
      clip-path: inset(50%);
      white-space: nowrap;
    }
  }

  /* Narrow and compact: a tighter margin around the surface. */
  @media (width < 1024px) {
    .k-frame__surface {
      gap: var(--k-space-2);
      padding: var(--k-space-2);
    }
  }

  /* Compact: one column; the sidebar in its drawer, opened from the bar; a reduced bar. */
  @media (width < 600px) {
    .k-frame {
      grid-template-columns: minmax(0, 1fr);
    }

    .k-frame > .k-sidebar {
      display: none;
    }

    .k-frame__nav {
      display: inline-flex;
      flex: none;
    }

    .k-frame__bar {
      gap: var(--k-space-1);
      padding-inline: var(--k-space-1);
    }

    .k-frame__bar-end {
      gap: var(--k-space-1);
    }
  }
}
</style>
