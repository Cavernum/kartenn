import type { LocationQuery } from "vue-router";

/** The switches of the catalogue, read from the query (design « Catalogue »). */
export interface Switches {
  mode: "light" | "dark";
  densite: "compacte" | "confortable";
  langue: "fr" | "en";
}

export const DEFAULT_SWITCHES: Switches = { mode: "light", densite: "compacte", langue: "fr" };

const pick = <T extends string>(value: unknown, allowed: readonly T[], fallback: T): T =>
  typeof value === "string" && (allowed as readonly string[]).includes(value)
    ? (value as T)
    : fallback;

export function readSwitches(query: LocationQuery): Switches {
  return {
    mode: pick(query.mode, ["light", "dark"], DEFAULT_SWITCHES.mode),
    densite: pick(query.densite, ["compacte", "confortable"], DEFAULT_SWITCHES.densite),
    langue: pick(query.langue, ["fr", "en"], DEFAULT_SWITCHES.langue),
  };
}

/**
 * Applies the switches to the root: forced mode (`data-theme`, never the machine's, so
 * that a capture fixes its mode), density and language.
 */
export function applySwitches(root: HTMLElement, switches: Switches): void {
  root.dataset.theme = switches.mode;
  if (switches.densite === "confortable") root.dataset.density = "comfortable";
  else delete root.dataset.density;
  root.lang = switches.langue;
}
