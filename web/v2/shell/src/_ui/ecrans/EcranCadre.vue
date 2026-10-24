<script setup lang="ts">
// The frame of the shell for the mock-ups (spec 021, « Cadre »): sidebar of spaces, status
// bar (breadcrumb, date pill, draft counter, user menu), band out of the present, work
// surface in panels, right panel (SidePanel) over the end of the surface. The real frame is a task of the 021;
// here it only places components of @kartenn/ui (layout, no look of its own).
import {
  Breadcrumb,
  DatePill,
  DraftCounter,
  Menu,
  SidebarNav,
  TimeBand,
  type Crumb,
  type MenuItem,
} from "@kartenn/ui";
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
    /** The date pill open on its picker. */
    picker?: boolean;
    draft?: number;
    draftOpen?: boolean;
    collapsed?: boolean;
  }>(),
  { at: undefined, draft: 2 },
);

const ZONE = "Europe/Paris";

const userMenu: MenuItem[] = [
  { kind: "label", label: "Camille Martin" },
  { id: "lang", label: "Langue : français", icon: "languages" },
  { id: "theme", label: "Thème : celui du système", icon: "monitor" },
  { id: "density", label: "Densité : compacte", icon: "layers" },
  { id: "keys", label: "Raccourcis à une touche : activés", icon: "keyboard" },
  { id: "help", label: "Raccourcis", icon: "circle-question-mark", shortcut: ["?"] },
  { kind: "separator" },
  { id: "about", label: "À propos", icon: "info" },
  { id: "logout", label: "Se déconnecter", icon: "log-out" },
];
</script>

<template>
  <div class="k-ecran" :data-sidebar="props.collapsed ? 'collapsed' : 'expanded'">
    <SidebarNav
      brand="kartenn"
      :groups="SPACES"
      :active="props.active"
      :closed="closedBut(props.space)"
      :collapsed="props.collapsed"
    />
    <div class="k-ecran__column">
      <header class="k-ecran__bar">
        <Breadcrumb :items="props.crumbs" />
        <div class="k-ecran__bar-end">
          <DatePill
            :at="props.at ?? null"
            :time-zone="ZONE"
            :expanded="props.picker"
            :aria-controls="$slots.picker ? 'k-ecran-picker' : undefined"
          />
          <DraftCounter
            :count="props.draft"
            :expanded="props.draftOpen"
            :controls="props.draftOpen ? 'k-ecran-draft' : undefined"
          />
          <Menu
            label="Compte de Camille Martin"
            icon="user"
            icon-only
            variant="ghost"
            align="end"
            :items="userMenu"
          />
        </div>
      </header>
      <TimeBand v-if="props.at" :at="props.at" :time-zone="ZONE" />
      <div v-if="$slots.picker" id="k-ecran-picker" class="k-ecran__picker">
        <slot name="picker" />
      </div>
      <div class="k-ecran__body">
        <main class="k-ecran__surface">
          <h1 class="k-visually-hidden">{{ props.title }}</h1>
          <slot />
        </main>
        <slot name="drawer" />
      </div>
    </div>
    <div id="k-ecran-layers" class="k-ecran__layers"></div>
  </div>
</template>

<style>
@layer components {
  /* The mock-ups take the whole page: no frame of the catalogue around them. */
  :root:has(.k-ecran),
  :root:has(.k-ecran) body,
  :root:has(.k-ecran) body > div {
    block-size: 100%;
  }

  .k-ecran {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    min-block-size: 100%;
    color: var(--k-text);
    background-color: var(--k-bg);
  }

  .k-ecran__column {
    display: flex;
    flex-direction: column;
    min-inline-size: 0;
  }

  .k-ecran__bar {
    position: sticky;
    inset-block-start: 0;
    z-index: var(--k-z-sticky);
    display: flex;
    gap: var(--k-space-4);
    align-items: center;
    justify-content: space-between;
    min-block-size: var(--k-size-status-bar);
    padding-inline: var(--k-space-4) var(--k-space-2);
    background-color: var(--k-surface);
    border-block-end: var(--k-border-width) solid var(--k-border-strong);
  }

  .k-ecran__bar-end {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
  }

  /* The period picker opens under the date pill, at the end of the bar. */
  .k-ecran__picker {
    display: flex;
    justify-content: end;
    padding: var(--k-space-2) var(--k-space-4) 0;
  }

  /* The right panel (SidePanel) lies over the end of the surface, which isolates its own
   * sticky headers below it. */
  .k-ecran__body {
    position: relative;
    display: flex;
    flex: 1;
    align-items: stretch;
    min-block-size: 0;
  }

  .k-ecran__surface {
    display: grid;
    flex: 1;
    gap: var(--k-space-3);
    align-content: start;
    min-inline-size: 0;
    padding: var(--k-space-3);
    scroll-padding-block-start: var(--k-size-status-bar);
    isolation: isolate;
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
