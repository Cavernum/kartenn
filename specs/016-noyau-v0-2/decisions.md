# 016 — Amendements du noyau pour la v0.2 : journal des décisions

Rédigé le 2026-10-21, sans entretien : l'utilisateur a délégué les choix réversibles ; les
choix structurants (contrat REST, schéma, sécurité, dépendances) sont listés à la fin pour
validation. Cadre : garde-fou D31 (le noyau accueille Politique et Flux sans refonte),
D11 (changesets), D13 et D36 (identités, RBAC), D19 à D30 (D28 et D30 amendées le
2026-10-21) ; ADR 0018 (outbox), 0023 (tâches sous bail), 0026 et son amendement 1
(contrat de module) ; specs 003, 004, 005, 006, 012 ; besoins N et M de
[`../010-politique-flux/design.md`](../010-politique-flux/design.md) § Exigences envers le
noyau et de [`../011-registre-flux/design.md`](../011-registre-flux/design.md) § Exigences ;
expression du besoin [`../010-politique-flux/besoin/`](../010-politique-flux/besoin/).

## Synthèse
- G1 périmètre : N5, N6 (ordre), N7 (valeurs partagées), N9, N14, N15, N20 et N28, N25
  (vérification), N26, N31, N38, N39 ; le reste des tableaux est tenu ou relève du module.
- G2 lecture groupée (N9) : port de service en processus, sans route REST en v0.2.
- G3 déclencheurs de module (N5) : mesures nommées issues des sections du différentiel,
  seuil par politique ; politiques par défaut installées par le module.
- G4 ordre de l'outbox (N6) : clés d'ordre généralisant le domaine.
- G5 liste de valeurs partagée (N7) : une déclaration emprunte la liste d'une autre.
- G6 tâches de fond de module (N14) : déclarées par le module, sous le bail de l'ADR 0023.
- G7 événements de module (N15) : émis dans la transaction, `org.kartenn.<module>.<nom>`.
- G8 sources de valeurs attendues v2 (N20, N28) : contexte de lecture, candidats
  multiples, lot.
- G9 type de valeur référence externe (N26).
- G10 identité agissant pour le compte d'un demandeur (N31).
- G11 crochet de fin de proposition (N38).
- G12 proposition forcée et permission de validation (N39, F5).
- G13 axes déclarés par l'administrateur (N25) : déjà permis, vérifié par un test.
- G14 ordre de travail et parallélisme.

## G1 — Périmètre
- Décision : la 016 livre ce que les tableaux N de la 010 et de la 011 marquent « à
  prévoir » ou « différé v0.2 » : N5, second volet de N6 (ordre de l'outbox), second volet
  de N7 (liste de valeurs partagée), N9, N14, N15, N20, N25 (vérification), N26, N28,
  N31, N38, N39. Les besoins tenus par le module seul (N27, N30, N32 à N37) et ceux déjà
  fournis par l'amendement 1 de l'ADR 0026 (Findings, `CheckContext`, `reads`,
  `prepare`) n'y figurent pas.
- Pourquoi : D31 ; une spec d'amendements unique évite que chaque tâche de la 010 et de
  la 011 modifie le noyau à sa façon.
- Rejeté : amender au fil des tâches 010 et 011 (contrat du noyau dispersé, revues
  mélangées).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-21)

## G2 — Lecture groupée des valeurs résolues (N9)
- Décision : fonction de lecture `resolved_many(objets, clés, valid_at, as_of)` dans le
  stockage et port de service filtré par les droits (objets lisibles, propriétés
  sensibles masquées comme REQ-003-27), qui rend pour chaque couple objet × propriété la
  valeur résolue et son origine (nœud, chaîne, provenance), comme l'explication unitaire.
  Une seule requête ensembliste par appel, sans boucle par objet. Pas de route REST en
  v0.2 : le seul consommateur est la politique, en processus.
- Pourquoi : budget de la 010 (512 objets × 6 propriétés < 30 ms) ; l'explication
  unitaire (`explain.rs`) ferait 3 072 requêtes.
- Rejeté : route `QUERY /v1/properties/resolved` dès maintenant (contrat REST sans
  consommateur externe ; ajout additif possible plus tard).
- Décidé par : Claude (délégation)

## G3 — Déclencheurs d'approbation de module (N5)
- Décision : un module déclare des **mesures** nommées (`policy.opens`, `flow.breaks`…)
  calculées par son gestionnaire à partir de ses sections du différentiel, dans la
  transaction de simulation et d'application. Une politique d'approbation gagne une liste
  `module_triggers` de couples (mesure, seuil) ; elle retient le changeset quand chaque
  déclencheur défini correspond (sémantique de REQ-004-40 inchangée). Créer, modifier ou
  désactiver une politique reste réservé à `core.approval_policy.admin`. Un module peut
  déclarer des politiques **par défaut**, installées une seule fois au premier déploiement
  du module, marquées `origin = module`, désactivables par l'administrateur et jamais
  réinstallées ensuite (F8 : `flow.breaks ≥ 1`).
- Pourquoi : N5, N30, F5, F8 ; REQ-004-39 tient (le noyau n'impose rien ; le module
  propose, l'administrateur dispose).
- Rejeté : déclencheur évalué par du code du module à chaque politique (opaque, non
  explicable) ; politiques par défaut réinstallées à chaque déploiement (écraseraient la
  décision de l'administrateur).
- Décidé par : Claude (délégation) ; principe des politiques par défaut décidé par
  l'utilisateur (011/F8).

## G4 — Ordre de l'outbox hors domaine (N6)
- Décision : chaque événement porte des **clés d'ordre** (`order_keys`), qui
  généralisent `domain_ids` : `domain:<uuid>` pour un domaine, `<module>:<clé>` pour une
  clé de verrou de module (ADR 0026, clause 14). La garantie de REQ-004-37 devient « dans
  l'ordre d'application des changesets qui partagent une clé d'ordre ». Les clés d'un
  changeset sont exactement ses clés de verrou ; l'ordre tient donc pour la même raison
  qu'aujourd'hui (ADR 0015 : verrous pris dans l'ordre, numérotation au commit).
- Pourquoi : la politique et le registre se sérialisent par les verrous `policy` et
  `flow`, hors de tout domaine ; leurs événements doivent rester ordonnés.
- Rejeté : un ordre global (barrière sur toute l'instance, retirée par l'amendement 2 de
  l'ADR 0018) ; un domaine fictif par module (confond périmètre RBAC et ordre).
- Décidé par : Claude (délégation)

## G5 — Liste de valeurs partagée (N7, second volet)
- Décision : une déclaration de type liste fermée peut **emprunter** les valeurs d'une
  autre déclaration (`values_of = <clé>`) du même déclarant (même module, ou
  administrateur pour une déclaration personnalisée). Les valeurs n'existent qu'une fois ;
  créer, renommer ou clore une valeur vaut pour les deux propriétés ; clore une valeur
  citée par l'une ou l'autre est refusé (REQ-003-10). Pas de chaîne d'emprunts.
- Pourquoi : `policy.environment` et `policy.declared_environment` (S3) doivent partager
  le vocabulaire des environnements sans double saisie.
- Rejeté : copie synchronisée (divergence) ; valeurs globales hors déclaration (casse le
  modèle de la 003).
- Décidé par : Claude (délégation)

## G6 — Tâches de fond de module (N14)
- Décision : un module déclare ses tâches (nom, période) ; le déploiement les inscrit
  dans la table des tâches sous le nom `<module>.<tâche>` ; l'ordonnanceur existant
  (ADR 0023) les exécute sous bail et appelle le gestionnaire du module, qui reçoit une
  transaction de lecture-écriture et un journal d'audit. Une tâche qui écrit des objets
  passe par un changeset au nom du compte système de l'instance, comme les autres tâches.
  Une tâche retirée du module est désactivée, jamais supprimée (historique des baux).
- Pourquoi : rapports d'audit (010-16), anomalies d'inférence (010-14), échéances des
  flux (011-8).
- Rejeté : processus séparé par module (déploiement alourdi, D35).
- Décidé par : Claude (délégation)

## G7 — Événements de module (N15)
- Décision : le gestionnaire émet ses événements par la transaction du module
  (`emit(nom, sujet, données)`), écrits dans l'outbox dans la même transaction que les
  données (REQ-004-36), en CloudEvents 1.0 de type `org.kartenn.<module>.<nom>`, avec les
  clés d'ordre du changeset (G4). Une tâche de fond émet de même, avec la clé d'ordre
  `<module>:<tâche>`. Le nom est déclaré par le module ; un nom non déclaré est refusé au
  déploiement du registre.
- Pourquoi : N15, N35 ; un événement publié hors transaction pourrait annoncer une écriture
  annulée.
- Rejeté : événements dérivés calculés par le consommateur (chaque connecteur
  réimplémenterait la politique).
- Décidé par : Claude (délégation)

## G8 — Sources de valeurs attendues, version 2 (N20, N28)
- Décision : le contrat de REQ-003-21 s'étend de façon additive :
  - la source reçoit un **contexte** : dates (`valid_at`, `as_of`), lecture des valeurs
    résolues des ancêtres (N9), lecture de l'état de la transaction en cours (règles du
    module écrites dans le même changeset) ;
  - elle rend **aucune**, **une** ou **plusieurs** valeurs candidates, chacune avec son
    explication et sa provenance `source:<id>@<révision>#<détail>` ; la révision est
    calculée à l'appel, plus fixée à la compilation ;
  - elle peut évaluer un **lot** d'objets en un appel.
  Valeur par défaut (REQ-003-22) : seulement s'il y a exactement un candidat. Écart
  (REQ-003-24) : seulement s'il y a exactement un candidat et qu'il diffère. Suggestion
  (REQ-003-23) : tous les candidats. La route de suggestion gagne un tableau `candidates`
  (ajout), le champ unique existant restant rempli quand il n'y a qu'un candidat.
- Pourquoi : REQ-010-45, 47, 50 à 52 (I6 adouci : égalité non bloquante) ; la règle d'un
  même changeset doit s'appliquer à ses créations.
- Rejeté : un mode « conflit » qui choisit un candidat (D23 : la décision reste humaine).
- Décidé par : Claude (délégation) ; sémantique des candidats décidée par l'utilisateur
  (session inférence, Q10 à Q13).

## G9 — Type de valeur référence externe (N26)
- Décision : nouveau type intégré `external_ref` (REQ-003-7) : système source (valeur de
  la liste fermée `core.external_system`, administrable), identifiant (texte normalisé
  NFC, 1 à 256 caractères), libellé facultatif (cache, 1 à 256). Égalité et opérateur
  « même valeur » sur (système, identifiant), jamais sur le libellé. Aucune vérification
  d'existence en v0.2. Utilisable en valeur unique ou en ensemble, par un module ou un
  administrateur.
- Pourquoi : références de tickets et d'applications (011/F2, Q21) ; axes portant une
  application d'un référentiel externe (Q5).
- Rejeté : texte libre (« même valeur » impossible entre systèmes) ; objet du noyau par
  référence externe (copie d'un autre référentiel, hors D1).
- Décidé par : Claude (délégation)

## G10 — Identité agissant pour le compte d'un demandeur (N31)
- Décision : un compte de service qui détient la permission globale
  `core.identity.act_on_behalf` peut transmettre, sur une requête d'écriture, l'identité
  d'un **demandeur** (émetteur facultatif, sujet, nom d'affichage) par l'en-tête
  `Kartenn-On-Behalf-Of`, dictionnaire de champs structurés (RFC 9651). kartenn enregistre
  le demandeur comme **auteur d'intention** : identité de type `requester`, sans droit de
  connexion, créée à la première mention et pseudonymisable (REQ-004-33). Le changeset et
  chaque événement d'audit qu'il produit portent `on_behalf_of` en plus de l'acteur. Les
  droits évalués restent ceux du compte de service, jamais ceux du demandeur. Sans la
  permission, l'en-tête est refusé (403), jamais ignoré.
- Pourquoi : demandes de flux venant d'un outil de tickets (Q3, Q17) ; traçabilité sans
  créer de comptes.
- Rejeté : jeton délégué (RFC 8693) dès la v0.2 (demande un émetteur qui le prend en
  charge ; envisageable plus tard) ; demandeur dans le seul motif (non structuré, non
  pseudonymisable).
- Décidé par : Claude (délégation) — **à valider** (sécurité, contrat REST).

## G11 — Crochet de fin de proposition (N38)
- Décision : le contrat de module gagne un crochet appelé dans la transaction qui met fin
  à une proposition sans l'appliquer (rejet, retrait, expiration), avec le motif, l'auteur
  de la décision et les opérations du module contenues ; implémentation par défaut sans
  effet. Ce que le module y écrit suit les règles ordinaires (objets du module versionnés).
- Pourquoi : la 011 enregistre une non-conformité au rejet (N37, N38).
- Rejeté : consommateur de l'événement `org.kartenn.changeset.rejected` (hors
  transaction : un rejet sans trace possible).
- Décidé par : Claude (délégation)

## G12 — Proposition forcée et validation par permission (N39, F5)
- Décision : un module peut exiger, pour une opération planifiée, qu'un changeset qui la
  contient soit enregistré comme **proposition** même si l'auteur a le droit d'écriture,
  en nommant la permission de validation (`flow.flow.review`). Valider exige alors
  REQ-004-42 et cette permission sur le périmètre. Le validateur peut ajouter à la
  validation des opérations que le module déclare « de revue » (`flow.approve_review`),
  validées et appliquées avec le reste ; c'est la seule exception à REQ-004-24. L'auteur
  ne valide jamais sa propre proposition.
- Pourquoi : 011/F5 accepté par l'utilisateur (demande non automatique = proposition
  forcée) ; séparation des tâches.
- Rejeté : politique d'approbation dédiée (une approbation n'ajoute pas d'opérations) ;
  second changeset de revue (deux décisions pour un même flux).
- Décidé par : l'utilisateur pour le principe (011/F5, 2026-10-21) ; forme par Claude —
  **à valider** (sécurité).

## G13 — Axes déclarés par l'administrateur (N25)
- Décision : rien à ajouter ; une propriété de liste fermée personnalisée (REQ-003-1) a
  déjà héritage, remplacement, explication et obligation par domaine. Un test vérifie
  qu'elle se lit par la lecture groupée (G2) et accepte une obligation de module (008-20).
- Décidé par : Claude (délégation)

## G14 — Ordre de travail
- Décision : trois files indépendantes, réparties pour éviter les mêmes fichiers :
  (A) propriétés : G5, G9, G8, G2 et G13 ; (B) contrat de module et outbox : G4, G6, G7,
  puis G3 ; (C) changesets et identités : G11, G12, G10. Synchronisation à la fin.
- Décidé par : Claude (délégation)

## Choix structurants à valider par l'utilisateur
1. **Schéma** : colonne `module_triggers` et origine des politiques (G3) ; `order_keys`
   de l'outbox en remplacement de `domain_ids` (G4, amendement 4 de l'ADR 0018) ;
   `values_of` des déclarations (G5) ; stockage du type `external_ref` (G9) ; identités de
   type `requester` et `on_behalf_of` sur le changeset (G10).
2. **Contrat REST** (tous additifs, 006/H12) : `module_triggers` des politiques
   d'approbation ; `candidates` des suggestions ; type `external_ref` dans les valeurs ;
   en-tête `Kartenn-On-Behalf-Of` ; ajout d'opérations à la validation (G12).
3. **Sécurité** : G10 (agir pour le compte d'un demandeur) et G12 (proposition forcée,
   opérations ajoutées par le validateur).
4. **Schéma d'audit** : champ `on_behalf_of` (REQ-012-10, ajout au schéma publié).
5. Aucune dépendance nouvelle ; aucune contradiction avec D1 à D37 relevée. Point de
   cohérence : `specs/011-registre-flux/decisions.md` F10 écrit encore
   `kartenn.flow.expiring` alors que les exigences de la 011 et G7 disent
   `org.kartenn.flow.expiring`.
