# 027 — Paramètres par déclarant et flux vus depuis une extrémité : décisions

## Synthèse

| Id | Décision | Par |
|---|---|---|
| P1 | Un flux se lit dès qu'on lit une de ses extrémités (entrants et sortants) | utilisateur |
| P2 | L'extrémité hors périmètre se montre par sa valeur seule | utilisateur |
| P3 | La lecture par une extrémité exige encore `flow.read`, sur n'importe quel périmètre | Claude, à confirmer |
| P4 | Droits des paramètres par déclarant, en changeset | utilisateur |
| P5 | Ces permissions sont globales | Claude |
| P6 | La sensibilité de toute propriété reste au cœur | Claude, à confirmer |
| P7 | Les règles de la politique ne sont pas des paramètres | Claude, à confirmer |
| P8 | Un écran Paramètres, un onglet par déclarant | Claude |

## P1 — Lecture d'un flux par une extrémité (amende REQ-011-17)
- Constat (2026-10-28) : l'utilisateur veut qu'une personne voie les flux qui partent de
  ses machines même sans lire la destination.
- Options : (A) une extrémité lue suffit, entrants et sortants ; (B) la source seulement.
- Décision : **A**.
- Pourquoi : le responsable d'une machine a besoin des deux sens pour la configurer
  (pare-feu local, supervision) ; la fiche machine d'E11 montre Entrants et Sortants.
- Décidé par : l'utilisateur (2026-10-28, « Une extrémité lue suffit »).

## P2 — Extrémité hors périmètre
- Options : (A) valeur seule (adresse ou préfixe ; adresses d'une interface) ;
  (B) entièrement masquée.
- Décision : **A**, avec `readable: false`, sans identifiant, nom, machine, propriété ni
  valeur d'axe ; explication réduite comme REQ-010-25.
- Pourquoi : la valeur est ce qu'il faut pour configurer un filtrage ; elle ne donne pas
  accès à l'objet. Le demandeur d'un flux voit déjà ces valeurs (`flow.request`).
- Décidé par : l'utilisateur (2026-10-28, « Valeur seule »).

## P3 — `flow.read` toujours exigée
- Décision : la lecture par une extrémité s'ajoute aux lecteurs de REQ-011-17 pour qui
  détient `flow.read` sur au moins un périmètre ; sans `flow.read`, aucun flux.
- Pourquoi : `flow.read` reste la porte du module (un rôle sans le module Flux n'en voit
  rien) ; c'est la lecture de l'extrémité qui fixe *lesquels*. Le rôle Lecteur livré la
  contient déjà.
- Rejeté : lecture d'une extrémité seule, sans `flow.read` (le module deviendrait visible
  à tout lecteur du référentiel).
- Décidé par : Claude ; à confirmer par l'utilisateur.

## P4 — Droits des paramètres par déclarant, en changeset
- Constat : `core.property.admin` (global) couvre toute déclaration et toute valeur de
  liste fermée, y compris celles des modules ; la Politique a ses `policy.*.admin`.
- Options : (A) `<déclarant>.settings.{edit,apply,approval.self}` ; (B) `prop.<clé>.admin`
  par champ ; (C) `<déclarant>.settings.admin` sans changeset.
- Décision : **A**. Remplace `core.property.admin` et `policy.{axis,environment,
  flow_type,service}.admin`.
- Pourquoi : une équipe de sécurité maintient la partie qui la concerne sans pouvoir
  casser le cœur ; même grammaire que les objets (024, M7).
- Décidé par : l'utilisateur (2026-10-28, « Par déclarant, en changeset »).

## P5 — Permissions globales
- Décision : les paramètres sont globaux (une valeur de liste vaut partout) ; leurs
  permissions ne s'accordent que sur le périmètre global, comme `core.property.admin`.
- Décidé par : Claude (conséquence de la nature des paramètres).

## P6 — La sensibilité reste au cœur
- Décision : rendre une propriété sensible ou non, quel que soit son déclarant, exige
  `core.settings.*`.
- Pourquoi : la sensibilité décide qui lit une valeur (`prop.<clé>.read`) ; c'est une
  décision de contrôle d'accès, pas un paramètre métier du module.
- Décidé par : Claude ; à confirmer par l'utilisateur.

## P7 — Les règles de la politique ne sont pas des paramètres
- Décision : `policy.rule.admin` (règles, motifs, conditions, signaux, rôles exemptés)
  reste à part ; la 027 ne la renomme pas.
- Pourquoi : les règles sont le contenu de la matrice, pas des valeurs possibles ; leur
  passage à `edit`/`apply`/`approval.self` relève d'une révision de la 010.
- Décidé par : Claude ; à confirmer par l'utilisateur.

## P8 — Un écran, un onglet par déclarant
- Décision : écran « Paramètres » dans l'espace Administration, un onglet par déclarant,
  lecture seule sans la permission (code nommé), modifications au brouillon.
- Pourquoi : la frontière de droits se voit à l'écran ; mêmes motifs que E4, E5.
- Décidé par : Claude (maquette, réversible).
