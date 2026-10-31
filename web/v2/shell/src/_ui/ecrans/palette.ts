import type { PaletteGroup } from "@kartenn/ui";

/**
 * The entries of the palette of the mock-ups (E8, and the button « Rechercher » or Ctrl+K
 * of every frame): objects, commands of the view, navigation (spec 021, REQ-021-3).
 */
export const PALETTE: PaletteGroup[] = [
  {
    label: "Objets",
    items: [
      { id: "p1", label: "Salle A", icon: "prefix", detail: "192.0.2.0/24" },
      { id: "p2", label: "Supervision", icon: "prefix", detail: "192.0.2.64/26" },
      { id: "a1", label: "srv-app-01", icon: "address", detail: "192.0.2.10" },
    ],
  },
  {
    label: "Commandes",
    items: [
      { id: "create", label: "Créer un préfixe dans 192.0.2.0/24", icon: "plus", shortcut: ["c"] },
      { id: "free", label: "Prochain libre dans 192.0.2.0/24", icon: "search" },
      { id: "date", label: "Lire à une autre date…", icon: "calendar" },
    ],
  },
  {
    label: "Navigation",
    items: [
      { id: "prefixes", label: "Préfixes", icon: "prefix", shortcut: [["g"], ["p"]] },
      { id: "addresses", label: "Adresses", icon: "address", shortcut: [["g"], ["a"]] },
    ],
  },
];

/** The entries whose label or detail holds the text typed (the host filters). */
export function paletteFilter(text: string): PaletteGroup[] {
  const wanted = text.trim().toLowerCase();
  return PALETTE.map((group) => ({
    ...group,
    items: group.items.filter((item) =>
      `${item.label} ${item.detail ?? ""}`.toLowerCase().includes(wanted),
    ),
  })).filter((group) => group.items.length > 0);
}
