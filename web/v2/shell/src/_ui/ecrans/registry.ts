/**
 * The mock-ups of the design system (spec 020, design « Maquettes », REQ-020-25; tasks 13
 * and 14; revised by the 024, task 11): eleven screens (E0 to E10) built with the
 * components of `@kartenn/ui`, on fictitious data, to be validated by the user on captures.
 * Each mock-up has one or more views (`?vue=`), each one URL and one capture per width
 * (1440, 1280, 1024, 768 px) and mode. Pure data: the captures (`e2e/visual/ecrans.spec.ts`)
 * read this list.
 */

/** A view of a mock-up: a state of the screen, shown alone. */
export interface MockupView {
  id: string;
  label: string;
  description: string;
  /** A modal layer is open (dialog, palette): it hides the rest of the page and traps focus. */
  modal?: boolean;
  /** The element that has the focus in the capture (a refused action and its tooltip). */
  focus?: string;
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
    id: "E0",
    title: "Accueil",
    shows:
      "Tableau de bord : tuiles du cœur (GET /v1/stats) et widget de deux plugins, une tuile en chargement.",
    views: [
      {
        id: "tuiles",
        label: "Tableau de bord",
        description:
          "Objets par sorte, changesets appliqués et histogramme, à instruire, les miens, anomalies ; tuiles VLAN et Flux ; Politique en chargement.",
      },
    ],
  },
  {
    id: "E1",
    title: "Topographie",
    shows:
      "Barre latérale, barre d'état, arbre des préfixes avec « + » sur un nœud, table filtrée par environnement avec une ligne en brouillon et une en revue, détail de la ligne choisie dans le panneau du bas.",
    views: [
      {
        id: "noeud",
        label: "Nœud choisi",
        description:
          "Arbre à parent déduit, contenu du nœud choisi filtré, détail de la ligne choisie dans le panneau du bas, en plusieurs colonnes ; arbre et table gardent toute leur largeur.",
      },
      {
        id: "ajout",
        label: "Ajout depuis l'arbre",
        description:
          "« + » d'un nœud : panneau d'édition de la sorte (le même partout), nœud pour parent.",
      },
      {
        id: "replie",
        label: "Barre latérale repliée",
        description: "Même écran, barre latérale repliée en icônes (touche [), panneau fermé.",
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
        description:
          "Menu « Actions » ouvert : actions du cœur puis des plugins, les refusées grisées.",
      },
      {
        id: "droits",
        label: "Action refusée",
        description: "Action interdite au focus : infobulle « Il faut core.address.edit ».",
        focus: ".k-button[aria-disabled='true']",
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
        description:
          "Filtres typés, cellule en édition, cellule refusée au focus : son message et son code dans une couche jointe sous la cellule, qui garde sa taille.",
        focus: "[role='gridcell'][aria-invalid='true']",
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
      "Panneau droit sur l'onglet Brouillon au-dessus d'une liste, opérations, simulation en cours puis avec violations codées, CommitForm et SplitButton ouvert ; page /drafts.",
    views: [
      {
        id: "simulation",
        label: "Simulation en cours",
        description:
          "Panneau non modal sur la fin de la liste ; simulation en cours, « Appliquer » attend.",
      },
      {
        id: "violations",
        label: "Violations",
        description:
          "Simulation terminée : violations en clair et codées par opération ; seul l'enregistrement reste possible.",
      },
      {
        id: "soumission",
        label: "Soumission",
        description:
          "Simulation valide mais retenue par une politique : chevron ouvert, « Appliquer » grisé avec son motif.",
        modal: true,
        focus: "[role='menuitem'][aria-disabled='true']",
      },
      {
        id: "liste",
        label: "Mes brouillons",
        description: "Page /drafts : reprendre, créer, supprimer ; suppression annoncée.",
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
    shows:
      "Droits par groupes (spec 025) : groupes, rôles, identités ; permissions module.action (spec 018), secret affiché une fois.",
    views: [
      {
        id: "groupes",
        label: "Groupes",
        description:
          "Groupes ; le groupe choisi dans le panneau droit : membres (identités, groupes de l'IdP « émetteur / groupe », sorte à droite), puis rôles sur périmètres (libellé et clé, périmètre) ; actions de chaque ligne dans une dernière colonne, « Ajouter » sous chaque liste.",
      },
      {
        id: "groupe-edition",
        label: "Groupe en édition",
        description:
          "Le même groupe modifié : un membre et un rôle retirés (barrés, « Annuler le retrait »), un groupe de l'IdP et un rôle ajoutés en fin de liste, au style du brouillon ; compteur et onglet « Brouillon 4 ».",
      },
      {
        id: "groupe-refus",
        label: "Groupe hors périmètre",
        description:
          "Un groupe lié sur un périmètre global, pour un administrateur d'entité : « Lecture seule » annoncé en tête (core.group.escalation), chaque action refusée par motif « Périmètre plus large que le vôtre » (core.group.escalation, core.binding.escalation).",
      },
      {
        id: "secours",
        label: "Groupe livré",
        description:
          "Le groupe livré secours, annoncé en tête : ni modifiable ni clos (core.group.builtin) ; ses membres se gèrent seulement par un administrateur global.",
      },
      {
        id: "identite",
        label: "Identité",
        description:
          "Identités ; l'identité choisie, en lecture pour ses droits : état et actions (désactiver, révoquer les sessions), groupes kartenn avec « via » (direct ou groupe de l'IdP), droits effectifs, groupes de l'IdP de sa dernière session datée.",
      },
      {
        id: "roles",
        label: "Rôles",
        description:
          "Rôles ; le rôle choisi : permissions une par ligne (libellé puis nom servi, action Retirer), groupes liés en lecture (les liaisons se gèrent depuis le groupe).",
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
      "Vue pleine page d'un plugin (VLAN) et matrice de politique (spec 017, MatrixGrid) dans le cadre du shell.",
    views: [
      {
        id: "vlan",
        label: "Plugin VLAN",
        description: "Groupes de VLAN, VLAN du groupe choisi, prochain VID libre.",
      },
      {
        id: "matrice",
        label: "Matrice de politique",
        description:
          "Matrice entre environnements (en-têtes figés des deux côtés, croix sur la cellule choisie, sens source → destination), cellule choisie dans le panneau droit, effet simulé.",
      },
      {
        id: "brouillon",
        label: "Brouillon de la matrice",
        description:
          "Onglet Brouillon : cellules changées dans les matrices de deux types de flux (type, source → destination, état avant et après, motif et libellé ajoutés), effet simulé groupé par type de flux, autres opérations, pied (simulation, CommitForm, SplitButton).",
      },
    ],
  },
];

/** The view `id` of a mock-up, or its first one. */
export function viewOf(mockup: Mockup, id: unknown): MockupView {
  return mockup.views.find((view) => view.id === id) ?? mockup.views[0]!;
}
