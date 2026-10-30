/**
 * The mock-ups of the design system (spec 020, design « Maquettes », REQ-020-25; tasks 13
 * and 14; revised by the 024, tasks 11 and 11b; E12 added by the 026, E13 by the 027):
 * fourteen screens (E0 to E13) built with the components of `@kartenn/ui`, on fictitious
 * data, to be validated by the user on captures.
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
      "Tableau de bord : tuiles du cœur (GET /v1/stats, dont l'occupation des groupes de VLAN) et widget d'un plugin, une tuile de plugin en chargement.",
    views: [
      {
        id: "tuiles",
        label: "Tableau de bord",
        description:
          "Objets par sorte, changesets appliqués et histogramme, à instruire, les miens, anomalies, VLAN et occupation des groupes ; widget du plugin Flux ; Politique en chargement.",
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
          "Vu par un relecteur sans core.vlan.apply : Valider et Rejeter grisés, la permission en infobulle.",
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
          "Pastille de date ouverte sur le sélecteur de période (valide au, connu au, raccourcis), en surimpression : la page ne bouge pas.",
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
      "Vue pleine page d'un plugin dans le cadre du shell : la matrice de politique (spec 017, MatrixGrid).",
    views: [
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
  {
    id: "E11",
    title: "Flux",
    shows:
      "Registre des flux (filtres typés, état, conformité à la matrice de flux, issue), proposition de flux en tableau dans un changeset avec la ligne choisie dans le panneau du bas, tester une connexion (ouvert, conforme) sans écriture, flux des interfaces d'une machine dans son onglet d'information, fiche d'une interface (média, rôle, VLAN, adresses, flux). Une extrémité est une adresse, une interface ou un préfixe, jamais une machine.",
    views: [
      {
        id: "registre",
        label: "Registre",
        description:
          "Flux déclarés et ouverts : source et destination (interface, adresse ou préfixe) en liens vers la fiche de leur machine, une extrémité dont l'objet n'est pas lu par sa sorte et ses valeurs (préfixe, ou adresses d'une interface), sans lien, et un cadenas dont l'infobulle dit « Fiche non accessible » et la permission qui l'ouvre (027) ; service (libellé et clé), type de flux, état, conformité et issue, échéance ; filtres typés en pastilles ; « Proposer des flux », « Tester une connexion ».",
      },
      {
        id: "proposition",
        label: "Proposition",
        description:
          "Formulaire en tableau, une ligne par flux (« Ajouter un flux »), verdict de chaque ligne (badge et code) : conforme automatique, sous réserve (justification manquante, en édition), non conforme (règle deny), non classé ; la ligne choisie dans le panneau du bas, un sujet par onglet (Extrémités, Matrice, Verdicts), chacun ouvert sur la même réponse d'une ligne (synthèse, issue, élément décisif, paires autorisées).",
      },
      {
        id: "envoi",
        label: "Envoi de la proposition",
        description:
          "Onglet Brouillon de la proposition : flux proposés et leur verdict, effet simulé par issue, pied (simulation, CommitForm, SplitButton retenu par l'instruction des flux).",
      },
      {
        id: "tester",
        label: "Tester une connexion : ouvert et conforme",
        description:
          "Adresse vers interface (2 paires), transport et port : couverte par un flux actif dont la source est un préfixe, autorisée par la matrice. Réponse en une ligne, puis « Ouvert ? » (flux cités) et « Conforme ? » (règle décisive, cellule), puis les deux réponses par paire d'adresses. Aucune écriture.",
      },
      {
        id: "tester-derogation",
        label: "Tester une connexion : ouvert mais non conforme",
        description:
          "Interface vers adresse, service : ouverte par un flux dérogé (dérogation et échéance citées) qu'une règle deny interdit.",
      },
      {
        id: "tester-a-declarer",
        label: "Tester une connexion : non ouvert mais conforme",
        description:
          "Une machine saisie en source demande le choix de son interface ou d'une adresse ; aucun flux ne couvre les paires, la matrice les autorise : « Déclarer ce flux » préremplit une ligne de la proposition.",
      },
      {
        id: "tester-ferme",
        label: "Tester une connexion : non ouvert et non conforme",
        description:
          "Interface vers interface : aucun flux ne couvre les paires, une règle deny les interdit.",
      },
      {
        id: "machine",
        label: "Flux d'une machine",
        description:
          "Fiche de srv-app-01, onglet Information : attributs (interfaces en liens, avec leur média et leur rôle), puis les flux de ses interfaces, entrants et sortants, en liste compacte : l'interface (et l'adresse ou le préfixe qui l'atteint), l'autre extrémité (une interface en lien vers sa fiche, une adresse vers la fiche de sa machine ; un objet non lu : sa sorte et ses valeurs, sans lien ni panneau, un cadenas dont l'infobulle dit « Fiche non accessible » et la permission, 027), service, type de flux, conformité à la matrice.",
      },
      {
        id: "interface",
        label: "Fiche d'une interface",
        description:
          "srv-app-01 · eth0 : nom, machine en lien, MAC, état, média (Ethernet, SFP, Wi-Fi, agrégat, virtuelle, bouclage ; clé et libellé, modifiable par la liste fermée), rôle, VLAN ; adresses rattachées, la principale marquée ; flux de l'interface, entrants et sortants, l'autre extrémité en lien, conformité à la matrice ; historique.",
      },
    ],
  },
  {
    id: "E12",
    title: "VLAN",
    shows:
      "Écrans du VLAN dans le cœur (026) : groupes (ancre, plage de VID, occupation), un groupe (tranches utilisées, réservées et libres, prochain VID libre, ses VLAN), fiche d'un VLAN (réseaux qui le portent, lus selon les droits), création d'un VLAN pour un réseau depuis la liste des préfixes et sa colonne VLAN.",
    views: [
      {
        id: "groupes",
        label: "Groupes de VLAN",
        description:
          "Groupes : ancre (entité, site ou domaine), plage de VID, nombre de VLAN, occupation et VID libres ; occupation des groupes en barres ; « Créer un groupe » refusé sans core.vlan_group.edit.",
      },
      {
        id: "groupe",
        label: "Un groupe",
        description:
          "campus-ouest : ses VLAN (VID, nom, statut, réseaux ; un en brouillon) ; attributs ; occupation en barres et en tranches de VID utilisées, réservées et libres ; prochain VID libre, le plus petit et à partir d'un VID.",
      },
      {
        id: "vlan",
        label: "Fiche d'un VLAN",
        description:
          "VLAN 121 supervision : VID, nom, statut, groupe en lien ; réseaux qui le portent, lus selon les droits (ni listés ni comptés hors droits), un réseau hors du site du groupe (core.vlan.site_mismatch) ; onglet Interfaces du plugin Machines.",
      },
      {
        id: "creation",
        label: "Créer un VLAN pour un réseau",
        description:
          "Liste des préfixes avec sa colonne VLAN (propriété core.vlan des réseaux) ; un réseau sans VLAN choisi, panneau droit : groupe, VID prérempli du prochain libre, nom, statut ; deux opérations au brouillon.",
      },
    ],
  },
  {
    id: "E13",
    title: "Paramètres",
    shows:
      "Paramètres par déclarant (027), entrée de l'espace Administration : un onglet par déclarant (Cœur, Machines, Politique) ; champs, listes fermées en tableau-formulaire, environnements, services et leurs couples transport/port, types de flux, axes ; lecture seule sans la permission du déclarant, nommée par son code ; modifications au brouillon ; clôture d'une valeur citée refusée avec la liste des objets.",
    views: [
      {
        id: "coeur",
        label: "Cœur",
        description:
          "Champs du cœur et custom.* (clé, libellés, type, porteurs, héritage, sensibilité ; custom.astreinte au brouillon) ; liste fermée core.external_system en tableau-formulaire : clé figée, libellés FR/EN, période de validité, citations ; un libellé en édition, un autre et une valeur au brouillon.",
      },
      {
        id: "machines",
        label: "Machines",
        description:
          "Vu avec machines.settings.edit seule : champs synchronisés du module, médias d'interface (ethernet, sfp, sfp28 ajoutée au brouillon, qsfp, wifi relibellée…) et rôles ; machines.fqdn choisi dans le panneau droit, « Rendre sensible » refusé : il faut core.settings.edit.",
      },
      {
        id: "politique",
        label: "Politique",
        description:
          "Environnements, services (un type de flux, couples tcp, udp, sctp, plage, icmp, icmpv6, ip), types de flux, axes et valeurs ; preprod, ldaps et la valeur d'axe pci au brouillon, panneau droit sur l'onglet Brouillon groupé par élément.",
      },
      {
        id: "politique-lecture",
        label: "Politique en lecture",
        description:
          "Même onglet sans policy.settings.edit : « Lecture seule » annoncé en tête avec le code, chaque ajout refusé par « Il faut policy.settings.edit ».",
        focus: ".k-button[aria-disabled='true']",
      },
      {
        id: "cloture",
        label: "Clôture refusée",
        description:
          "L'environnement sandbox choisi : sa clôture refusée (policy.reference.closed), les objets qui le citent listés — deux réseaux, une adresse, une règle.",
      },
    ],
  },
];

/** The view `id` of a mock-up, or its first one. */
export function viewOf(mockup: Mockup, id: unknown): MockupView {
  return mockup.views.find((view) => view.id === id) ?? mockup.views[0]!;
}
