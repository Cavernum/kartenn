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
// Left to the next tasks: the shortcut registry, `[`, `]`, Ctrl+K and the full palette
// (task 10; here « Rechercher » opens it on the navigation); the content of the draft tab
// (task 14); the counters of the sidebar (tasks 11, 14), the entries of the plugins and the
// session's login, about and logout (task 16).
import { drafts as draftStore, type DraftList } from "@kartenn/sdk";
import {
  Breadcrumb,
  CommandPalette,
  DatePill,
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
  type TabItem,
  type WidthClass,
} from "@kartenn/ui";
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { RouterView, useRoute, useRouter } from "vue-router";
import { useQuery } from "../data/query.ts";
import { changeLocale, LOCALE_NAMES, LOCALES, type Locale } from "../i18n/index.ts";
import KLink from "../prefetch/KLink.vue";
import { useSession } from "../rights/index.ts";
import {
  checkPeriod,
  goToPeriod,
  toPresent,
  useTime,
  type Period,
  type PeriodRefusal,
} from "../time/index.ts";
import { crumbsOf } from "./breadcrumb.ts";
import { keepPanel, readPanel, useSettings, THEMES, DENSITIES } from "./settings.ts";
import { EDITS, locate, sidebarGroups, SPACE_ORDER } from "./spaces.ts";

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

// The sidebar: the spaces the rights show, the entry and space of the current page.
const groups = computed(() => sidebarGroups(session.value.rights, t));
const located = computed(() => locate(route.path));
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

// The dated context: the date pill applies a period once the API reads at it (its refusal
// back on the field, a new object each time), the band goes back to the present and gives
// the focus to the pill (2.4.3).
const pill = ref<{ focus(): void } | null>(null);
const refusal = ref<PeriodRefusal>();
async function applyPeriod(period: Period): Promise<void> {
  const now = time.value;
  if (period.validAt === now.validAt && period.asOf === now.asOf) return;
  const refused = await checkPeriod(period);
  if (refused) {
    refusal.value = Object.fromEntries(
      Object.entries(refused).map(([field, reason]) => [
        field,
        field === "asOf" && /later than the time/.test(reason)
          ? t("time.asOfLater")
          : t("time.refused", { reason }),
      ]),
    );
    return;
  }
  refusal.value = undefined;
  await goToPeriod(router, period);
}
async function backToPresent(): Promise<void> {
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
}

// The palette, opened by « Rechercher » of the sidebar (from the drawer of the compact class,
// once it has closed and given the focus back to its button), on the navigation for now.
const paletteOpen = ref(false);
const paletteQuery = ref("");
const paletteGroups = computed<PaletteGroup[]>(() => {
  const wanted = paletteQuery.value.trim().toLowerCase();
  const items = groups.value
    .flatMap((group) => group.entries)
    .filter((entry) => entry.label.toLowerCase().includes(wanted))
    .map((entry) => ({ id: entry.id, label: entry.label, icon: entry.icon }));
  return items.length === 0 ? [] : [{ label: t("palette.navigation"), items }];
});
function openPalette(): void {
  paletteQuery.value = "";
  if (!navOpen.value) {
    paletteOpen.value = true;
    return;
  }
  paletteAfterNav = true;
  navOpen.value = false;
}
function onNavFocus(): void {
  if (!paletteAfterNav) return;
  paletteAfterNav = false;
  paletteOpen.value = true;
}
function onPaletteSelect(id: string): void {
  const entry = groups.value.flatMap((group) => group.entries).find((e) => e.id === id);
  paletteOpen.value = false;
  if (entry) void router.push(entry.to);
}

// A navigation closes the drawer of the sidebar (the compact class).
watch(
  () => route.fullPath,
  () => (navOpen.value = false),
);

const userLabel = computed(() =>
  userName.value ? t("user.menu", { name: userName.value }) : t("user.anonymousMenu"),
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
              :errors="refusal"
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
        <main id="k-surface" class="k-frame__surface" tabindex="-1">
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
        >
          <template #detail>
            <p class="k-frame__muted">{{ t("app.noSelection") }}</p>
          </template>
          <template #draft>
            <p class="k-frame__muted">
              {{ draftCount === 0 ? t("app.draftEmpty") : t("app.draftCount", draftCount) }}
            </p>
          </template>
        </SidePanel>
      </div>
    </div>
    <CommandPalette
      v-model:open="paletteOpen"
      v-model:query="paletteQuery"
      :groups="paletteGroups"
      @select="onPaletteSelect"
    />
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
