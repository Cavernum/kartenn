<script setup lang="ts">
// The frame of the shell for the mock-ups (spec 021, « Cadre », revised by the 024: M8,
// M15): fixed slots laid by its grid — sidebar of spaces (collapsible), status bar
// (breadcrumb, date pill, draft counter, user menu), band out of the present, work
// surface, right panel (SidePanel) over the end of the surface. Each slot scrolls inside
// itself; the page never does. The surface is an isolated stacking context: its sticky
// headers never pass over the panel nor over a floating layer. The draft counter opens
// the panel on its draft tab, or closes it when open (its icon is then a cross); closed,
// the panel leaves its strip at the end of the body, whose chevron reopens it. Under 1024 px the sidebar is
// collapsed and the panel covers the surface. The real frame is a task of the 021; here
// it only places components of @kartenn/ui (layout, no look of its own).
import {
  Breadcrumb,
  DatePill,
  DraftCounter,
  Menu,
  SidePanel,
  SidebarNav,
  TimeBand,
  applyMotion,
  currentMotion,
  type Crumb,
  type MotionSetting,
  type DraftSummary,
  type MenuItem,
  type TabItem,
} from "@kartenn/ui";
import { computed, onBeforeUnmount, ref, useSlots } from "vue";
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

// A narrow window (under 1024 px): the sidebar collapses, the panel covers the surface.
const query = typeof window === "undefined" ? null : window.matchMedia("(width < 1024px)");
const narrow = ref(query?.matches ?? false);
const onQuery = (event: MediaQueryListEvent) => (narrow.value = event.matches);
query?.addEventListener("change", onQuery);
onBeforeUnmount(() => query?.removeEventListener("change", onQuery));

const collapsed = ref(props.collapsed || narrow.value);
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
  <div class="k-ecran" :data-sidebar="collapsed ? 'collapsed' : 'expanded'">
    <SidebarNav
      v-model:collapsed="collapsed"
      brand="kartenn"
      :groups="SPACES"
      :active="props.active"
      :closed="closedBut(props.space)"
    />
    <div class="k-ecran__column">
      <header class="k-ecran__bar">
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
      <TimeBand v-if="props.at" :at="props.at" :time-zone="ZONE" />
      <div class="k-ecran__body">
        <main class="k-ecran__surface">
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
          :cover="narrow"
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
  /* The mock-ups take the whole window: no frame of the catalogue around them. */
  :root:has(.k-ecran),
  :root:has(.k-ecran) body,
  :root:has(.k-ecran) body > div {
    block-size: 100%;
  }

  /* The slots: sidebar, then a column of bar, band and body. The window never scrolls. */
  .k-ecran {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    block-size: 100%;
    overflow: hidden;
    color: var(--k-text);
    background-color: var(--k-bg);
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
