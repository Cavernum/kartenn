# 004 — Changesets et bitemporel : journal des décisions

Interrogatoire ciblé avant `requirements.md`. Contexte : D8, D10, D11, D33
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; ADR 0005, 0006, 0015 ;
besoins des specs 002 et 003 (différentiel, simulation, approbation, verrous par domaine).

## C1 — Approbation
- Décision : **une approbation** suffit, par une personne qui détient la permission
  d'approbation sur **tout** le périmètre du changeset et qui n'en est **pas l'auteur**
  (séparation des tâches). Revalidation à l'application. **Expiration** configurable
  (30 jours par défaut). L'auteur peut **retirer** son changeset ; l'approbateur peut le
  **rejeter** avec un motif. Un changeset en attente n'est pas modifiable : on le retire
  et on en soumet un autre.
- Rejeté : approbations multiples (modèle de politique plus lourd, évolution possible),
  auto-approbation (aucune valeur de sécurité).
- Décidé par : recommandation acceptée

## C2 — Écritures rétroactives libres
- Décision : toute personne autorisée à écrire sur un périmètre peut viser une période
  applicative qui commence dans le passé. Aucune permission dédiée.
- Garde-fous qui restent : le temps système conserve ce que kartenn affirmait avant la
  correction (audit jamais perdu) ; les droits ordinaires et le droit sur l'effet (003)
  s'appliquent sur toute la période visée ; un changeset rétroactif est **marqué** comme
  tel dans l'audit et filtrable.
- Risque accepté : un prestataire peut réécrire la réalité passée de son périmètre, ce qui
  peut fausser des audits de flux a posteriori (v0.2) ; détectable par le marquage.
- Rejeté : permission dédiée (recommandée, non retenue), interdiction (pas d'import
  d'historique ni de correction de date).
- Décidé par : l'utilisateur

## C3 — Taille bornée et imports par lots
- Décision : un changeset est **borné** en nombre d'opérations (10 000 par défaut,
  configurable) et traité de façon **synchrone**. Un import massif est un **import** :
  job de worker qui ordonne les opérations par dépendance (conteneurs, réseaux, plages,
  adresses, affectations), les découpe en changesets successifs, chacun atomique, avec
  reprise après erreur et rapport. Les verrous de domaine ne sont tenus que le temps
  d'un lot. L'import est **rejouable** (identifiants externes, 002/REQ-002-41).
- Précision : la borne porte sur les **opérations**, pas sur les objets touchés par le
  différentiel ; une seule opération structurelle peut toucher jusqu'à 10⁶ valeurs
  résolues (budget 003).
- Rejeté : changeset illimité asynchrone (domaine verrouillé longtemps, une erreur annule
  tout), synchrone illimité (expirations HTTP, verrous prolongés).
- Décidé par : recommandation acceptée

## C4 — Rétention et données personnelles
- Décision : l'historique des objets est **conservé sans limite** (raison d'être du
  bitemporel ; pas de données personnelles). L'audit référence les personnes par un
  **identifiant interne stable** ; leurs données d'identité (nom, courriel, sujet OIDC)
  sont dans une table séparée, **pseudonymisable** après le départ de la personne et une
  durée configurable, sans casser l'historique.
- Limite : un **motif** saisi en texte libre peut contenir des données personnelles ; la
  pseudonymisation ne le couvre pas (à signaler dans l'interface de saisie).
- Rejeté : tout en clair (effacement RGPD impossible sans casser l'audit), purge après N
  ans (perte de la reconstitution ancienne).
- Décidé par : recommandation acceptée

## C5 — Annulation par changeset inverse
- Décision : kartenn **génère** un changeset inverse qui rétablit l'état antérieur des
  objets touchés par un changeset appliqué, sur la même période applicative. Il est
  proposé, puis soumis comme tout changeset (droits, approbation, différentiel). Si des
  changesets ultérieurs ont modifié les mêmes objets, la génération **signale le
  conflit** au lieu d'écraser. Rien n'est effacé de l'historique.
- Rejeté : pas d'annulation (pénible après un import raté), annulation physique
  (contraire à INV-11).
- Décidé par : recommandation acceptée
