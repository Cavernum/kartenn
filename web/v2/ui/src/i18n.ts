// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * The few words of the library itself (copy, copied, loading…), in the language the shell
 * sets with `setLocale`. Everything else is text the host passes. Reactive: a component
 * that calls `t` in its render follows a change of language.
 */

import { ref } from "vue";

export type Locale = "fr" | "en";

const MESSAGES = {
  fr: {
    copy: "Copier",
    copied: "Copié",
    copyFailed: "Copie impossible",
    loading: "Chargement",
    required: "obligatoire",
    keyEscape: "Échap",
    keyEnter: "Entrée",
    keySpace: "Espace",
    keyTab: "Tab",
    breadcrumb: "Fil d'Ariane",
    close: "Fermer",
    cancel: "Annuler",
    present: "Présent",
    readingAt: "Lecture au",
    backToPresent: "Revenir au présent",
    draft: "Brouillon",
    draftOperations: "opérations en brouillon",
    sidebar: "Navigation principale",
    collapse: "Replier la barre latérale",
    expand: "Déplier la barre latérale",
    palette: "Palette de commandes",
    paletteSearch: "Rechercher une commande, un objet, une page",
    noResults: "Aucun résultat",
    tabLoading: "en chargement",
    tabError: "en erreur",
    removeFilter: "Retirer le filtre",
    expandAll: "Tout déplier",
    before: "avant",
    after: "après",
    added: "ajouté",
    removed: "retiré",
    changed: "modifié",
    anomaly: "anomalie",
    movePending: "déplacement en attente",
    moveRefused: "déplacement refusé",
    dropOk: "cible acceptée",
    dropRefused: "cible refusée",
    childrenLoading: "chargement des enfants",
    pending: "en attente",
    inDraft: "en brouillon",
    inReview: "en revue",
    removedInDraft: "retirée dans le brouillon",
    search: "Rechercher",
    sidebarToggle: "Barre latérale",
    selected: "sélectionnée",
    needs: "Il faut",
    moreActions: "Plus d'actions",
    panelResize: "Redimensionner le panneau",
    panelClose: "Fermer le panneau",
    summary: "Résumé",
    descriptionOptional: "Description (facultative)",
    summaryMissing: "Le résumé est obligatoire.",
    chooseAction: "Choisir l'action",
    drafts: "Brouillons",
    draftsList: "Liste des brouillons",
    filtered: "filtrée",
    panelOpen: "Ouvrir le panneau",
    panelBack: "Retour",
    panelTabs: "Onglets du panneau",
    columns: "Colonnes",
    chartValues: "valeurs",
    chartMax: "maximum",
    chartLast: "dernière",
    chartEmpty: "aucune donnée",
    chartPoint: "Point",
    chartValue: "Valeur",
    enlarge: "Agrandir",
    row: "ligne",
    rows: "lignes",
  },
  en: {
    copy: "Copy",
    copied: "Copied",
    copyFailed: "Copy failed",
    loading: "Loading",
    required: "required",
    keyEscape: "Esc",
    keyEnter: "Enter",
    keySpace: "Space",
    keyTab: "Tab",
    breadcrumb: "Breadcrumb",
    close: "Close",
    cancel: "Cancel",
    present: "Present",
    readingAt: "Reading as of",
    backToPresent: "Back to the present",
    draft: "Draft",
    draftOperations: "draft operations",
    sidebar: "Main navigation",
    collapse: "Collapse the sidebar",
    expand: "Expand the sidebar",
    palette: "Command palette",
    paletteSearch: "Search a command, an object, a page",
    noResults: "No results",
    tabLoading: "loading",
    tabError: "failed",
    removeFilter: "Remove the filter",
    expandAll: "Expand all",
    before: "before",
    after: "after",
    added: "added",
    removed: "removed",
    changed: "changed",
    anomaly: "anomaly",
    movePending: "move pending",
    moveRefused: "move refused",
    dropOk: "accepted target",
    dropRefused: "refused target",
    childrenLoading: "loading children",
    pending: "pending",
    inDraft: "in draft",
    inReview: "in review",
    removedInDraft: "removed in the draft",
    search: "Search",
    sidebarToggle: "Sidebar",
    selected: "selected",
    needs: "Requires",
    moreActions: "More actions",
    panelResize: "Resize the panel",
    panelClose: "Close the panel",
    summary: "Summary",
    descriptionOptional: "Description (optional)",
    summaryMissing: "The summary is required.",
    chooseAction: "Choose the action",
    drafts: "Drafts",
    draftsList: "List of drafts",
    filtered: "filtered",
    panelOpen: "Open the panel",
    panelBack: "Back",
    panelTabs: "Tabs of the panel",
    columns: "Columns",
    chartValues: "values",
    chartMax: "maximum",
    chartLast: "last",
    chartEmpty: "no data",
    chartPoint: "Point",
    chartValue: "Value",
    enlarge: "Enlarge",
    row: "row",
    rows: "rows",
  },
} as const satisfies Record<Locale, Record<string, string>>;

export type MessageKey = keyof (typeof MESSAGES)["fr"];

const locale = ref<Locale>("fr");

/** Sets the language of the library's own words. */
export function setLocale(next: Locale): void {
  locale.value = next;
}

/** The current language, for `Intl` formats. */
export function currentLocale(): Locale {
  return locale.value;
}

/** A word of the library in the current language. */
export function t(key: MessageKey): string {
  return MESSAGES[locale.value][key];
}
