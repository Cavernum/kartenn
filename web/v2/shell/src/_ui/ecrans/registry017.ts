import type { Mockup } from "./registry.ts";

/**
 * The mock-ups of the plugins `policy` and `flow` (spec 017, task 1; U14 amended): one
 * screen per view of the two manifests (design, tables « Plugin `policy` » and « Plugin
 * `flow` »), its tabs and its actions as views, with the empty, error (412, 422, 404) and
 * reduced states. They follow E10 (matrix) and E11 (register, proposal, test of a
 * connection), validated on 2026-10-29, and E13 (settings of the policy, 027), and name
 * their gaps with the design of the 017 in their description.
 */
export const MOCKUPS_017: Mockup[] = [
  {
    id: "E14",
    title: "Règles et motifs",
    shows:
      "Vue policy-rules (REQ-017-4, 5) : règles filtrées par clé, effet, motif, type de flux et service, la règle choisie lue en clair dans le panneau droit avec son texte et son historique ; formulaire d'une règle (sélecteurs axe → valeurs, « même valeur », types de flux, services, effet, motifs, textes par langue) ; refus du client et du serveur placés sur leurs champs ; 412 saisie gardée ; motifs.",
    views: [
      {
        id: "regles",
        label: "Règles",
        description:
          "Règles du présent, filtres typés ; une règle multi-axes marquée « hors matrice » ; deny-deve-prod choisie : « interdire, source {environment: deve}, destination {environment: prod}, tous types de flux », texte, période, historique.",
      },
      {
        id: "formulaire",
        label: "Nouvelle règle interdite sans texte",
        description:
          "Formulaire d'une règle deny dans le panneau droit : sélecteurs axe → valeurs, contrainte « même valeur », types de flux et services, effet ; texte français vide : « Ajouter au brouillon » refusé, son motif en infobulle.",
        focus: ".k-side-panel .k-button[aria-disabled='true']",
      },
      {
        id: "violations",
        label: "Refus du serveur (422)",
        description:
          "La même règle envoyée : policy.rule.flow_selector_empty sur les types de flux, policy.reference.closed sur un service clos, policy.text.missing sur le texte ; la saisie reste.",
      },
      {
        id: "conflit",
        label: "Règle changée entre-temps (412)",
        description:
          "Deux onglets sur la même règle : le second reçoit 412 ; valeur actuelle, saisie gardée, relire ou réappliquer.",
        modal: true,
      },
      {
        id: "motifs",
        label: "Motifs",
        description:
          "Onglet Motifs : clé, libellé, texte pédagogique, règles qui le requièrent ; un motif choisi, sa clôture refusée tant qu'une règle le cite (policy.reference.closed).",
      },
    ],
  },
  {
    id: "E15",
    title: "Matrice au travail",
    shows:
      "Vue policy-matrix (REQ-017-6, 7, 8, 26 ; Q-017-1 amendée, Q-017-7), à la suite d'E10 : cellule choisie au clavier et écrite dans le panneau, cellules en brouillon, effet simulé du brouillon en pied de matrice, application d'une seule cellule (Ctrl+Entrée), matrice relue après 412, soumission retenue (202), type de flux sans environnement, lecture au passé.",
    views: [
      {
        id: "clavier",
        label: "Au clavier, effet en pied",
        description:
          "Cellule Recette → Production atteinte aux flèches (croix et sens), son état choisi dans le panneau (sous motif, motif exigé) ; deux cellules en brouillon ; pied de matrice : combinaisons qui s'ouvrent, se ferment, passent sous réserve, flux qui deviennent non conformes ou redeviennent conformes, premières lignes et élément décisif avant → après ; 2 règles hors vue en lien.",
        focus: ".k-matrix [role='gridcell'][tabindex='0']",
      },
      {
        id: "appliquer",
        label: "Appliquer une cellule (Ctrl+Entrée)",
        description:
          "PUT de la seule cellule sous If-Match, sans passer par le brouillon : motif demandé dans un dialogue ; le reste du brouillon n'est pas soumis.",
        modal: true,
      },
      {
        id: "relue",
        label: "Matrice relue (412)",
        description:
          "Une autre cellule écrite entre la lecture et l'envoi : policy.matrix.stale ; matrice relue (la cellule changée par Lou Bernard marquée), brouillon intact, il reprendra la nouvelle version à la soumission.",
      },
      {
        id: "retenue",
        label: "Soumission retenue (202)",
        description:
          "Brouillon soumis, retenu par la politique d'approbation policy.opens : brouillon vidé, matrice relue au présent, cellules en revue, lien vers l'effet du changeset.",
      },
      {
        id: "vide",
        label: "Aucun environnement",
        description:
          "Type de flux « Sauvegarde » sans environnement vivant à la date : table vide et phrase, lien vers les Paramètres de la politique.",
      },
      {
        id: "passe",
        label: "Au passé",
        description:
          "Lecture au 2025-11-03 : bande ambre, matrice et règles hors vue de cette date, aucune écriture (cellule en lecture, actions grisées avec leur motif).",
      },
    ],
  },
  {
    id: "E16",
    title: "Effet d'un changement de politique",
    shows:
      "Vue policy-changes (REQ-017-9) et action effect du changeset (E4, « Effet sur la politique ») : en-tête du changeset, sections policy.combinations, policy.reclassified et flow.flows lues page par page (links.next), lien vers le changeset où l'approbateur valide.",
    views: [
      {
        id: "combinaisons",
        label: "Combinaisons",
        description:
          "Changeset retenu par policy.opens ≥ 1 : décompte par nature, combinaisons source → destination × service, élément décisif avant → après, page suivante.",
      },
      {
        id: "reclasses",
        label: "Nœuds reclassés",
        description:
          "Section policy.reclassified : nœud, axe, valeur avant → après, nombre d'héritiers reclassés avec lui.",
      },
      {
        id: "flux",
        label: "Flux déclarés",
        description:
          "Section flow.flows (module flow présent) : flux qui deviennent non conformes, redeviennent conformes ou changent d'issue, dans une seule table triée par type de flux, règle décisive.",
      },
    ],
  },
  {
    id: "E17",
    title: "Catalogue de la politique",
    shows:
      "Vue policy-catalogue (REQ-017-10, 30, 31 ; Q-017-5) après la 027 : conditions d'admissibilité, signaux de risque et rôles exemptés (policy.rule.admin) ; environnements, services, types de flux, axes et valeurs sont écrits dans les Paramètres (E13, policy.settings.edit), le catalogue y renvoie.",
    views: [
      {
        id: "conditions",
        label: "Conditions",
        description:
          "Onglet Conditions : liste datée, la condition choisie en formulaire (sélecteur, exigence) ; texte pédagogique effacé : refusé côté client, puis policy.text.missing placé sur le champ.",
      },
      {
        id: "signaux",
        label: "Signaux de risque",
        description:
          "Onglet Signaux : sorte, paramètres, poids ; un poids modifié au brouillon, l'onglet Brouillon annonce les flux dont le score change (section flow.flows).",
      },
      {
        id: "roles",
        label: "Rôles exemptés",
        description:
          "Onglet Rôles exemptés : rôle, depuis ; création (un rôle choisi) et clôture ; lu sans policy.rule.admin : « Lecture seule » en tête, ajout refusé avec la permission.",
      },
    ],
  },
  {
    id: "E18",
    title: "Inférence",
    shows:
      "Vue policy-inference (REQ-017-32 à 34) et action expected : règles d'inférence (axe visé, attendus, critères, exceptions, priorité, domaine, révision du jeu), formulaire avec critères et exceptions imbriquées (profondeur 4 au plus) et aperçu sans écriture, écarts acquittables, attendu d'un nœud et son explication.",
    views: [
      {
        id: "regles",
        label: "Règles d'inférence",
        description:
          "Règles du jeu r17 par priorité ; env-par-site choisie : critères, exceptions, domaine, historique.",
      },
      {
        id: "formulaire",
        label: "Formulaire et aperçu",
        description:
          "Règle « prod » sur 192.0.2.0/26 dans le panneau droit, à côté des règles du jeu (un panneau d'édition par sorte, 024/M13) : critères (dont une expression régulière refusée, policy.inference.regex_invalid), exceptions en arbre jusqu'à la profondeur 4, la cinquième refusée ; aperçu : 12 nœuds, 3 écarts ouverts, avertissement policy.inference.conflict.",
      },
      {
        id: "ecarts",
        label: "Écarts",
        description:
          "Anomalies policy.inference.mismatch ouvertes : nœud d'origine, attendu et déclaré, héritiers en écart, règle ; un écart acquitté quitte la liste (annonce).",
      },
      {
        id: "attendu",
        label: "Attendu d'un nœud",
        description:
          "Action « Attendu d'inférence » d'une adresse : deux candidats à égalité, tous montrés, aucun retenu, aucun écart ; explication de chacun (règle et exceptions, révision, nœud de correspondance, critères et valeurs observées, règles écartées).",
        modal: true,
      },
    ],
  },
  {
    id: "E19",
    title: "Conformité",
    shows:
      "Vue policy-compliance (REQ-017-11 à 13) et action check : requête de conformité (extrémités, objet, dossier, entité, dates du contexte), synthèse puis évaluations par paire dépliables en explication complète ou réduite ; port sans service ; 422 et 404.",
    views: [
      {
        id: "complete",
        label: "Explication complète",
        description:
          "Source pré-remplie depuis la fiche de 192.0.2.10 (action check), destination l'interface srv-db-03 · eth0 (3 adresses), service postgres, sans dossier : synthèse en partie conforme, 3 évaluations (dont un point non classé) ; la non conforme expliquée dans le panneau droit : élément décisif, combinaison, règles écartées, conditions non évaluées, signaux et score, valeurs par point et axe avec leur source et leur origine.",
      },
      {
        id: "reduite",
        label: "Explication réduite",
        description:
          "Demandeur sous policy.flow.request : entité exigée, « point 1 → point 2 », valeurs d'environnement, aucune adresse, 2 adresses non lisibles omises.",
      },
      {
        id: "port",
        label: "Port sans service",
        description:
          "tcp/9999 : non conforme, unknown_service, issue à instruire, invitation à nommer le service (Paramètres de la politique).",
      },
      {
        id: "trop-large",
        label: "Requête trop large (422)",
        description:
          "policy-query-too-large : message traduit, invitation à restreindre les extrémités, saisie gardée.",
      },
      {
        id: "absente",
        label: "Extrémité absente (404)",
        description:
          "Identifiant collé inconnu ou non lisible : « absente ou non autorisée » sur le champ.",
      },
    ],
  },
  {
    id: "E20",
    title: "Rapports d'audit",
    shows:
      "Vue policy-reports (REQ-017-14) : rapports datés et leur état (pending, running, done), demande d'un rapport, 409 qui mène au rapport en cours, décompte par rubrique (indisponible ≠ 0), détail paginé lié aux fiches, export JSON.",
    views: [
      {
        id: "liste",
        label: "Rapports",
        description:
          "Liste des rapports ; un en cours relu toutes les 5 s ; « Demander un rapport » au 2026-01-15 : 409, le rapport en cours ouvert.",
      },
      {
        id: "rapport",
        label: "Un rapport",
        description:
          "Rapport fait : décompte par rubrique, une rubrique « indisponible » ; rubrique « non classifiés » de 12 nœuds, chacun lié à sa fiche ; export JSON.",
      },
    ],
  },
  {
    id: "E21",
    title: "Proposer des flux",
    shows:
      "Vue flow-request (REQ-017-15, 16) dans la forme d'E11 (proposition en tableau dans le brouillon) : dossier de la demande, pré-vérification, refus du serveur placés sur leurs champs, envoi (202 à instruire, 201 appliqué).",
    views: [
      {
        id: "dossier",
        label: "Dossier et pré-vérification",
        description:
          "Nouvelle demande pré-remplie depuis le flux échu flux-0988 : la ligne et son verdict ; panneau droit : dossier commun (entité, pour le compte de, responsables, nature des données, données personnelles, début et fin) ; panneau du bas : pré-vérification de la ligne (synthèse, issue, élément décisif, texte), sans écriture.",
      },
      {
        id: "refus",
        label: "Refus du serveur",
        description:
          "flow.reference.missing sur la référence d'une ligne, flow.endpoint.invalid sur une plage choisie en extrémité, core.object.forbidden sur l'entité (prop.core.owner.write manquant) ; saisie gardée.",
      },
      {
        id: "instruction",
        label: "Envoyée, à instruire (202)",
        description:
          "La proposition part à l'instruction : changeset en lien, issue et élément décisif de la pré-vérification (marqués comme tels) et texte de la règle, flux « Demandé » dans Mes demandes.",
      },
      {
        id: "appliquee",
        label: "Enregistrée (201)",
        description:
          "Proposition conforme : flux actifs et leurs évaluations, liens vers leurs fiches.",
      },
    ],
  },
  {
    id: "E22",
    title: "Fiche d'un flux",
    shows:
      "Fiche d'un flux (REQ-017-18, 22, 25) : champs, période, état, évaluation de l'enregistrement et évaluations courantes par paire, dérogation, historique ; modifier, avancer la fin, prolonger (pré-vérification de la portion ajoutée), clore, nouvelle demande pré-remplie ; réduite sans flow.read ; 404.",
    views: [
      {
        id: "fiche",
        label: "Fiche",
        description:
          "flux-1037 dérogé (dev-web-01 → srv-db-02, postgres) : non conforme à la matrice (deny-deve-prod), dérogation jusqu'au 2026-03-31 (motif, risque), évaluations par paire, actions dont Prolonger et Recertifier.",
      },
      {
        id: "reduite",
        label: "Lue par le demandeur",
        description:
          "Sans flow.read : évaluations réduites (points par rang, élément décisif absent), rendues sans complément ; écritures selon flow.request.",
      },
      {
        id: "prolonger",
        label: "Prolonger",
        description:
          "Nouvelle fin au 2026-12-31 : pré-vérification de la portion ajoutée avec le dossier du flux, sous réserve ; la prolongation partira à l'instruction.",
        modal: true,
      },
      {
        id: "prolongation",
        label: "Prolongation en attente (202)",
        description:
          "flux-1043 : la fiche garde l'ancienne fin et signale la prolongation en attente, lien vers son changeset.",
      },
      {
        id: "echu",
        label: "Flux échu",
        description:
          "flux-0988 échu : ni Prolonger ni Modifier, seule « Nouvelle demande pré-remplie » ; un flux sans paire d'adresses à la date le dit.",
      },
      {
        id: "absent",
        label: "Absent ou non autorisé (404)",
        description:
          "Lien vers un flux inexistant ou illisible : « absent ou non autorisé », code et Request-Id.",
      },
    ],
  },
  {
    id: "E23",
    title: "Instruction",
    shows:
      "Vue flow-review (REQ-017-20, 21 ; Q-017-3) et action review-in-flow : file des demandes et prolongations en attente (évaluation du triage lue telle quelle), fiche d'instruction, approbation avec fin et dérogation, rejet motivé, refus du serveur, séparation des tâches, « Instruire dans Flux » depuis « À instruire ».",
    views: [
      {
        id: "file",
        label: "File d'instruction",
        description:
          "Demandes et prolongations : nature, demandeur, entité, extrémités, service, issue, élément décisif, score, avertissements ; une demande de l'instructeur lui-même sans « Instruire » ; une page de 100.",
      },
      {
        id: "decision",
        label: "Approuver avec dérogation",
        description:
          "review/<changeset> : triage non conforme (deny-deve-prod et son texte), dossier ; approbation : fin ≤ 12 mois après la décision, dérogation exigée (motif, risque accepté, fin ≤ 6 mois).",
      },
      {
        id: "rejet",
        label: "Rejeter",
        description: "Dialogue de rejet : motif obligatoire, règle en cause reprise du triage.",
        modal: true,
      },
      {
        id: "refus",
        label: "Fin refusée",
        description:
          "Fin à 13 mois : refusée côté client ; forcée, flow.review.end_required affiché sans perte de saisie.",
      },
      {
        id: "demandeur",
        label: "Sa propre demande",
        description:
          "L'instructeur ouvre une demande faite pour son compte (on_behalf_of) : boutons absents, notice flow.separation.",
      },
      {
        id: "a-instruire",
        label: "« Instruire dans Flux »",
        description:
          "« À instruire » du shell (/proposals) : une demande et une prolongation de flux portent « Instruire dans Flux » à la place de « Valider » ; les autres changesets gardent Valider et Rejeter.",
      },
      {
        id: "vide",
        label: "File vide",
        description: "Rien à instruire : phrase et lien vers les échéances.",
      },
    ],
  },
  {
    id: "E24",
    title: "Échéances",
    shows:
      "Vue flow-expiring (REQ-017-22, 23 ; Q-017-4) : flux datés et dérogations finissant sous 30 jours, J-7 marqués, tri par fin ; recertifier une dérogation (réévaluée contre la politique du jour) ou prolonger le flux.",
    views: [
      {
        id: "echeances",
        label: "Échéances",
        description:
          "Fins sous 30 jours, la plus proche des deux fins (flux, dérogation) : deux à J-7 marquées, action de chaque ligne (Recertifier, Prolonger) ; un flux sans fin n'y figure jamais.",
      },
      {
        id: "recertifier",
        label: "Recertifier",
        description:
          "Dérogation de flux-1037 : réévaluation du jour (toujours non conforme, deny-deve-prod), motif, risque accepté, nouvelle fin ≤ 6 mois ; 202 si la politique flow.waiver la retient.",
        modal: true,
      },
      {
        id: "refusee",
        label: "Dérogation refusée",
        description:
          "Fin au-delà de 6 mois : refusée côté client ; forcée, flow.waiver.invalid sur le champ, saisie gardée.",
        modal: true,
      },
    ],
  },
  {
    id: "E25",
    title: "Non-conformités",
    shows:
      "Vue flow-nonconformities (REQ-017-24) : filtres entité, demandeur, règle, période, état et sorte ; motif, règle (code et libellé) et texte pédagogique dans la langue de l'utilisateur, repli indiqué ; lien vers le flux ou le changeset rejeté ; acquittement d'un flux non conforme ; curseur.",
    views: [
      {
        id: "liste",
        label: "Non-conformités",
        description:
          "Rejets, flux non conformes, flux dérogés ; une règle sans texte français : texte anglais, langue indiquée ; « Suivantes » par curseur.",
      },
      {
        id: "miennes",
        label: "Mes non-conformités",
        description:
          "Le demandeur voit les siennes (requester = lui) : un rejet motivé avec la règle et son texte, lien vers le changeset rejeté.",
      },
      {
        id: "acquitter",
        label: "Acquitter",
        description:
          "Acquitter la non-conformité de flux-1029 (anomalie flow.verdict.denied) : commentaire, l'anomalie quitte les ouvertes.",
        modal: true,
      },
    ],
  },
  {
    id: "E26",
    title: "Tuiles de l'Accueil",
    shows:
      "Points widget des deux plugins (REQ-017-35) : policy-home (écarts d'inférence ouverts, dernier rapport d'audit) et flow-home (à instruire, échéances J-7, mes demandes en attente), filtrés par les droits, relus par le flux d'événements.",
    views: [
      {
        id: "responsable",
        label: "Responsable sécurité et instructeur",
        description:
          "Les deux tuiles en entier, chaque ligne en lien vers sa vue ; à côté des tuiles du cœur.",
      },
      {
        id: "demandeur",
        label: "Demandeur",
        description:
          "Sans flow.review ni policy.catalogue.read : tuile Flux sans la ligne « à instruire », pas de tuile Politique.",
      },
    ],
  },
  {
    id: "E27",
    title: "Fiches : onglet et actions",
    shows:
      "Points tab et action des deux plugins sur les fiches du cœur (REQ-017-13, 15, 19, 33) : onglet « Flux » d'une adresse et d'un préfixe (flux dont l'objet est extrémité ou dont une extrémité le contient) ; menu d'actions : Vérifier un flux, Attendu d'inférence, Demander un flux.",
    views: [
      {
        id: "onglet",
        label: "Onglet Flux d'une adresse",
        description:
          "192.0.2.14 : flux dont elle est extrémité par son interface, et flux vers le préfixe 192.0.2.0/26 qui la contient, l'extrémité en cause dite ; lien vers le registre filtré.",
      },
      {
        id: "prefixe",
        label: "Onglet Flux d'un préfixe",
        description:
          "192.0.2.64/26 : flux dont il est extrémité et flux vers des adresses qu'il contient ; un flux en attente sans fiche, lien vers son changeset.",
      },
      {
        id: "actions",
        label: "Menu d'actions",
        description:
          "Menu « Actions » d'une adresse ouvert : Vérifier un flux (policy), Attendu d'inférence (policy), Demander un flux (flow) ; une entrée refusée nomme sa permission.",
      },
    ],
  },
];
