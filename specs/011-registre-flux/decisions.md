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

**Révision du 2026-10-21.** L'entretien sécurité de la 010 (Q1 à Q24,
[`../010-politique-flux/besoin/`](../010-politique-flux/besoin/)) a amendé F1, F2, F4 à
F7, F10 à F12, F14, F15 ; F16 à F18 sont ajoutées. L'utilisateur a confirmé le même jour
l'amendement de D30 (validité dérivée).

## Synthèse
- F1 périmètre : registre, triage et instruction, conformité continue, non-conformités,
  différentiel, échéances ; sorte « constaté » réservée (amendée).
- F2 flux : source, destination, service, références externes obligatoires,
  justification facultative, dossier, demandeur, entité, fin facultative (amendée).
- F3 extrémités : adresse, préfixe, interface, machine.
- F4 cycle de vie dérivé du changeset, de la période, de l'évaluation et de la
  dérogation ; validité dérivée pour un flux conforme (amendée).
- F5 approbation : triage automatique ou instruction ; déclencheurs de module (amendée).
- F6 flux refusé : instruction, puis dérogation datée, recertifiée, plafonnée (amendée).
- F7 évaluations matérialisées (verdict, issue, conditions, score), figées à
  l'enregistrement, maintenues dans la transaction (amendée).
- F8 un changement qui rend des flux non conformes à la matrice de flux est accepté, montré, approuvable par défaut.
- F9 différentiel des flux : basculements de verdict et d'issue.
- F10 échéances des seuls flux datés et dérogations ; événements d'issue (amendée).
- F11 requêtes : extrémité, service, état, issue, règle, demandeur, axes (amendée).
- F12 droits : demander, instruire, déroger, maintenir ; séparation des tâches
  (amendée).
- F13 concurrence : verrou `flow` pour les écritures de flux, verrous de ligne pour la
  réévaluation.
- F14 budgets chiffrés, réévaluation de 10⁵ flux comprise (amendée).
- F15 API selon l'ADR 0026 (amendée).
- F16 sorte de flux : `requested`, `observed` réservée à la v0.3.
- F17 non-conformités consultables.
- F18 rien ne se prolonge seul.
- F19 demandes en attente servies par la lecture des flux (`state=requested`), avec le
  triage et le changeset à instruire (amende F4, F11, F15).
- F20 prolongation d'un flux : triée et instruite comme une demande (amende F4, F5 ;
  écart « `update` ne fait que raccourcir » levé).

## F1 — Périmètre
- Décision : la 011 livre le registre des flux déclarés, leur cycle de vie, leur
  conformité à la politique (010) **réévaluée à chaque changement**, le différentiel des
  flux dans les changesets, les échéances et les requêtes. Hors périmètre : import des
  règles de pare-feu et atteignabilité effective (étape 2 de D20), services exposés par
  une machine (D29, plus tard).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 (Q2, Q18, complément après Q9) : s'ajoutent le triage à
  l'enregistrement et l'instruction, les non-conformités consultables (F17), la sorte
  « constaté » réservée (F16) ; campagnes de revue en v0.3 (filtres seulement en v0.2).

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
- Amendé le 2026-10-21 (Q3, Q16, Q21) : **référence externe obligatoire** (une au
  moins : système source en liste fermée, identifiant), justification **facultative** ;
  s'ajoutent les responsables des extrémités et la nature des données (valeurs de l'axe
  de sensibilité, indicateur de données personnelles) ; « pour le compte de » est une
  référence externe typée (N26) quand le demandeur n'a pas de compte kartenn ; la fin de
  période est **facultative** pour un flux enregistré automatiquement (F4), obligatoire
  pour un flux instruit et une dérogation.
- Décidé par (amendement) : l'utilisateur (Q3, Q16)

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
- Amendé le 2026-10-21 (Q2, Q16, D30 amendée) : états `requested`, `rejected`,
  `scheduled` (appliqué, période à venir ; ex-`approved`), `active`, `noncompliant`
  (verdict courant refusé sans dérogation), `waived` (refusé, dérogation valide),
  `expired` (flux daté arrivé à son terme), `withdrawn`. Un flux enregistré
  automatiquement n'a pas de fin : il reste `active` tant que sa synthèse est
  `allowed` (**validité dérivée**) ; s'il bascule, il devient `noncompliant`, sans
  écriture. Seuls les flux instruits et les dérogations expirent.
- Décidé par (amendement) : l'utilisateur (Q16)

## F5 — Approbation
- Décision : un flux suit les mécanismes de la 004 : droit de proposer
  (`flow.flow.propose`, REQ-004-41) et politiques d'approbation de l'administrateur. Le
  module ajoute les déclencheurs `flow.created` (création ou extension d'un flux) et
  `flow.nonconforming` (F8). Seul `flow.nonconforming` est actif par défaut, par une politique créée à
  l'installation du module (F8) ; le noyau n'impose toujours rien (REQ-004-39).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 (Q2, Q3, EB-15, EB-25) : **triage** à l'enregistrement : une
  demande `allowed` d'issue `automatic` est appliquée ; toute autre devient une
  **proposition à instruire**, même si l'auteur détient l'écriture (N39) ; l'instructeur
  (`flow.flow.review`, distinct du demandeur, C1) l'approuve avec une date de fin, ou la
  rejette avec un motif (non-conformité, F17). Déclencheur `flow.waiver` ajouté (octroi
  ou prolongation d'une dérogation), sans défaut.
- Décidé par (amendement) : l'utilisateur (Q2) ; proposition forcée (N39, amende la 004)
  et date de fin de l'instruction (12 mois par défaut, réglable) : l'utilisateur
  (2026-10-21), sur la proposition de Claude

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
- Amendé le 2026-10-21 (Q8, Q11) : une demande refusée n'est plus rejetée d'emblée :
  elle part à l'instruction (F5). La dérogation porte sur ce flux seul : motif, **risque
  accepté**, date de fin obligatoire plafonnée (`[flow] waiver_max_months`, 6 par
  défaut), accordée par `flow.flow.waive` **distinct du demandeur**, **recertifiée** par
  une prolongation qui réévalue le flux contre la politique du jour ; elle ne modifie
  jamais la politique. Un flux autorisé sous réserve n'est pas refusé : il est instruit.
- Décidé par (amendement) : l'utilisateur (Q8, Q11) ; plafond de 6 mois : l'utilisateur
  (2026-10-21), sur la proposition de Claude ; ferme la question ouverte de la durée
  maximale

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
- Amendé le 2026-10-21 (Q2, Q16) : l'évaluation matérialisée porte aussi l'issue, les
  conditions non remplies et le score, avec la révision de politique ; l'évaluation de
  l'enregistrement est **figée** à part (élément décisif, révision, instructeur) ; les
  changements de condition ou de signal font aussi réévaluer ; un changement de règle
  d'inférence ne réévalue aucun flux (D23, S21 close).

## F8 — Changement qui rend des flux non conformes
- Décision : un changeset (environnement, politique, rattachement) qui fait passer des
  flux enregistrés à `denied` est **accepté** ; le différentiel les nomme ; chaque flux
  touché reçoit l'anomalie `flow.verdict.denied` (avertissement), close quand il
  redevient conforme ; une approbation est exigée par défaut : le module installe une
  politique active `flow.nonconforming ≥ 1`, que l'administrateur peut désactiver.
- Pourquoi : bloquer rendrait la politique otage des flux existants (fermer une cellule
  exigerait de clore d'abord tous les flux) ; le différentiel et l'approbation donnent la
  maîtrise (D25).
- Rejeté : refus (otage) ; dérogation automatique (écart masqué) ; anomalie sans
  approbation.
- Décidé par : l'utilisateur (2026-10-20), approbation par défaut comprise

## F9 — Différentiel des flux
- Décision : section du différentiel du changeset : flux dont la synthèse ou un verdict
  bascule, avec avant, après et élément décisif ; décompte d'abord, détail paginé, en
  simulation comme à l'application.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 (Q15) : les basculements d'**issue** y figurent aussi.

## F10 — Échéances
- Décision : une tâche quotidienne émet `kartenn.flow.expiring` à J-30 et J-7 (réglable)
  et `kartenn.flow.expired` ; l'expiration ne fait aucune écriture (F4).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 (Q16, Q24) : ne concerne que les flux datés (instruits) et les
  dérogations ; s'ajoutent `org.kartenn.flow.outcome_changed` (état ou issue changé) et
  `org.kartenn.flow.closed`, pour les outils externes (événements de module, N15, N35).
  Préfixe des types aligné sur `org.kartenn.` (ADR 0026, clause 10).

## F11 — Requêtes
- Décision : filtres par extrémité (exacte, sous-arbre d'un préfixe, machine et ses
  adresses), service, type de flux, état, synthèse du verdict, dérogation, entité,
  demandeur, échéance ; « flux qui touchent cet objet » pour la fiche d'un objet.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 (complément après Q9) : filtres par issue, règle décisive,
  sorte, référence externe et valeur d'axe d'une extrémité (application, propriétaire) ;
  vue des non-conformités (F17).

## F12 — Droits
- Décision : `flow.flow.read`, `flow.flow.write`, `flow.flow.propose`,
  `flow.flow.waive`, portées sur l'**entité** du flux ; créer un flux exige aussi la
  lecture des deux extrémités (pas de flux vers un objet invisible).
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 (Q17) : `flow.flow.propose` est remplacée par
  **`flow.flow.request`** (demander sans lecture des extrémités, explication réduite
  comme 010/S15, lecture de ses propres demandes et non-conformités) ; s'ajoute
  `flow.flow.review` (instruire) ; `flow.flow.write` (maintenance directe) garde
  l'exigence de lecture des extrémités ; instructeur et auteur d'une dérogation
  distincts du demandeur. Un compte de service transmet l'identité du demandeur (N31).
- Décidé par (amendement) : l'utilisateur (Q17)
- Amendé le 2026-10-23 (018/R2, Q-018-1) : les permissions deviennent `flow.read`,
  `flow.request`, `flow.review`, `flow.write` et `flow.waiver.grant` (dérogation) ;
  portées et exigences inchangées. Les anciens noms sont refusés, sans alias.
- Décidé par (amendement) : l'utilisateur (spec 018)

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
- Amendé le 2026-10-21 (Q16) : la réévaluation de 10⁵ flux par un changement de
  politique entre dans le budget exceptionnel de la 003 (< 60 s) ; la demande inclut le
  triage ; page des non-conformités < 150 ms.

## F15 — API
- Décision : `/v1/flows` (collection, routes de ressource, filtres, `QUERY`),
  `/v1/flows/{id}/verdicts` (détail par paire, daté) ; additif à `/v1`.
- Décidé par : Claude (délégation de l'utilisateur, 2026-10-20)
- Amendé le 2026-10-21 : `/v1/flows/{id}/evaluations` (remplace `verdicts`),
  `/v1/flow-nonconformities` ; instruction par les routes des changesets de la 004 ;
  forme des chemins selon l'ADR 0026, clause 9.

## F16 — Sorte de flux
- Décision : chaque flux porte une **sorte** : `requested` (demandé, v0.2) ou
  `observed` (constaté : flux déjà ouvert, lu dans un export de pare-feu ou
  d'infrastructure as code, évalué a posteriori), réservée à la v0.3 et refusée en v0.2.
  Évaluation, filtres et non-conformités l'admettent déjà ; demandeur et références
  sont obligatoires par règle pour `requested` seulement.
- Pourquoi : Q18 et complément après Q9 ; la v0.3 ajoute l'audit a posteriori sans
  refonte.
- Décidé par : l'utilisateur (Q18)

## F17 — Non-conformités consultables
- Décision : une non-conformité est une demande rejetée à l'instruction (motif, règle
  en cause, instructeur : objet `flow_rejection` enregistré au rejet, N38), un flux
  `noncompliant` (dérivé, acquittable par son anomalie) ou un flux `waived`. Toutes sont
  consultables et filtrables (application, demandeur, règle, période), avec le texte
  pédagogique de la règle ; le demandeur voit les siennes.
- Pourquoi : complément de l'utilisateur après Q9 (accompagnement, sensibilisation) ;
  EB-27.
- Rejeté : non-conformités comme simples réponses d'API (aucune trace consultable).
- Décidé par : l'utilisateur (complément après Q9) ; forme : Claude (délégation,
  2026-10-21)

## F18 — Rien ne se prolonge seul
- Décision : aucune écriture automatique : ni prolongation, ni création, ni
  reclassement ; toute écriture passe par un changeset d'un humain ou d'un compte de
  service agissant pour un demandeur identifié. L'état et la validité sont calculés.
- Pourquoi : D1, Q16, EB-18.
- Décidé par : l'utilisateur (Q16)

## F19 — Demandes en attente servies par la lecture (amende F4, F11, F15)
- Constat (2026-10-22, spec 017, Q-017-3) : REQ-011-5 promet l'état `requested` et le
  design le fait venir des propositions en attente, mais la lecture des flux
  (`crates/kartenn-flow/src/read.rs`, `flows`) ne rend que des flux appliqués
  (`Creation::Applied`) : une demande en attente n'existe que comme proposition
  (`GET /v1/proposals`, `GET /v1/changesets/{id}`), et l'évaluation de son triage n'est
  lisible nulle part comme telle. Une file d'instruction devrait lire chaque changeset
  et recalculer l'évaluation (N+1, politique du jour).
- Options : (A) laisser le client bâtir la file depuis les propositions ; (B) servir les
  demandes en attente par `GET /v1/flows?state=requested`, avec leur triage et le
  changeset à instruire.
- Décision : **B**. `GET` et `QUERY /v1/flows` rendent, quand le filtre `state` cite
  `requested`, une ligne par proposition en attente qui porte une opération
  `flow.request` : la demande telle que l'opération complétée la garde, `state:
  requested`, `id: null` (le flux n'existe qu'à l'application) et un membre `review` :
  changeset à instruire, nature (`request`), évaluation du triage (synthèse, issue,
  élément décisif, conditions non remplies, score, avertissements, révision de
  politique, évaluations par paire) lue dans l'opération, **sans recalcul**. Le même
  membre `review` (nature `extension`) décrit une prolongation en attente d'un flux
  appliqué (F20) ; le filtre `review=pending` rend les deux (demandes et prolongations
  en attente), le filtre `changeset=<id>` la ligne d'une proposition. Sans `requested`
  dans `state`, la liste reste celle des flux appliqués (aucune ligne à `id` nul pour
  les clients actuels). Droits : lecture de l'entité (`flow.flow.read`), instruction
  (`flow.flow.review` sur l'entité) ou demandeur de la ligne (`flow.flow.request`) ;
  évaluation réduite (REQ-010-25) pour qui ne lit pas les extrémités. Datation : une
  demande est `requested` à `as_of` si sa proposition était soumise et non terminée à
  cette date (horodatages de la 004) ; `valid_at` ne l'écarte pas.
- Pourquoi : l'écart est dans la 011 (REQ-011-5 non tenue) ; la file d'instruction se
  lit en une requête, avec l'évaluation qui a décidé du triage, sous les droits du
  serveur.
- Rejeté : recalculer l'évaluation à la lecture (elle peut différer du triage et coûte
  une évaluation par ligne) ; état stocké (F4).
- Décidé par : l'utilisateur (2026-10-22, Q-017-3, option B) ; forme de la ligne
  (`id` nul, membre `review`), filtres `review` et `changeset`, lignes en attente
  seulement sur demande explicite : Claude, choix de conception

## F20 — Prolongation d'un flux (amende F4, F5 ; REQ-011-4, 6)
- Constat (2026-10-22, spec 017, Q-017-4) : `PATCH /v1/flows/{id}` (`update`) ne fait
  que raccourcir la période (écart au design, « le code fait foi ») ; seule une
  dérogation se recertifie. REQ-011-6 et F5 nomment pourtant la « prolongation d'un
  flux » (déclencheur `flow.created`), et F4 « renouveler = prolonger la période par
  changeset ».
- Options : (A) renouveler par une nouvelle demande pré-remplie (nouveau flux, l'ancien
  expire) ; (B) amender la 011 : prolongation par `update`, réévaluée et instruite.
- Décision : **B**. Un `update` dont la fin est postérieure à la fin courante, ou qui
  retire la fin d'un flux daté, est une **prolongation** ; avancer la fin reste un
  raccourcissement immédiat. La prolongation est **triée comme une demande**
  (REQ-011-21) : le flux est évalué, avec son dossier, sur la portion ajoutée et contre
  la politique du jour ; synthèse `allowed` et issue `automatic` → appliquée (200) ;
  sinon **proposition à instruire** (202), même si l'auteur détient `flow.flow.write`
  (N39). L'instructeur l'approuve par `approve_review` : fin obligatoire, au plus
  `review_max_months` après la date de la décision (F5) ; si la portion ajoutée est
  refusée, une dérogation (`waive`) au plus `waiver_max_months` après son octroi et pas
  au-delà de la fin du flux (F6) ; il la rejette avec un motif (non-conformité, F17 ; le
  flux garde sa fin). Séparation des tâches (F12) : l'instructeur et l'auteur de la
  dérogation sont distincts du demandeur du flux et de l'auteur de la prolongation (ou
  de son « pour le compte de »). Droit de demander une prolongation : `flow.flow.request`
  (explication réduite) ou `flow.flow.write` sur l'entité. Un flux arrivé à sa fin
  (`expired`, `withdrawn`) ne se prolonge pas (`flow.update.ended`) : il se renouvelle
  par une nouvelle demande. La prolongation compte pour le déclencheur `flow.created`.
- Pourquoi : un flux instruit garde son identité et son historique à l'échéance, au lieu
  d'un nouveau flux par an ; l'instruction et ses plafonds s'appliquent comme à la
  demande, rien ne se prolonge seul (F18).
- Rejeté : prolongation directe sous `flow.flow.write` (contournerait l'instruction et
  les plafonds) ; prolongation d'un flux expiré (son historique dirait « actif » sur une
  période où il ne l'était pas).
- Décidé par : l'utilisateur (2026-10-22, Q-017-4, option B, plafonds F5 et F6 et
  séparation F12 respectés) ; plafond compté depuis la décision, refus d'un flux
  terminé, retrait de la fin traité comme prolongation : Claude, choix de conception

## F21 — Extrémités sans machine ; ouvert et conforme (amende F3 ; REQ-011-2, 10 ; REQ-010-13)
- Constat (2026-10-26, relecture de la maquette E11, spec 024) : une machine comme
  extrémité, développée en toutes ses adresses, ne correspond pas à la pratique : une
  ouverture se fait sur une adresse, une interface ou un sous-réseau. Et tester une
  connexion mêlait deux questions : la paire est-elle **ouverte** (un flux du registre la
  couvre) et est-elle **conforme** (la matrice l'autorise) ?
- Décision : une extrémité est une adresse, un préfixe ou une interface ; une machine
  est refusée (`flow.endpoint.machine` au registre, `policy.endpoint.machine` à la
  requête de conformité). La machine reste un contexte (l'interface d'une machine) et
  un point d'entrée de lecture (REQ-011-14, 15 : les flux de ses interfaces et adresses).
  Le test d'une connexion (REQ-011-26) répond séparément « ouvert » (flux `active` ou
  `waived` qui couvrent la paire et le service) et « conforme » (verdict), et se résume
  en quatre cas.
- Pourquoi : une règle de pare-feu vise des adresses ou des réseaux ; un flux déclaré
  sur une machine ouvrirait sans le dire ses interfaces d'administration. Un flux peut
  être ouvert et non conforme (dérogation, politique changée) ou conforme sans être
  ouvert (à déclarer) : les confondre cache l'un des deux.
- Rejeté : garder la machine au contrat et la masquer dans l'interface (deux vérités).
- Décidé par : l'utilisateur (2026-10-26, arbitrages « Retirer la machine » et
  « Couvert par un flux du registre ») ; codes et migration : tâche 14.
