# 003 — Propriétés : journal des décisions

Interrogatoire ciblé avant `requirements.md`. Contexte : D19, D21–D27, D30
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) et A10
([`../002-noyau-adressage/decisions.md`](../002-noyau-adressage/decisions.md)) : tout
attribut d'interprétation est une propriété ; les propriétés sont typées, validées par
leur déclarant, héritables ou non.

## P1 — Déclarants
- Décision : les **modules** déclarent leurs propriétés dans le code (ex. environnement
  pour Politique, FQDN pour DNS) ; les **administrateurs** peuvent en déclarer à
  l'exécution, uniquement avec les types et validateurs intégrés, sans code.
- Pourquoi : les modules comptent sur l'existence et la forme de leurs propriétés ;
  l'équipe réseau reste autonome pour ses besoins descriptifs. Validation stricte
  dans les deux cas (D1).
- Rejeté : modules seulement (un libellé exige une version), administrateurs seulement
  (Politique dépendrait d'une configuration manuelle).
- Décidé par : recommandation acceptée

## P2 — Types de valeurs
- Décision : jeu standard en v0.1 : **liste fermée** (valeurs administrables), **texte**
  (longueur min/max, expression régulière optionnelle), **entier** (bornes),
  **booléen**, **date**, **FQDN** (RFC 1123 §2.1, A-label, minuscules sans point final)
  et **référence** vers un objet d'un type donné. Chaque type existe en **valeur unique**
  ou en **ensemble** (D25). Un module peut ajouter un type avec son validateur ; un
  administrateur ne peut utiliser que les types existants (P1).
- Pas de JSON libre : une valeur non typée contredit la validation stricte (D1).
- Rejeté : minimal (nombres et dates en texte), JSON Schema (indexation, filtrage et
  héritage difficiles, budgets D14).
- Décidé par : recommandation acceptée

## P3 — Arbres d'héritage
- Décision : **un arbre par famille d'objets**, sans saut entre arbres :
  - adressage : domaine → conteneurs → réseau → plage ou adresse (plages et adresses
    sont toutes deux enfants du réseau ; une adresse n'hérite **pas** d'une plage) ;
  - entités : arbre des entités ;
  - sites : arbre des sites.
  Une propriété héritable déclare l'arbre qu'elle suit. Un réseau n'hérite pas de son
  site. Les modules déclareront leurs arbres ou hériteront via l'adresse (D22).
- Pourquoi : un seul parent par nœud, résolution non ambiguë et explicable.
- Amende D19 (chaîne entité → site → domaine → préfixe → adresse → machine).
- Rejeté : chaîne unique D19 (deux parents, priorité à inventer), adressage seulement
  (pas d'héritage dans l'arbre des entités).
- Décidé par : recommandation acceptée (adresse non enfant de plage : choix de
  conception, à confirmer en relecture)

## P4 — Effacement explicite et obligation portée
- Décision : une affectation peut valoir explicitement **« aucune valeur »** ; elle
  remplace l'héritage comme toute affectation (D26) et apparaît dans l'explication.
  Une propriété peut être déclarée **obligatoire sur une portée** (domaine, entité, type
  d'objet) : une écriture qui laisse un objet de la portée sans valeur résolue est
  refusée ; l'effacement explicite y est donc interdit. Généralise D27.
- Rejeté : pas d'effacement (impossible de sortir une sous-branche), obligation au
  domaine seulement (chaque besoin demanderait du code).
- Décidé par : recommandation acceptée

## P5 — Inférence : contrat en 003, moteur en v0.2
- Décision : la 003 spécifie le **contrat générique** d'une source de valeurs attendues
  pour toute propriété : suggestion (autocomplétion), valeur par défaut à l'écriture
  avec provenance (D24), anomalie en cas d'écart entre déclaré et attendu (D23). Une
  source ne lit que le noyau et les propriétés. Le **moteur de règles** (DSL, exceptions
  imbriquées, priorité explicite, D24) est spécifié et livré en v0.2 comme première
  implémentation.
- Rejeté : tout en 003 (alourdit le MVP réseau, D31), tout en Politique (mécanisme
  dupliqué pour d'autres propriétés).
- Décidé par : recommandation acceptée

## P6 — Permission par propriété
- Décision : chaque propriété a sa **propre permission d'écriture** (ex.
  `prop.environment.write`), distincte du droit de modifier l'objet, appliquée sur un
  périmètre (D36) ; une propriété peut aussi exiger l'**approbation** du changeset (D11).
- Pourquoi : une classification de sécurité n'a de valeur que si ceux qu'elle encadre
  ne peuvent pas la changer (un prestataire qui se reclasse en « prod » s'ouvrirait des
  flux, D25).
- Rejeté : droit de l'objet (auto-reclassement), permission par module (trop grossier
  pour les propriétés d'administrateur).
- Décidé par : recommandation acceptée

## P7 — Le droit s'applique à l'effet, pas seulement à l'opération
- Décision : chaque changeset calcule le **différentiel des valeurs résolues** (D34). Si
  la valeur résolue d'une propriété change sur un **objet existant**, que ce soit par
  affectation directe ou indirectement (conteneur inséré, réseau redimensionné, objet
  déplacé sous un autre parent, effacement), l'auteur doit détenir la permission de
  cette propriété (P6) sur ces objets ; sinon le changeset est refusé, ou mis en
  approbation si la propriété l'exige. Un **nouvel** objet qui hérite d'une valeur
  existante n'exige rien.
- Pourquoi : sans cela, une modification de structure permettrait une reclassification
  détournée, ce qui annulerait P6.
- Conséquence de performance : le différentiel doit être calculé dans le budget d'un
  changeset (D14) ; d'où une résolution matérialisée et incrémentale (D30).
- Rejeté : avertir seulement (faille ouverte jusqu'à l'intervention humaine), interdire
  les changements de structure (trop restrictif et incomplet).
- Décidé par : recommandation acceptée

## P8 — Évolution des déclarations
- Décision : les **déclarations** de propriétés et les **valeurs des listes fermées**
  sont des données **bitemporelles** (D10) : on peut consulter la liste des
  environnements à une date donnée. Un changement incompatible avec les valeurs
  existantes (type, bornes, retrait d'une valeur utilisée, passage héritable ↔ non
  héritable qui changerait des valeurs résolues sans affectation explicite) est
  **refusé** en listant les objets concernés. Une valeur de liste fermée se **clôt**, ne
  s'efface pas. Supprimer une propriété revient à la clore avec toutes ses affectations,
  dans un même changeset. Les propriétés déclarées par un module évoluent par migration
  de ce module (dépassé : synchronisation depuis le code, P12).
- Rejeté : modification libre (données invalides en base, contraire à D1), déclarations
  immuables (lourd pour ajouter une valeur).
- Décidé par : recommandation acceptée

## Note — amendement par 004/C6
P6 prévoyait qu'une propriété puisse « exiger l'approbation ». Remplacé par 004/C6 :
aucune déclaration (de module ou d'administrateur) ne porte d'exigence d'approbation ;
l'approbation relève uniquement de politiques facultatives configurées par un
administrateur (déclencheurs : volume, propriété, périmètre, auteur).

## P9 — Matérialiser l'origine, pas la valeur (prototype 003)
- Décision : la résolution matérialisée stocke, par objet et propriété héritable,
  l'**objet d'origine** de l'affectation applicable ; la valeur se lit par jointure sur
  cette affectation. Les propriétés non héritables ne sont pas matérialisées. La
  maintenance suit les quatre règles du prototype 003. Amende l'ADR 0016.
- Pourquoi : matérialiser la valeur dépasse les budgets (16 s pour 7,6×10⁵ objets) ;
  l'origine les tient (0,63 s pour 1,6×10⁴ objets, changement de valeur sans
  réécriture, pages filtrées en 27 à 78 ms).
- Rejeté : matérialiser la valeur (réécriture massive, doublée par l'historique).
- Décidé par : recommandation acceptée (2026-10-14)

## P10 — Budget des grands changements d'origine
- Décision : quand plus de 10⁵ objets changent d'origine dans un changeset, le budget
  du différentiel et de l'application passe à **< 60 s** ; la simulation donne d'abord
  le décompte ; le changeset reste atomique.
- Pourquoi : cas rare (clôture ou effacement très haut dans l'arbre), ≈ 30 µs par
  objet mesurés, hors de portée de < 10 s pour 10⁶ objets.
- Rejeté : imposer un import par lots (casse l'atomicité) ; garder < 10 s (aucune
  solution connue).
- Décidé par : recommandation acceptée (2026-10-14)

## P11 — Pas d'ensemble vide
- Décision : une valeur d'ensemble contient au moins un élément ; l'absence de valeur
  s'exprime par l'effacement (REQ-003-13).
- Pourquoi : deux façons de dire « rien » (ensemble vide, effacement) donneraient deux
  résultats de filtrage et d'obligation différents pour la même intention.
- Rejeté : ensemble vide distinct de l'effacement (ambigu, non demandé).
- Décidé par : recommandation acceptée (2026-10-14)

## P12 — Déclarations de module synchronisées depuis le code
- Décision : le code du module est la seule source de ses déclarations ; le job de
  migration les synchronise par un changeset de l'auteur « système » (ajout, évolution
  contrôlée par P8, clôture), et un refus fait échouer le déploiement.
- Pourquoi : écritures auditées et contrôle de compatibilité automatique ; aucune copie
  SQL à maintenir à côté du code.
- Rejeté : `INSERT` dans les migrations SQL du module (contourne changeset, audit et
  contrôle de compatibilité ; deux sources à garder alignées).
- Décidé par : recommandation acceptée (2026-10-14)

## P13 — Pas de ligne d'origine sans origine
- Décision : `property_origin` ne contient une ligne que là où un objet a une origine
  (affectation propre ou d'un ancêtre, effacement compris) ; l'absence de ligne vaut
  « aucune valeur héritée ».
- Pourquoi : déclarer une propriété héritable ou ajouter un porteur écrirait sinon une
  ligne nulle par objet (≈ 10⁶), et chaque objet créé une ligne par propriété.
- Rejeté : une ligne nulle par objet porteur (filtre « absent » par index simple, au prix
  de ces écritures massives).
- Décidé par : recommandation acceptée (2026-10-14)

## P14 — Budget du changeset touchant jusqu'à 10⁴ objets
- Décision : un changeset dont le différentiel touche jusqu'à 10⁴ objets tient en
  **< 2,5 s**, mesuré sur le changeset entier (pipeline, écriture des origines et de leur
  historique, différentiel, journal, validation) ; le budget de 1 s est abandonné.
- Pourquoi : le banc 003-16 mesure 2,07 s pour 8 315 objets qui changent d'origine, sans
  goulot unique (écriture avec historique ≈ 0,55 s, segments ≈ 0,45 s, différentiel
  ≈ 0,4 s, relecture et journal ≈ 0,2 s) ; le prototype (0,63 s) ne comptait ni
  l'historique ni le journal.
- Rejeté : garder 1 s (refontes au gain estimé à 0,5 s, sans garantie) ; 1 s pour le
  calcul seul (le temps qui compte pour l'utilisateur est celui du changeset entier).
- Décidé par : recommandation acceptée (2026-10-15)
