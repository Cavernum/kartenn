# 011 — Registre de flux : journal des décisions

Rédigé le 2026-10-20 sans entretien : l'utilisateur a délégué les choix de cette spec.
Chaque décision retient la recommandation la plus standard ; elles restent à relire avant
l'implémentation (v0.2, D31).

Cadre : D30 (registre de flux déclarés), D20 (conformité puis atteignabilité), D22
(résolution par adresse), D25 (différentiel des flux), D28, D29, D33 de
[`../001-cadrage/decisions.md`](../001-cadrage/decisions.md) ; spec 010 (politique,
verdicts, différentiel de politique) ; 003 (différentiel des propriétés) ; 004
(changesets, propositions, approbations) ; 009 (machines et interfaces, en cours) ;
contrat de module (ADR 0026, en cours).

## Synthèse
- F1 périmètre : registre, cycle de vie, conformité continue, différentiel, échéances.
- F2 flux : source, destination, service, justification, demandeur, entité, période.
- F3 extrémités : adresse, préfixe, interface, machine.
- F4 cycle de vie dérivé du changeset et de la période, sans état stocké.
- F5 approbation : par les mécanismes de la 004, un déclencheur de module en plus.
- F6 enregistrement d'un flux non conforme : refusé, sauf dérogation explicite.
- F7 verdicts matérialisés par flux et période, maintenus dans la transaction.
- F8 un changement qui casse des flux est accepté, montré et approuvable.
- F9 différentiel des flux : basculements de verdict avec l'élément décisif.
- F10 échéances : événements avant expiration, aucune écriture automatique.
- F11 requêtes : par extrémité (sous-arbre, machine), service, état, verdict.
- F12 droits : par entité du flux, plus lecture des extrémités.
- F13 concurrence : verrou `flow` pour les écritures de flux, verrous de ligne pour la
  réévaluation.
- F14 budgets chiffrés.
- F15 API sous `/v1/flows`.

## F1 — Périmètre
- Décision : la 011 livre le registre des flux déclarés, leur cycle de vie, leur
  conformité à la politique (010) **réévaluée à chaque changement**, le différentiel des
  flux dans les changesets, les échéances et les requêtes. Hors périmètre : import des
  règles de pare-feu et atteignabilité effective (étape 2 de D20), services exposés par
  une machine (D29, plus tard).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F2 — Contenu d'un flux
- Décision : un flux a une extrémité source, une extrémité destination, **un** service
  du catalogue (010), une justification (texte non vide), un **demandeur** (identité
  kartenn, par son identifiant, plus un texte facultatif « pour le compte de »), une
  **entité** porteuse (ancre RBAC), une référence externe facultative (numéro de
  demande) et une **période de validité** en temps applicatif (D30).
- Pourquoi : D30 ; un service unique rend le verdict non ambigu (D29) ; l'identité par
  identifiant reste pseudonymisable (REQ-004-33).
- Rejeté : plusieurs services par flux (un verdict par flux devient une synthèse ; on
  crée plusieurs flux dans un changeset) ; demandeur en texte libre seul (donnée
  personnelle copiée, non pseudonymisable).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F3 — Extrémités
- Décision : adresse, préfixe, interface ou machine (009) ; pas de plage (pas
  d'environnement, 010/S3). Un hôte hors du référentiel (Internet, partenaire) se
  modélise comme un préfixe ou une adresse d'un domaine dédié.
- Pourquoi : mêmes extrémités que la requête de conformité (REQ-010-13) ; aucune
  extrémité « libre » qui échapperait au déclaratif (D1).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F4 — Cycle de vie
- Décision : l'état d'un flux est **dérivé**, jamais stocké : `requested` (changeset de
  création en attente : proposition ou approbation, 004) ; `rejected` (ce changeset
  rejeté, retiré ou expiré) ; `approved` (appliqué, période à venir) ; `active` (période
  en cours) ; `expired` (période close à son terme) ; `withdrawn` (clos avant son
  terme). Renouveler = prolonger la période par changeset.
- Pourquoi : D30 demande un cycle par changesets ; le dériver du changeset et du temps
  applicatif évite un second automate qui pourrait diverger.
- Rejeté : colonne d'état mise à jour par une tâche (écriture sans changeset, dérive).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F5 — Approbation
- Décision : un flux suit les mécanismes de la 004 : droit de proposer
  (`flow.flow.propose`, REQ-004-41) et politiques d'approbation de l'administrateur. Le
  module ajoute les déclencheurs `flow.created` (création ou extension d'un flux) et
  `flow.breaks` (F8). Aucune approbation n'est imposée par défaut (REQ-004-39).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F6 — Flux non conforme à l'enregistrement
- Décision : créer ou prolonger un flux dont un verdict est `denied` sur une partie de
  sa période est **refusé** (`flow.verdict.denied`, erreur), sauf **dérogation** :
  motif obligatoire, date de fin obligatoire, permission `flow.flow.waive`. Une
  dérogation est visible dans toutes les lectures et dans le rapport.
- Pourquoi : la validation bloquante est la règle (D1) ; la dérogation documente un écart
  réel plutôt que de le cacher.
- Rejeté : enregistrement libre avec avertissement (registre qui contredit la politique
  sans trace de décision).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F7 — Verdicts matérialisés
- Décision : chaque flux a ses verdicts (par paire d'adresses) et sa synthèse
  matérialisés par segment de temps applicatif, versionnés en temps système, recalculés
  dans la transaction de tout changeset qui touche : le flux, l'environnement résolu
  d'une adresse d'extrémité (différentiel 003), la politique (différentiel 010), les
  rattachements des machines et interfaces d'extrémité (009).
- Pourquoi : D30 (index incrémental pour les différentiels dans les budgets) ;
  « flux non conformes au 1er mars » se lit sans recalcul.
- Rejeté : calcul à la lecture (aucun différentiel possible sans tout recalculer).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F8 — Changement qui casse des flux
- Décision : un changeset (environnement, politique, rattachement) qui fait passer des
  flux enregistrés à `denied` est **accepté** ; le différentiel les nomme ; chaque flux
  touché reçoit l'anomalie `flow.verdict.denied` (avertissement), close quand il
  redevient conforme ; le déclencheur `flow.breaks` permet à l'administrateur d'exiger
  une approbation.
- Pourquoi : bloquer rendrait la politique otage des flux existants (fermer une cellule
  exigerait de clore d'abord tous les flux) ; le différentiel et l'approbation donnent la
  maîtrise (D25).
- Rejeté : refus (otage) ; dérogation automatique (écart masqué).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F9 — Différentiel des flux
- Décision : section du différentiel du changeset : flux dont la synthèse ou un verdict
  bascule, avec avant, après et élément décisif ; décompte d'abord, détail paginé, en
  simulation comme à l'application.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F10 — Échéances
- Décision : une tâche quotidienne émet `kartenn.flow.expiring` à J-30 et J-7 (réglable)
  et `kartenn.flow.expired` ; l'expiration ne fait aucune écriture (F4).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F11 — Requêtes
- Décision : filtres par extrémité (exacte, sous-arbre d'un préfixe, machine et ses
  adresses), service, type de flux, état, synthèse du verdict, dérogation, entité,
  demandeur, échéance ; « flux qui touchent cet objet » pour la fiche d'un objet.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F12 — Droits
- Décision : `flow.flow.read`, `flow.flow.write`, `flow.flow.propose`,
  `flow.flow.waive`, portées sur l'**entité** du flux ; créer un flux exige aussi la
  lecture des deux extrémités (pas de flux vers un objet invisible).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F13 — Concurrence
- Décision : les opérations sur les flux prennent le verrou de module `flow` (après
  domaines et `policy`, ordre canonique) ; la réévaluation déclenchée par un changeset
  d'un domaine verrouille seulement les lignes de verdict des flux touchés, dans
  l'ordre des identifiants.
- Pourquoi : un changeset de domaine ne doit pas sérialiser toute l'instance derrière le
  verrou `flow` ; l'ordre global (domaines, `policy`, lignes de flux) évite les
  interblocages.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F14 — Budgets
- Décision (p95 serveur, 10⁶ objets, 10⁵ flux, politique du banc 010) : créer un flux
  machine × machine 16 × 16 < 300 ms ; réévaluer 10⁴ flux dans un changeset < 3 s
  ajoutées ; page filtrée de flux < 150 ms ; réévaluation complète en tâche de fond
  < 5 min.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)

## F15 — API
- Décision : `/v1/flows` (collection, routes de ressource, filtres, `QUERY`),
  `/v1/flows/{id}/verdicts` (détail par paire, daté) ; additif à `/v1`.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
