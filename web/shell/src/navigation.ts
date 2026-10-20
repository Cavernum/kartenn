import type { Rights } from "./rights/permissions.ts";

/** An entry of the main navigation: shown when one of `anyOf` is held, or always. */
export interface NavEntry {
  to: string;
  label: string;
  anyOf?: readonly string[];
}

/** Permissions that open the administration (spec 005: RBAC, identities; spec 012: audit). */
export const ADMIN_PERMISSIONS = ["core.rbac.read", "core.rbac.admin", "core.audit.read"] as const;

/** The main navigation (design « Écrans et routes du client »), in this order. */
export const NAVIGATION: readonly NavEntry[] = [
  { to: "/", label: "nav.tree" },
  { to: "/admin", label: "nav.admin", anyOf: ADMIN_PERMISSIONS },
  { to: "/about", label: "nav.about" },
];

/** The entries a user may see: a hint only, the server stays the judge. */
export function visibleEntries(rights: Rights, entries = NAVIGATION): NavEntry[] {
  return entries.filter((e) => !e.anyOf || rights.hasAny(e.anyOf));
}
