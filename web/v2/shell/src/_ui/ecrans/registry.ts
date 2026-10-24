/**
 * The mock-ups of the design system (spec 020, design « Maquettes », REQ-020-25; tasks 13
 * and 14): ten screens built with the visual version of the components of `@kartenn/ui`,
 * on fictitious data, to be validated by the user on captures (task 15). Each mock-up has
 * one or more views (`?vue=`), each one URL and one capture per mode and density. Pure
 * data: the captures (`e2e/visual/ecrans.spec.ts`) read this list.
 */

/** A view of a mock-up: a state of the screen, shown alone. */
export interface MockupView {
  id: string;
  label: string;
  description: string;
  /** A modal layer is open (dialog, palette): it hides the rest of the page and traps focus. */
  modal?: boolean;
}

export interface Mockup {
  id: `E${number}`;
  title: string;
  /** What the mock-up must show (design « Maquettes »). */
  shows: string;
  views: MockupView[];
}

export const MOCKUPS: Mockup[] = [
  {
    id: "E1",
    title: "Référentiel",
    shows:
      "Barre latérale, barre d'état, arbre des préfixes, table filtrée par environnement avec une ligne en brouillon et une en revue.",
    views: [
      {
        id: "noeud",
        label: "Nœud choisi",
        description: "Arbre à parent déduit, contenu du nœud choisi, filtre d'environnement.",
      },
      {
        id: "replie",
        label: "Barre latérale repliée",
        description: "Même écran, barre latérale repliée en icônes (touche [).",
      },
    ],
  },
  {
    id: "E2",
    title: "Fiche d'un préfixe",
    shows:
      "Titre, attributs, propriétés résolues et leur explication, occupation, historique, onglets de plugins, menu d'actions.",
    views: [
      {
        id: "proprietes",
        label: "Propriétés",
        description: "Onglet des propriétés résolues, explication d'une valeur héritée ouverte.",
      },
      {
        id: "historique",
        label: "Historique",
        description: "Onglet de l'historique : versions bitemporelles et changesets.",
      },
      {
        id: "actions",
        label: "Menu d'actions",
        description: "Menu « Actions » ouvert : actions du cœur puis des plugins.",
      },
    ],
  },
  {
    id: "E3",
    title: "Liste d'adresses",
    shows:
      "Filtres typés en pastilles, édition en place d'une cellule, cellule invalide avec code, sélection multiple et action groupée.",
    views: [
      {
        id: "edition",
        label: "Édition en place",
        description: "Filtres typés, cellule en édition, cellule refusée avec son code.",
      },
      {
        id: "selection",
        label: "Sélection multiple",
        description: "Trois lignes choisies, barre d'action groupée, menu ouvert.",
      },
    ],
  },
  {
    id: "E4",
    title: "Changeset et revue",
    shows:
      "Différentiel groupé par objet, anomalies rattachées, actions de revue selon les droits (action interdite grisée, permission en infobulle, sans encart des droits), état « en revue ».",
    views: [
      {
        id: "relecteur",
        label: "Relecteur",
        description: "Changeset en revue, vu par qui a le droit de valider.",
      },
      {
        id: "droits",
        label: "Droit manquant",
        description:
          "Vu par un relecteur sans vlan.apply : Valider et Rejeter grisés, la permission en infobulle.",
      },
      {
        id: "auteur",
        label: "Auteur",
        description:
          "Vu par son auteur : une politique le retient, Valider grisé avec son motif, notice changeset.approval.self.",
      },
      {
        id: "rejet",
        label: "Rejet avec motif",
        description: "Dialogue de rejet : motif obligatoire.",
        modal: true,
      },
    ],
  },
  {
    id: "E5",
    title: "Brouillon",
    shows:
      "Panneau latéral ouvert sur une liste, opérations, simulation en cours puis avec violations codées, soumission avec motif.",
    views: [
      {
        id: "simulation",
        label: "Simulation en cours",
        description: "Panneau non modal qui pousse la liste ; simulation en cours.",
      },
      {
        id: "violations",
        label: "Violations",
        description: "Simulation terminée : violations codées par opération.",
      },
      {
        id: "soumission",
        label: "Soumission",
        description: "Simulation valide, motif saisi, soumission (Ctrl+Entrée).",
      },
    ],
  },
  {
    id: "E6",
    title: "Lecture au passé",
    shows:
      "Bande ambre, pastille de date et sélecteur de période ouvert en surimpression, écritures grisées avec motif, chronologie des versions.",
    views: [
      {
        id: "selecteur",
        label: "Sélecteur de date",
        description:
          "Pastille de date ouverte sur le sélecteur (composant à venir), en surimpression : la page ne bouge pas.",
      },
      {
        id: "chronologie",
        label: "Chronologie",
        description: "Fiche lue au passé : écritures grisées avec motif, versions.",
      },
    ],
  },
  {
    id: "E7",
    title: "Administration",
    shows: "Rôles et liaisons, permissions module.action (spec 018), secret affiché une fois.",
    views: [
      {
        id: "roles",
        label: "Rôles",
        description:
          "Rôles ; le rôle choisi dans le panneau droit : permissions une par ligne (nom servi et libellé), liaisons qui défilent dans le panneau.",
      },
      {
        id: "liaisons",
        label: "Liaisons",
        description:
          "Liaisons (sujet, rôle, portée, période) ; nouvelle liaison refusée dans le panneau droit.",
      },
      {
        id: "secret",
        label: "Secret affiché une fois",
        description: "Nouveau secret d'un compte de service, montré une seule fois.",
        modal: true,
      },
    ],
  },
  {
    id: "E8",
    title: "Palette et aide",
    shows: "Palette Ctrl+K (commandes, navigation, objets), aide « ? » des raccourcis.",
    views: [
      {
        id: "palette",
        label: "Palette",
        description: "Palette ouverte : objets, commandes, navigation.",
        modal: true,
      },
      {
        id: "aide",
        label: "Aide des raccourcis",
        description: "Aide « ? » : raccourcis lus au registre, par portée.",
        modal: true,
      },
    ],
  },
  {
    id: "E9",
    title: "États",
    shows:
      "Vide (phrase + action), chargement (squelettes dans les cellules), erreur en langage courant (code + Request-Id), 412 avec saisie gardée, 202 « proposé, en revue » avec son motif.",
    views: [
      {
        id: "zones",
        label: "Vide, chargement, erreur",
        description:
          "Trois zones de données, chacune dans un de ses états ; squelettes animés dans les cellules de la table.",
      },
      {
        id: "conflit",
        label: "412, saisie gardée",
        description:
          "« L'objet a changé depuis votre lecture » : valeur actuelle, code à côté, saisie gardée.",
        modal: true,
      },
      {
        id: "revue",
        label: "202, proposé en revue",
        description: "Écriture reçue mais soumise à revue, avec son motif : cellule « en revue ».",
      },
    ],
  },
  {
    id: "E10",
    title: "Hôte de plugin",
    shows:
      "Vue pleine page d'un plugin (VLAN) et matrice de politique (spec 017) dans le cadre du shell.",
    views: [
      {
        id: "vlan",
        label: "Plugin VLAN",
        description: "Groupes de VLAN, VLAN du groupe choisi, prochain VID libre.",
      },
      {
        id: "matrice",
        label: "Matrice de politique",
        description: "Matrice entre environnements, cellule choisie, effet simulé.",
      },
    ],
  },
];

/** The view `id` of a mock-up, or its first one. */
export function viewOf(mockup: Mockup, id: unknown): MockupView {
  return mockup.views.find((view) => view.id === id) ?? mockup.views[0]!;
}
