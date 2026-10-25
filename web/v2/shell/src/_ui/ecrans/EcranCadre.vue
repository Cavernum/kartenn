<script setup lang="ts">
// The frame of the shell for the mock-ups (spec 021, « Cadre », revised by the 024: M8,
// M15): fixed slots laid by its grid — sidebar of spaces (collapsible), status bar
// (breadcrumb, date pill, draft counter, user menu), band out of the present, work
// surface, right panel (SidePanel) over the end of the surface. Each slot scrolls inside
// itself; the page never does. The surface is an isolated stacking context: its sticky
// headers never pass over the panel nor over a floating layer. The draft counter opens
// the panel on its draft tab, or closes it when open (its icon is then a cross); closed,
// the panel leaves its strip at the end of the body, whose chevron reopens it. The real
// frame is a task of the 021; here it only places components of @kartenn/ui (layout, no
// look of its own).
//
// Adaptation to the width (spec 020, « Adaptation à la largeur »): the frame alone reads
// the window, in four classes (`WIDTH_CLASSES`, copied in the media queries below, equal
// by test). Wide (≥ 1280) and standard (≥ 1024): sidebar expanded, the panel beside the
// surface while the surface keeps `--k-size-surface-min` (a query on the body), over its
// end otherwise. Narrow (≥ 600): sidebar as a rail, the panel covers the surface.
// Compact: one column, the sidebar in a drawer opened from the status bar, a reduced bar
// (date pill and draft counter as icons), the panel a sheet over the window with a back
// button. CSS lays all this out; the script keeps what CSS cannot: `data-width` on the
// root, the state of the sidebar, the kind of panel, the drawer and the focus.
import {
  Breadcrumb,
  DatePill,
  DraftCounter,
  Drawer,
  IconButton,
  Menu,
  SidePanel,
  SidebarNav,
  TimeBand,
  WIDTH_CLASSES,
  applyMotion,
  currentMotion,
  t,
  widthQuery,
  type Crumb,
  type MotionSetting,
  type DraftSummary,
  type MenuItem,
  type TabItem,
  type WidthClass,
} from "@kartenn/ui";
import { computed, onBeforeUnmount, ref, useSlots, watch } from "vue";
import Brouillon from "./EcranBrouillon.vue";
import Soumission from "./EcranSoumission.vue";
import { DRAFTS } from "./donnees.ts";
import { SPACES, closedBut } from "./sidebar.ts";

const props = withDefaults(
  defineProps<{
    /** The title of the page: the h1, hidden (the panels show it). */
    title: string;
    /** The active entry of the sidebar, and the space that holds it. */
    active: string;
    space: string;
    crumbs: Crumb[];
    /** The instant read, out of the present (band and date pill). */
    at?: string | undefined;
    /** The date pill open on its picker (slot `picker`, laid over the surface). */
    picker?: boolean;
    /** The operations of the active draft. */
    draft?: number;
    /** The right panel open on this tab at first; closed when unset. */
    panel?: "detail" | "draft" | undefined;
    /** Former name of `panel="draft"`. */
    draftOpen?: boolean;
    /** The menu of the drafts open (chevron of the counter). */
    draftsOpen?: boolean;
    collapsed?: boolean;
  }>(),
  { at: undefined, draft: 3, panel: undefined },
);
const slots = useSlots();

const ZONE = "Europe/Paris";
const PANEL = "k-ecran-panel";

// « Mouvement » toggles between the system's preference and reduced motion (SCR40): the
// one entry of the mock-up that acts, on the root, as the real frame will.
const motion = ref<MotionSetting>(typeof document === "undefined" ? "system" : currentMotion());
const userMenu = computed<MenuItem[]>(() => [
  { kind: "label", label: "Camille Martin" },
  { id: "lang", label: "Langue : français", icon: "languages" },
  { id: "theme", label: "Thème : celui du système", icon: "monitor" },
  { id: "density", label: "Densité : compacte", icon: "layers" },
  {
    id: "motion",
    label: motion.value === "reduce" ? "Mouvement : réduit" : "Mouvement : celui du système",
    icon: "move",
  },
  { id: "keys", label: "Raccourcis à une touche : activés", icon: "keyboard" },
  { id: "help", label: "Raccourcis", icon: "circle-question-mark", shortcut: ["?"] },
  { kind: "separator" },
  { id: "about", label: "À propos", icon: "info" },
  { id: "logout", label: "Se déconnecter", icon: "log-out" },
]);
function onUserMenu(id: string): void {
  if (id !== "motion") return;
  motion.value = motion.value === "reduce" ? "system" : "reduce";
  applyMotion(motion.value);
}
const drafts: DraftSummary[] = DRAFTS.map((draft, i) => ({
  id: draft.id,
  label: draft.title,
  count: i === 0 ? props.draft : draft.operations,
  ...(i === 0 ? { current: true } : {}),
}));

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

// Narrow: the sidebar is a rail; back to a wider class, it takes the state asked again.
const collapsed = ref(props.collapsed || width.value === "narrow");
watch(width, (now) => {
  collapsed.value = props.collapsed || now === "narrow";
  if (now !== "compact") navOpen.value = false;
});
/** The drawer of the sidebar (compact class). */
const navOpen = ref(false);
/** The panel is a sheet over the window: what it hides leaves the focus order. */
const sheetOpen = computed(() => width.value === "compact" && open.value);
const initial = props.draftOpen ? "draft" : props.panel;
const open = ref(initial !== undefined);
const tab = ref<string>(initial ?? "detail");
const tabs = computed<TabItem[]>(() => [
  { value: "detail", label: "Détail" },
  { value: "draft", label: "Brouillon", count: props.draft },
]);
/** The counter opens the panel on the draft, or closes it, whatever its tab. */
function toggleDraft(): void {
  if (open.value) open.value = false;
  else {
    open.value = true;
    tab.value = "draft";
  }
}
</script>

<template>
  <div class="k-ecran" :data-sidebar="collapsed ? 'collapsed' : 'expanded'" :data-width="width">
    <SidebarNav
      v-model:collapsed="collapsed"
      brand="kartenn"
      :groups="SPACES"
      :active="props.active"
      :closed="closedBut(props.space)"
    />
    <Drawer
      v-if="width === 'compact'"
      v-model:open="navOpen"
      :title="t('sidebar')"
      side="start"
      size="sm"
    >
      <SidebarNav
        brand="kartenn"
        :groups="SPACES"
        :active="props.active"
        :closed="closedBut(props.space)"
      />
    </Drawer>
    <div class="k-ecran__column">
      <header class="k-ecran__bar" :inert="sheetOpen || undefined">
        <IconButton
          class="k-ecran__nav"
          icon="menu"
          variant="ghost"
          :label="t('sidebar')"
          aria-haspopup="dialog"
          :aria-expanded="navOpen ? 'true' : 'false'"
          @click="navOpen = true"
        />
        <Breadcrumb :items="props.crumbs" class="k-ecran__crumbs" />
        <div class="k-ecran__bar-end">
          <DatePill
            :at="props.at ?? null"
            :time-zone="ZONE"
            :expanded="props.picker"
            :aria-controls="slots.picker ? 'k-ecran-picker' : undefined"
          />
          <DraftCounter
            :count="props.draft"
            :expanded="open"
            :controls="open ? PANEL : undefined"
            :drafts="drafts"
            :default-open="props.draftsOpen"
            @toggle="toggleDraft"
          />
          <Menu
            label="Compte de Camille Martin"
            icon="user"
            icon-only
            variant="ghost"
            align="end"
            :items="userMenu"
            @select="onUserMenu"
          />
        </div>
      </header>
      <TimeBand v-if="props.at" :at="props.at" :time-zone="ZONE" :inert="sheetOpen || undefined" />
      <div class="k-ecran__body">
        <main class="k-ecran__surface" :inert="sheetOpen || undefined">
          <h1 class="k-visually-hidden">{{ props.title }}</h1>
          <slot />
        </main>
        <slot v-if="slots.drawer" name="drawer" />
        <SidePanel
          v-else
          :id="PANEL"
          v-model:open="open"
          v-model:tab="tab"
          label="Panneau droit"
          :tabs="tabs"
          :cover="width === 'narrow'"
          :sheet="width === 'compact'"
          rail
        >
          <template #detail>
            <slot name="detail">
              <p class="k-ecran-muted">
                Aucune sélection : choisir une ligne ou un nœud en montre ici le détail.
              </p>
            </slot>
          </template>
          <template #draft>
            <slot name="draft"><Brouillon :count="props.draft" /></slot>
          </template>
          <template #footer="{ tab: current }">
            <slot v-if="current === 'draft'" name="draft-footer">
              <Soumission />
            </slot>
            <slot v-else name="detail-footer" />
          </template>
        </SidePanel>
      </div>
      <div v-if="slots.picker" id="k-ecran-picker" class="k-ecran__picker">
        <slot name="picker" />
      </div>
    </div>
    <div id="k-ecran-layers" class="k-ecran__layers"></div>
  </div>
</template>

<style>
@layer components {
  /* The slots: sidebar, then a column of bar, band and body, at the height of the window
   * (dynamic where supported). The window never scrolls. */
  .k-ecran {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    block-size: var(--k-size-viewport);
    min-block-size: var(--k-size-viewport);
    overflow: hidden;
    color: var(--k-text);
    background-color: var(--k-bg);
  }

  /* The button of the drawer of the sidebar: the compact class only. */
  .k-ecran__nav {
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

  .k-ecran > .k-sidebar {
    min-block-size: 0;
    overflow: auto;
  }

  .k-ecran__column {
    position: relative;
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr);
    grid-template-columns: minmax(0, 1fr);
    min-inline-size: 0;
    min-block-size: 0;
  }

  /* Without the band, the body takes its row. */
  .k-ecran__column > .k-ecran__body:nth-child(2) {
    grid-row: 2 / -1;
  }

  .k-ecran__bar {
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
  .k-ecran__crumbs {
    min-inline-size: 0;
    overflow: hidden;
  }

  .k-ecran__bar-end {
    display: flex;
    flex: none;
    gap: var(--k-space-2);
    align-items: center;
  }

  /* The surface, the strip of the closed panel, and the panel over their end; the panel is
   * positioned in here. */
  .k-ecran__body {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    min-block-size: 0;
    container: k-ecran-body / inline-size;
  }

  /* Room for the surface and the panel side by side (`--k-size-surface-min` plus the
   * least panel, 640 + 320): the open panel leaves the surface's end for its own column,
   * at most what keeps the surface at its least width; the strip gives it its column.
   * Not when it covers the surface (narrow class). */
  @container k-ecran-body (width >= 960px) {
    .k-ecran__body > .k-side-panel:not([data-cover], [data-sheet]) {
      position: relative;
      grid-row: 1;
      grid-column: 2;
      inline-size: min(var(--k-side-panel-width), var(--k-size-panel-beside));
      box-shadow: none;
    }

    .k-ecran__body:has(> .k-side-panel) > .k-side-panel-rail {
      display: none;
    }
  }

  .k-ecran__surface {
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

  /* The period picker, laid over the band and the surface under the date pill, out of the
   * flow: the page does not move (024/M15). */
  .k-ecran__picker {
    position: absolute;
    inset-block-start: calc(var(--k-size-status-bar) + var(--k-space-1));
    inset-inline-end: var(--k-space-2);
    z-index: var(--k-z-overlay);
    max-inline-size: calc(100% - var(--k-space-4));
    max-block-size: calc(100% - var(--k-size-status-bar) - var(--k-space-2));
    overflow: auto;
    background-color: var(--k-bg);
    border-radius: var(--k-radius-md);
  }

  .k-ecran__layers {
    display: contents;
  }

  /* The width classes (WIDTH_CLASSES of @kartenn/ui, equal by test): compact under 600 px,
   * narrow from 600, standard from 1024, wide from 1280. */

  /* Under wide: the zone of the date pill stays in its name only, room for the path. */
  @media (width < 1280px) {
    .k-ecran__bar .k-date-pill__zone {
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
    .k-ecran__surface {
      gap: var(--k-space-2);
      padding: var(--k-space-2);
    }
  }

  /* Compact: one column; the sidebar in its drawer, opened from the bar; a reduced bar.
   * The body is no container here: the sheet of the panel is laid on the window. */
  @media (width < 600px) {
    .k-ecran {
      grid-template-columns: minmax(0, 1fr);
    }

    .k-ecran > .k-sidebar {
      display: none;
    }

    .k-ecran__nav {
      display: inline-flex;
      flex: none;
    }

    .k-ecran__bar {
      gap: var(--k-space-1);
      padding-inline: var(--k-space-1);
    }

    .k-ecran__bar-end {
      gap: var(--k-space-1);
    }

    .k-ecran__body {
      container-type: normal;
    }
  }

  /* Secondary text of the mock-ups (an origin, a detail after a value). */
  .k-ecran-muted {
    color: var(--k-text-muted);
  }

  /* A list drawn by its items (operations, permissions). */
  .k-ecran-list {
    list-style: none;
  }

  /* A heading inside a panel or a drawer. */
  .k-ecran-subtitle {
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
    font-weight: var(--k-weight-semibold);
  }
}
</style>
