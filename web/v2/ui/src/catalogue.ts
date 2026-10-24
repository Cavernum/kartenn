// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * `@kartenn/ui/catalogue` (design « Catalogue »): the states each component declares in
 * its sibling `<Nom>.catalogue.ts`, read by the catalogue /_ui of the shell and by the
 * captures (one state, one capture). Never imported by a production build.
 */

import type { Component } from "vue";

/**
 * One state of a component: a stable name (part of its URL and capture) and its props;
 * `slot` fills the default slot with text; `demo` renders a composition instead of the
 * component alone (layouts, a button beside a field, an open menu with its trigger);
 * `interaction` shows a state that only the pointer or the keyboard reach.
 */
export interface CatalogueState {
  name: string;
  description: string;
  props: Record<string, unknown>;
  slot?: string;
  demo?: Component;
  /**
   * An interaction the capture performs before shooting, on the first focusable element
   * of the state (or `target`, a selector inside it): hover, keyboard focus, press.
   */
  interaction?: "hover" | "focus" | "press";
  target?: string;
  /** The state takes the whole width of the page (layouts, grids, bars). */
  wide?: boolean;
  /**
   * Shown alone only (`?etat=`): an open modal layer would trap the focus of the whole
   * page. The page of all states links to it.
   */
  isolated?: boolean;
}

/** The declaration of a component in the catalogue. */
export interface CatalogueEntry {
  component: Component;
  /** WAI-ARIA APG pattern or role (design « Inventaire des composants »). */
  pattern: string;
  states: CatalogueState[];
}

const modules = import.meta.glob<{ default: CatalogueEntry }>("./components/*/*.catalogue.ts");

const nameOf = (path: string) => path.split("/").at(-1)!.replace(".catalogue.ts", "");

/** Names of the components that declare their states, sorted. */
export const catalogueNames: string[] = Object.keys(modules).map(nameOf).sort();

/** The declaration of a component, or null if it has none. */
export async function loadCatalogue(name: string): Promise<CatalogueEntry | null> {
  const path = Object.keys(modules).find((key) => nameOf(key) === name);
  return path ? (await modules[path]!()).default : null;
}
