# 017 — Plugins Politique et Flux : journal des décisions

Brouillon du 2026-10-22, **sans entretien**. Ce journal consigne les choix déjà fixés
par la 007 (shell et plugins), la 010 (politique de flux), la 011 (registre de flux) et
leurs précédents d'interface (008, 009). Les points que ces specs ne tranchaient pas
étaient listés comme questions ; **l'utilisateur les a arbitrées le 2026-10-22**
(Q-017-1 à 7, en fin de journal) et a ajouté une phase de maquettes (U14). La spec est
validée ; trois réponses amendent les specs de module (010/S35, 011/F19, 011/F20).

Cadre : 007/W1 à W18 (en particulier W4, W5, W6, W9, W10, W11, W14) ; 010/S15, S28, S29,
S31, S33, N36 ; 011/F5, F6, F10, F12, F15, F17 ; plugins `web/plugins/vlan` (008, tâche
17) et `web/plugins/machines` (009, tâche 13). Surface REST : celle servie au 2026-10-22
(`docs/spec/interfaces.md`, § API REST ; `web/sdk/src/api/schema.d.ts`).

Renvois : 010/requirements.md, « Non-objectifs » (« plugin de première partie
`web/plugins/policy/` (007/W6), à spécifier avec la 007 ») ; 011/requirements.md,
« Non-objectifs » (« plugin `web/plugins/flow/` (007/W6) »).

## Synthèse
- U1 deux plugins de première partie, `policy` et `flow`, chacun lié à son module.
- U2 écrits avec le seul SDK public, en Custom Elements sans framework (précédent 008, 009).
- U3 points d'extension : les quatre de W5 ; les fiches du shell ne couvrent que les types
  du noyau.
- U4 contexte temporel : W9 ; lectures datées, écritures au présent seulement.
- U5 écriture : W10 ; route de ressource (motif, `If-Match`) ou changeset composé.
- U6 langue : W11 ; textes pédagogiques rendus dans la langue servie par l'API, repli
  signalé.
- U7 droits : le serveur juge ; l'interface masque selon `/v1/me` (REQ-007-20).
- U8 explication réduite : rendue telle quelle, jamais complétée (REQ-010-25).
- U9 matrice : vue de saisie N36 par `PUT /v1/policy-matrix/…` (`policy.matrix.set`).
- U10 instruction : routes des changesets (004) avec les opérations de revue du module.
- U11 échéances : événements émis par le serveur ; l'interface les liste, ne les émet pas.
- U12 accessibilité : WCAG 2.2 AA, axe-core bloquant (W14).
- U13 périmètre : routes servies, plus les amendements 010/S35, 011/F19 et F20 livrés
  avant les écrans qui en dépendent (amendée).
- U14 maquettes des écrans validées par l'utilisateur avant tout développement web.
- Q-017-1 (A) le plugin `policy` simule son ensemble et rend les sections.
- Q-017-2 (A) recherche dans le lisible et identifiant collé.
- Q-017-3 (B) file d'instruction lue par `GET /v1/flows` (011/F19).
- Q-017-4 (B) prolongation instruite d'un flux (011/F20).
- Q-017-5 tous les objets de la 010 en écriture ; inférence comprise.
- Q-017-6 (A) flux d'une machine ou d'une interface par les filtres de la liste.
- Q-017-7 (B) `PUT` de la matrice sous `If-Match` (010/S35).

## U1 — Deux plugins, un par module
- Décision : `web/plugins/policy/` (manifeste `"module": "policy"`) et
  `web/plugins/flow/` (`"module": "flow"`), livrés compilés dans le paquet, publiés
  seulement si leur module figure dans `modules` des capacités (REQ-007-11). Le plugin
  `flow` n'importe rien du plugin `policy` : il lit la politique par l'API (textes,
  services, verdicts).
- Pourquoi : W6 (« Politique et Flux » nommés dans la décision) ; un module retiré
  n'emporte que son plugin ; les modules ont des permissions et des publics distincts
  (équipe sécurité, demandeurs).
- Décidé par : hérité de 007/W6, 010 et 011 (« Non-objectifs »)

## U2 — Forme des plugins
- Décision : TypeScript, Custom Elements sans framework, `@kartenn/sdk` seul (client
  typé, contexte, i18n, brouillon), licence MPL-2.0, catalogues `i18n/fr.json` et
  `i18n/en.json`, tests Vitest par le bac à sable du SDK, construction par `serve.sh`.
- Pourquoi : W7 permet d'embarquer un framework, mais les deux plugins livrés (VLAN,
  machines) n'en ont pas ; garder la même forme garde la même revue et les mêmes budgets.
- Décidé par : précédent 008 (tâche 17) et 009 (tâche 13) ; choix peu coûteux à défaire

## U3 — Points d'extension et fiches
- Décision : vues pleine page (entrée de navigation), onglets et actions sur les fiches
  `address` et `prefix` ; pas d'onglet sur une machine ni une interface : le shell n'a de
  fiche que pour les types du noyau (`web/shell/src/data/collections.ts:TYPES`), les
  objets des modules sont montrés par les vues de leur plugin.
- Pourquoi : W5, liste fermée ; REQ-007-13 (filtre par type d'objet et permission).
- Décidé par : hérité de 007/W5 ; constat sur le code du shell

## U4 — Contexte temporel
- Décision : chaque lecture suit `valid_at` et `as_of` du contexte (verdicts, matrice,
  flux, évaluations, non-conformités, règles) ; les formulaires d'écriture ne s'offrent
  qu'au présent ; un verdict ou une matrice affichés hors présent le disent (bandeau du
  shell).
- Pourquoi : W9 ; REQ-010-16 (verdicts datés), REQ-011-16 (évaluations datées), UC-8.
- Décidé par : hérité de 007/W9

## U5 — Parcours d'écriture
- Décision : un objet de politique ou un flux s'écrit par sa route de ressource (motif
  obligatoire, `If-Match` de la lecture ; 412 → relire en gardant la saisie ; 202 → le
  changeset en attente ; 422 → violations sur les champs) ; un ensemble de changements se
  compose et se simule avant soumission, dans le plugin (Q-017-1, A).
- Pourquoi : W10, REQ-007-25, 26.
- Décidé par : hérité de 007/W10

## U6 — Langue et textes pédagogiques
- Décision : libellés de l'interface dans les catalogues du plugin (mêmes clés en FR et
  EN, testé) ; erreurs de l'API traduites par leur `type` ; textes pédagogiques des
  règles, motifs et conditions lus dans `texts` par langue, affichés dans la langue de
  l'utilisateur, sinon dans la langue par défaut de l'instance (`fr`) avec la langue
  indiquée.
- Pourquoi : W11 ; REQ-010-33 (repli sur la langue par défaut, langue indiquée) ; S29.
- Décidé par : hérité de 007/W11 et 010/S29

## U7 — Droits
- Décision : les vues et actions déclarent la permission sans laquelle le shell les
  masque ; une permission est « détenue » si une liaison la couvre, quelle qu'en soit la
  portée (design 007, « Droits dans l'interface ») ; le serveur reste seul juge : 404
  d'une extrémité invisible, 403 d'une route, `core.object.forbidden` (422) et
  `flow.separation` affichés comme des refus sans perte de saisie.
- Permissions en jeu : `policy.catalogue.read`, `policy.verdict.read`,
  `policy.flow.request`, `policy.{axis,environment,flow_type,service,rule}.admin`,
  `policy.inference.admin` ; `flow.flow.{read,request,review,waive,write}` ;
  `core.changeset.approve` ; `prop.core.owner.write` (exigée par la demande d'un flux,
  écart du design 011).
- Pourquoi : REQ-007-20 ; REQ-010-24, 25 ; REQ-011-17.
- Décidé par : hérité de 007, 010 et 011

## U8 — Explication réduite
- Décision : l'interface rend une réponse `access: reduced` (verdict) ou une évaluation
  de flux réduite (adresses et élément décisif `null`) telle quelle : points repérés par
  leur rang, valeurs d'axes, règles, motifs, conditions ; elle ne tente aucune lecture
  pour compléter ce que le serveur a masqué ; `hidden` (adresses omises) est affiché en
  décompte.
- Pourquoi : REQ-010-25 et S15 (le demandeur n'apprend rien du référentiel qu'il ne lit
  pas) ; REQ-011-17.
- Décidé par : hérité de 010/S15 et 011/F12

## U9 — Vue matricielle
- Décision : la matrice (environnement source × environnement destination, pour un type
  de flux) se lit par `GET /v1/policy-matrix?flow_type=` et s'écrit cellule par cellule
  par `PUT /v1/policy-matrix/{source}/{flow_type}/{destination}` (`policy.matrix.set`,
  sous `If-Match` de la matrice : 010/S35, Q-017-7) :
  états `allow`, `deny`, `conditional` (motifs requis) ou `none` (clôture de la règle) ;
  une cellule absente vaut refus par défaut ; le décompte `outside` des règles hors vue
  est affiché ; une interdiction exige un texte pédagogique (`texts.fr`).
- Pourquoi : REQ-010-34, S28, N36 (vue de saisie alternative, sans objet propre).
- Décidé par : hérité de 010/S28 et du design 010 (N36)

## U10 — Instruction
- Décision : une demande dont l'issue n'est pas automatique est une proposition (202 de
  `POST /v1/flows`) ; l'instructeur (`flow.flow.review`, distinct du demandeur)
  l'approuve par `POST /v1/changesets/{id}/validation` en y joignant les opérations « de revue » du module
  `flow` : `approve_review` (fin obligatoire, au plus 12 mois) et, si le verdict est
  refusé, `waive` ; il la rejette par `POST /v1/changesets/{id}/rejection` avec
  un motif, ce qui crée une non-conformité.
- Pourquoi : REQ-011-21, 22 ; design 011, « Opérations et règles » ; REQ-016-11
  (opérations de revue).
- Décidé par : hérité de 011/F5, F17

## U11 — Échéances
- Décision : les avis J-30 et J-7 sont des événements sortants émis par la tâche
  `flow.expiry` (`org.kartenn.flow.expiring`) ; l'interface n'envoie aucune notification,
  elle liste les flux et dérogations qui arrivent à échéance (`ending_before`) et offre la
  recertification.
- Pourquoi : REQ-011-7, F10 ; écart du design 011 (fenêtres de 30 et 7 jours en
  constantes).
- Décidé par : hérité de 011/F10

## U12 — Accessibilité
- Décision : WCAG 2.2 AA ; chaque parcours Playwright passe axe-core en bloquant ; la
  matrice est une table à en-têtes de lignes et de colonnes, navigable au clavier, dont
  l'état d'une cellule ne se lit jamais par la seule couleur.
- Pourquoi : W14, REQ-007-33.
- Décidé par : hérité de 007/W14

## U13 — Périmètre de l'API
- Décision : les écrans ne s'appuient que sur les routes servies au 2026-10-22 ; un écran
  qui demanderait une route nouvelle, ou un changement de contrat de la 010 ou de la 011,
  devient une question ouverte, pas une tâche.
- Pourquoi : consigne de la demande ; une route nouvelle relève de la spec du module
  (amendement de la 010 ou de la 011), pas d'une spec d'interface.
- Décidé par : consigne de la demande (2026-10-22)
- Amendé le 2026-10-22 (arbitrage de Q-017-3, 4, 5, 7) : les amendements de module
  décidés avec la 017 (010/S35, tâche 010-21 ; 011/F19, tâche 011-12 ; 011/F20, tâche
  011-13) sont des prérequis : une tâche d'écran qui en dépend le dit dans `dépend de :`.
  Un écran qui demanderait une route encore non spécifiée reste une tâche, bloquée sur
  un amendement de module **à rédiger**, nommé dans sa ligne `dépend de :` (aperçu
  d'une règle d'inférence, Q-017-5) ; aucune route n'est inventée par la 017.
- Décidé par (amendement) : l'utilisateur (2026-10-22)

## U14 — Maquettes avant le développement web
- Décision : avant toute tâche d'écran des plugins `policy` et `flow`, une phase de
  maquettes : les écrans des deux plugins (vues, formulaires, matrice, différentiel,
  requête de conformité, demande, file d'instruction, échéances, non-conformités,
  administration du catalogue et de l'inférence) sont maquettés et **validés par
  l'utilisateur** ; toute tâche web de la 017 en dépend. Les tâches d'amendement de la
  010 et de la 011 n'en dépendent pas. Une maquette validée qui contredit le design le
  fait amender avant l'écran.
- Pourquoi : les écrans sont nombreux et écrits à la main sur un schéma générique ;
  corriger une maquette coûte moins qu'un Custom Element livré.
- Décidé par : l'utilisateur (2026-10-22)

## Questions arbitrées

Chaque question porte ses options, la recommandation de Claude et la décision de
l'utilisateur.

### Q-017-1 — Où montrer le différentiel d'un changement de politique
- Constat : la simulation (`POST /v1/changesets?simulate=true`) rend les sections de
  module (`policy.combinations`, `policy.reclassified`, `flow.flows`) avec leurs
  premières lignes, et `GET /v1/changesets/{id}/diff?section=` les pagine ; le brouillon
  et la fiche de changeset du shell n'affichent aucune section de module. UC-7 et
  REQ-010-18 exigent de voir, avant d'appliquer, ce qu'un changement ouvre ou ferme.
- Options : (A) le plugin `policy` compose ses changements (cellules, règles) dans un
  ensemble local, les simule lui-même et rend les sections ; il offre aussi une vue des
  sections d'un changeset en attente (pour l'approbateur) ; (B) amender la 007 : le shell
  rend les sections de module de façon générique dans le brouillon et la fiche de
  changeset ; (C) les deux, A d'abord.
- Recommandation : **A** pour la 017 (aucun changement du shell ni du SDK) ; B comme
  amendement ultérieur de la 007 si d'autres modules en ont besoin.
- Décision : **A**. Le plugin `policy` compose son ensemble en préparation, le simule
  (`POST /v1/changesets?simulate=true`) et rend les sections de différentiel de la
  politique (et `flow.flows` si `flow` est actif) ; il offre la vue des sections d'un
  changeset en attente. Aucun changement du shell ni du SDK ; le rendu générique par le
  shell (B) viendra plus tard, hors de cette spec.
- Décidé par : l'utilisateur (2026-10-22)

### Q-017-2 — Désigner une extrémité sans lire le référentiel
- Constat : un demandeur muni de `flow.flow.request` (ou `policy.flow.request`) peut
  demander un flux vers un réseau qu'il ne lit pas (REQ-011-17), mais `POST /v1/flows` et
  `QUERY /v1/verdicts` désignent une extrémité par son identifiant (UUID) ; les listes
  (`/v1/addresses`, `/v1/prefixes`, `/v1/machines`) ne rendent que ce qu'il lit. L'entité
  de la demande pose le même problème.
- Options : (A) recherche dans les objets lisibles, plus la saisie d'un identifiant collé
  (venu d'un ticket) ; (B) amender la 011 : une résolution d'extrémité par littéral
  (adresse, préfixe, nom de machine) à réponse réduite, sous `flow.flow.request` ;
  (C) réserver le formulaire aux demandeurs qui lisent les extrémités, les autres passant
  par un outil appelant l'API.
- Recommandation : **A** pour la 017 ; **B** à instruire comme amendement de la 011 si
  les demandeurs sans lecture sont nombreux (c'est le cas d'usage UC-1 par excellence).
- Décision : **A**. Le formulaire cherche parmi les objets que le demandeur lit et
  accepte un identifiant collé ; aucun changement du serveur.
- Décidé par : l'utilisateur (2026-10-22)

### Q-017-3 — File d'instruction : les demandes en attente ne sont pas des flux lisibles
- Constat : REQ-011-5 promet l'état `requested`, mais la lecture des flux
  (`crates/kartenn-flow/src/read.rs`) ne connaît que des flux appliqués : une demande en
  attente n'existe que comme proposition (`GET /v1/proposals`, `GET /v1/changesets/{id}`,
  opération `request` du module `flow`). L'évaluation du triage n'est pas exposée comme telle.
- Options : (A) le plugin bâtit la file depuis les propositions en attente, lit
  l'opération de chaque changeset et recalcule l'évaluation par `QUERY /v1/verdicts`
  avec le dossier de la demande (même évaluation que l'enregistrement, REQ-010-38, mais
  à la politique du jour et sous les droits de l'instructeur) ; (B) corriger d'abord
  l'écart dans la 011 : `GET /v1/flows?state=requested` rend les demandes en attente
  avec l'évaluation du triage et le changeset à instruire.
- Recommandation : **B** (l'écart est dans la 011, la file s'en déduit sans lectures en
  N+1) ; A seulement comme repli si B attend.
- Décision : **B**. L'écart de la 011 se corrige d'abord (011/F19, tâche 011-12) :
  `GET /v1/flows?state=requested` sert les demandes en attente avec l'évaluation du
  triage et le changeset à instruire ; la file du plugin la lit (avec `review=pending`
  pour y joindre les prolongations, 011/F20). Pas de repli A.
- Décidé par : l'utilisateur (2026-10-22)

### Q-017-4 — Renouveler un flux instruit à son échéance
- Constat : `PATCH /v1/flows/{id}` ne fait que raccourcir (écart du design 011) ; seule
  une dérogation se recertifie (`POST /v1/flows/{id}/waiver`). REQ-011-6 parle pourtant
  de « prolongation d'un flux » (déclencheur `flow.created`).
- Options : (A) renouveler = nouvelle demande pré-remplie depuis l'ancien flux (nouveau
  triage, l'ancien expire) ; (B) amender la 011 : prolongation par `update` avec
  réévaluation et instruction.
- Recommandation : **A** pour la 017 ; signaler l'incohérence de REQ-011-6 à la 011.
- Décision : **B**. La 011 est amendée (011/F20, tâche 011-13) : `PATCH /v1/flows/{id}`
  prolonge aussi ; la prolongation est triée et instruite comme une demande (plafonds
  F5 et F6, séparation F12) ; REQ-011-6 devient cohérente. Le plugin offre
  « prolonger » sur un flux daté non terminé ; la nouvelle demande pré-remplie reste
  pour un flux terminé ou pour changer d'extrémité ou de service.
- Décidé par : l'utilisateur (2026-10-22)

### Q-017-5 — Périmètre des écrans d'administration de la politique
- Constat : la 010 sert onze collections (axes, valeurs, environnements, types de flux,
  services, règles, motifs, conditions, signaux, rôles exemptés, règles d'inférence) au
  schéma générique `PolicyObject` ; chaque formulaire est à écrire à la main.
- Options : (A) tout en v1 ; (B) v1 = règles, motifs, matrice, services, types de flux,
  environnements en écriture ; axes, valeurs, conditions, signaux, rôles exemptés et
  règles d'inférence en lecture seule (écriture par l'API ou l'import hors ligne,
  REQ-010-35) ; (C) lecture seule partout sauf la matrice.
- Recommandation : **B** ; l'inférence et ses suggestions (UC-5) relèvent du panneau des
  propriétés du shell et d'une spec ultérieure.
- Décision : **tous** les objets de la 010 sont modifiables dans le plugin `policy` :
  axes, valeurs (description), environnements, types de flux, services, règles, motifs,
  conditions d'admissibilité, signaux, rôles exemptés et règles d'inférence (critères,
  exceptions, priorité), chacun selon les opérations que sa collection sert
  (`crates/kartenn-policy/src/rest.rs:COLLECTIONS`). Inférence : la 010 sert déjà
  l'attendu et son explication par nœud (routes transverses du noyau
  `GET /v1/{collection}/{id}/properties/{key}/suggestion` et `…/explanation`) et les
  écarts comme anomalies (`policy.inference.mismatch`) ; le plugin les rend (action sur
  les fiches `address` et `prefix`, vue des écarts). L'aperçu de l'effet d'une règle
  d'inférence avant écriture (nœuds qui correspondraient, écarts ouverts ou fermés) n'a
  pas de route : il reste une tâche, bloquée sur un amendement de la 010 à rédiger
  (section de différentiel ou simulation de l'inférence).
- Décidé par : l'utilisateur (2026-10-22) ; constat des routes servies : Claude

### Q-017-6 — Flux d'une machine ou d'une interface
- Constat : `GET /v1/{machines,interfaces}/{id}/flows` existe, mais il n'y a pas de fiche
  shell pour ces types (U3).
- Options : (A) la vue des flux accepte les filtres `machine=` et `endpoint=interface:…`
  dans son URL ; le plugin `machines` pourra y lier sa fiche plus tard (lien par URL, sans
  import) ; (B) amender la 007 pour des fiches d'objets de module.
- Recommandation : **A**.
- Décision : **A**, comme recommandé.
- Décidé par : l'utilisateur (2026-10-22)

### Q-017-7 — Écritures concurrentes de la matrice
- Constat : `PUT /v1/policy-matrix/…` n'exige pas d'`If-Match` ; deux responsables qui
  modifient la même cellule : le dernier l'emporte. La matrice porte un ETag (sa
  révision).
- Options : (A) le plugin relit l'ETag de la matrice juste avant de soumettre et
  prévient si la révision a changé (contrôle non atomique) ; (B) amender la 010 : `PUT`
  sous `If-Match` de la matrice.
- Recommandation : **A** pour la 017, **B** à proposer à la 010 (choix de contrat).
- Décision : **B**. La 010 est amendée (010/S35, tâche 010-21) : le `PUT` d'une cellule
  exige `If-Match` comme toute écriture (006/H8 : 428 absent, 412 périmé) ; une
  opération `policy.matrix.set` d'un changeset composé porte l'ETag lu dans son membre
  `if_match`. Le plugin n'a plus de contrôle client non atomique.
- Décidé par : l'utilisateur (2026-10-22)
