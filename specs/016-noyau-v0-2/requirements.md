---
status: implemented
updated: 2026-10-21
---

# 016 — Amendements du noyau pour la v0.2 : exigences

## Résumé

Spec d'amendements : ce que le noyau (specs 003 à 006, 012) et le contrat de module
(ADR 0026) doivent offrir pour que la politique de flux (010) et le registre de flux (011)
s'implémentent sans refonte (garde-fou D31). Chaque exigence cite la REQ ou la clause
amendée et le besoin N qui la motive (tableaux « Exigences envers le noyau » des designs
de la 010 et de la 011). Toutes les modifications sont **additives** : aucun
comportement de la v0.1 ne change pour un appelant qui n'utilise pas les nouveautés.

Décisions : [`decisions.md`](decisions.md) (G1 à G14) ; D11, D13, D23, D28 et D30
amendées, D31, D36.

### Vocabulaire

- **Mesure de module** : nombre calculé par un module sur un changeset, à partir de ses
  sections du différentiel, et comparé à un seuil par une politique d'approbation.
- **Clé d'ordre** : clé d'un événement de l'outbox ; deux événements qui partagent une clé
  sont livrés dans l'ordre d'application.
- **Candidat** : valeur proposée par une source de valeurs attendues.
- **Demandeur** : personne au nom de qui un compte de service agit, sans compte kartenn.
- **Proposition forcée** : changeset enregistré comme proposition à la demande d'un module,
  quels que soient les droits de l'auteur.
- Conventions EARS, « changeset », « proposition », « politique d'approbation » : comme
  dans la 004.

## Exigences

### A. Propriétés (003)

#### REQ-016-1 — Lecture groupée (N9 ; amende REQ-003-14, 15, 26)
kartenn doit rendre, en un appel, la valeur résolue et son explication (nœud d'origine,
chaîne, provenance) de K propriétés pour N objets à (`valid_at`, `as_of`), restreintes
aux objets lisibles par l'appelant et sans valeur sensible non autorisée (REQ-003-27).

Acceptation :
- Pour 512 objets et 6 propriétés sur le jeu de référence (10⁶ objets, 3 ans
  d'historique), p95 < 30 ms au banc.
- Sur 1 000 couples tirés au hasard, le résultat est identique à celui de l'explication
  unitaire (REQ-003-14).
- Un objet illisible est absent du résultat, sans erreur.

#### REQ-016-2 — Liste de valeurs partagée (N7 ; amende REQ-003-3, 10)
Où une déclaration de liste fermée désigne une autre déclaration du même déclarant comme
source de ses valeurs, kartenn doit servir aux deux propriétés les mêmes valeurs, et
refuser la clôture d'une valeur citée par l'une ou l'autre.

Acceptation :
- `policy.declared_environment` emprunte les valeurs de `policy.environment` : une valeur
  créée pour l'une est affectable à l'autre dans le même changeset.
- Une déclaration qui emprunte à une déclaration qui emprunte elle-même est refusée
  (`prop.declaration.values_of`).

#### REQ-016-3 — Type référence externe (N26 ; amende REQ-003-7, 8)
kartenn doit fournir le type de valeur `external_ref` : système (valeur de la liste
fermée administrable `core.external_system`), identifiant (texte NFC, 1 à 256 caractères)
et libellé facultatif (1 à 256 caractères), en valeur unique ou en ensemble ; deux
valeurs sont égales si et seulement si leurs système et identifiant le sont.

Acceptation :
- Deux valeurs de même système et identifiant, de libellés différents, sont égales (filtre
  et « même valeur ») ; de systèmes différents, elles diffèrent.
- Un système clos est refusé à l'écriture ; l'existence de l'identifiant n'est pas
  vérifiée.

#### REQ-016-4 — Contexte d'une source de valeurs attendues (N20 ; amende REQ-003-21)
kartenn doit fournir à une source de valeurs attendues les dates de l'évaluation, les
valeurs résolues des ancêtres de l'objet et l'état de la transaction en cours, et lui
permettre d'évaluer un lot d'objets en un appel.

Acceptation :
- Une règle de source écrite dans un changeset s'applique aux objets créés par ce même
  changeset.
- Une source évalue 10⁴ objets par lots ; le nombre d'appels est celui des lots.

#### REQ-016-5 — Candidats multiples (N28 ; amende REQ-003-22, 23, 24)
kartenn doit accepter qu'une source rende aucun, un ou plusieurs candidats, chacun avec
son explication et sa provenance `source:<id>@<révision>#<détail>` ; il doit n'écrire de
valeur par défaut et ne produire d'écart que lorsqu'il y a exactement un candidat, et
restituer tous les candidats en suggestion.

Acceptation :
- Deux candidats : création sans défaut (REQ-010-45), aucune anomalie d'écart
  (REQ-010-47), la suggestion rend les deux.
- La route de suggestion rend `candidates` ; avec un seul candidat, le champ existant est
  aussi rempli.

#### REQ-016-6 — Axe déclaré par l'administrateur (N25 ; vérifie REQ-003-1)
kartenn doit permettre qu'une propriété de liste fermée déclarée par un administrateur
soit lue par la lecture groupée et porte une obligation de module, comme une propriété de
module.

### B. Changesets et approbation (004)

#### REQ-016-7 — Mesures de module (N5 ; amende REQ-004-40)
kartenn doit permettre à une politique d'approbation de définir des déclencheurs de
module (mesure déclarée par un module, seuil), évalués en simulation et à l'application,
avec la même sémantique que les autres déclencheurs (tous ceux définis doivent
correspondre).

Acceptation :
- Politique `policy.opens ≥ 1` : un changeset qui ouvre une combinaison est mis en
  attente ; un changeset sans ouverture est appliqué.
- Une mesure non déclarée par un module du build est refusée à l'écriture de la politique.

#### REQ-016-8 — Politiques par défaut d'un module (N5 ; précise REQ-004-39)
Quand un module est déployé pour la première fois, kartenn doit installer les politiques
d'approbation par défaut qu'il déclare, marquées d'origine module, et ne jamais les
réinstaller ensuite, même désactivées ou closes.

Acceptation :
- `flow.nonconforming ≥ 1` est active après le premier déploiement de la 011 ; désactivée par
  l'administrateur, elle le reste après un redéploiement.

#### REQ-016-9 — Fin de proposition notifiée au module (N38 ; amende REQ-004-23, ADR 0026)
Quand une proposition qui contient des opérations d'un module est rejetée, retirée ou
expire, kartenn doit appeler le module dans la même transaction, avec le motif, l'identité
qui décide (ou l'expiration) et ses opérations.

Acceptation :
- Le rejet motivé d'une demande de flux crée la non-conformité de la 011 dans la
  transaction du rejet ; si l'écriture du module échoue, le rejet échoue.

#### REQ-016-10 — Proposition forcée (N39 ; amende REQ-004-41)
Quand un changeset contient une opération pour laquelle le module exige une revue,
kartenn doit l'enregistrer comme proposition même si l'auteur détient le droit
d'écriture, en retenant la permission de validation nommée par le module.

Acceptation :
- Un auteur qui détient `flow.write` soumet un flux dont l'issue est « à instruire » :
  proposition en attente, file des réviseurs (REQ-004-43).

#### REQ-016-11 — Validation d'une proposition forcée (N39 ; amende REQ-004-42, 24)
kartenn doit n'accepter la validation d'une proposition forcée que d'une identité qui
satisfait REQ-004-42 et détient la permission de validation retenue sur le périmètre, et
doit permettre à ce validateur d'y joindre des opérations que le module déclare « de
revue », validées et appliquées avec la proposition.

Acceptation :
- Sans `flow.review` : 403. L'auteur : refusé même avec la permission.
- `flow.approve_review` jointe à la validation : un seul changeset appliqué, opérations
  de revue comprises ; une opération non déclarée « de revue » jointe : 422.

### C. Identités et audit (005, 006, 012)

#### REQ-016-12 — Agir pour le compte d'un demandeur (N31 ; amende REQ-005 comptes de service, REQ-006 en-têtes)
Où un compte de service détient `core.identity.act_on_behalf`, kartenn doit accepter sur
une requête d'écriture l'en-tête `Kartenn-On-Behalf-Of` (dictionnaire RFC 9651 : `sub`,
`iss` facultatif, `name` facultatif), enregistrer le demandeur comme auteur d'intention du
changeset et évaluer les droits du seul compte de service.

Acceptation :
- Le changeset porte `on_behalf_of` ; le demandeur, inconnu jusque-là, devient une
  identité `requester` sans droit de connexion.
- Sans la permission, ou depuis une session humaine : 403 `core.identity.act_on_behalf`.
- Le demandeur ne gagne aucun droit : une écriture que le compte de service ne peut pas
  faire est refusée comme sans l'en-tête.

#### REQ-016-13 — Demandeur dans l'audit (N31 ; amende REQ-012-10, REQ-004-33)
kartenn doit inclure `on_behalf_of` dans chaque événement d'audit d'une requête faite pour
le compte d'un demandeur, et pseudonymiser l'identité `requester` comme toute identité.

Acceptation :
- Les événements valident le schéma publié étendu ; après pseudonymisation, ni le sujet ni
  le nom du demandeur ne restent en clair.

### D. Contrat de module et événements (ADR 0026, ADR 0018, ADR 0023, 006)

#### REQ-016-14 — Clés d'ordre (N6 ; amende REQ-004-37, ADR 0018)
kartenn doit livrer dans l'ordre d'application les événements de changesets qui partagent
une clé d'ordre ; les clés d'ordre d'un changeset sont ses domaines et ses clés de verrou
de module.

Acceptation :
- Deux changesets successifs sous le verrou `policy` : leurs événements sont livrés dans
  l'ordre, sans domaine commun.
- Un consommateur v0.1 reçoit les mêmes événements, au même format.

#### REQ-016-15 — Événements de module (N15)
kartenn doit permettre à un module d'émettre, dans la transaction d'un changeset ou d'une
tâche de fond, des événements déclarés, de type `org.kartenn.<module>.<nom>`, livrés comme
ceux du noyau.

Acceptation :
- `org.kartenn.policy.revised` est livré si et seulement si le changeset est appliqué ;
  un changeset annulé ou une simulation n'émet rien.
- Un module qui émet un nom non déclaré échoue au démarrage (registre).

#### REQ-016-16 — Tâches de fond de module (N14 ; amende REQ-006-38)
kartenn doit exécuter les tâches de fond déclarées par un module (nom, période) sous le
bail de l'ADR 0023, une seule instance à la fois, et désactiver sans la supprimer une
tâche que le module ne déclare plus.

Acceptation :
- Trois instances, la tâche `flow.expiry` : une exécution par échéance ; l'instance qui
  l'exécute s'arrête, la tâche s'exécute ailleurs après l'expiration du bail.

## Non-objectifs

- Route REST de lecture groupée (G2) ; jeton délégué RFC 8693 (G10) ; vérification
  d'existence d'une référence externe (G9) ; ordre global des événements (G4).
- Tout ce que les tableaux N disent tenu par le module seul (N27, N30, N32 à N37).

## Questions ouvertes

- Q-016-1 (utilisateur) : validation des choix structurants listés à la fin de
  [`decisions.md`](decisions.md).
