---
status: proposed
date: 2026-11-09
---

# 0043 — Observations : hors référentiel, hors changesets

## Context

La spec 019 (modèle de calcul et Kubernetes) doit répondre à « quelle application avait
l'IP X à l'instant T ? », besoin de l'enquête sur un flux et, plus tard, des flux
constatés (sorte `observed`, v0.4, D31 amendée). Une IP de pod change à chaque
recréation : un cluster de 10⁴ pods renouvelés chaque jour en produit 3,6·10⁶ par an ;
dix clusters sur trois ans dépasseraient dix fois la volumétrie de D2, et 10⁴ changesets
par jour et par cluster entameraient le budget de 50 écritures/s de D14 (019/Q-019-3).

Le référentiel tient ses objets dans l'historique bitemporel (D8, D10, ADR 0005, 0017)
et toute écriture passe par un changeset (D11, ADR 0006) : validation, différentiel,
approbation possible, journal, événement. Une observation ne se refuse pas : la faire
passer par ce pipeline coûterait sans rien contrôler. Un changeset est en outre du temps
système immuable, que la purge à échéance d'un journal violerait (019/Q-019-4, enjeux).

Le journal ne fonde aucun verdict, mais il sert à l'**attribution** en enquête : un
connecteur compromis pourrait faire porter un flux à la mauvaise application. L'option
retenue par l'utilisateur (Q-019-4, A, 2026-11-09) l'est donc **avec garde-fous**.

## Decision

1. **Une catégorie « observations », distincte du référentiel.** Une observation est un
   fait daté lu d'une source (un pod vu avec une IP sur un nœud), portant sa seule
   **période observée** `[début, fin)`. Elle n'est pas versionnée : ni temps système, ni
   lecture `as_of`, ni correction. Elle vit dans une table propre au module qui la
   déclare (pour la 019 : `kube.pod_observation`), hors de `module_object`. D8 et D11 sont
   amendées en ce sens. La catégorie vaut pour le journal des pods de la 019, pour les
   flux constatés et pour toute télémétrie future, chacun dans son module.
2. **Droit propre.** L'écriture se fait sous une permission du module,
   `<module>.observation.record`, la lecture sous `<module>.observation.read` (pour la
   019 : `kube.observation.record`, `kube.observation.read`). Ces noms suivent la
   grammaire de l'ADR 0026 (amendement 5) et de la 024 (M7) : la ressource
   `observation` n'est pas un nom d'action (`catalogue_problems`), aucun ne figure dans
   `RENAMED_PERMISSIONS`, et l'action `write`, retirée, n'est pas employée. Le droit
   d'écrire est réservé au compte de service de la source (pour la 019 : membre du
   groupe du connecteur que désigne le cluster, rôle `kube_connector`) et s'évalue sur le
   périmètre de la source. Aucun rôle livré du lecteur, de l'éditeur ou de l'approbateur
   ne porte l'un ou l'autre ; `*` les couvre.
3. **Écriture par lots, validée à l'entrée.** Une route dédiée du module reçoit un lot
   (pour la 019 : `POST /v1/kube-clusters/{id}/observations`). Le lot est accepté ou
   refusé **en entier** : une ligne hors de la source (pour la 019, une IP hors des
   plages de pods et des adresses de nœuds du cluster, un workload ou un nœud d'un autre
   cluster) refuse le lot (422, Problem Details qui nomment les lignes).
4. **Ajout seul.** Un lot ne fait qu'**ouvrir** des observations (insertion) et
   **fermer** la période d'observations ouvertes (borne haute posée une seule fois).
   Les droits PostgreSQL du rôle de l'application s'y limitent : insertion, et mise à
   jour de la seule borne haute encore ouverte, gardée par un déclencheur ; aucune autre
   mise à jour, aucune suppression ligne à ligne. Seule la purge supprime, par partition
   entière.
5. **Audit par lot, empreinte du contenu.** Chaque lot accepté inscrit dans sa
   transaction l'événement d'audit `observation.batch.recorded` au journal scellé (spec
   012, ADR 0019) : journal, source, nombre d'ouvertures et de fermetures, empreinte
   SHA-256 du **contenu canonique** du lot (ouvertures et fermetures triées). Chaque ligne
   garde l'identifiant du lot qui l'a ouverte et de celui qui l'a fermée. Une tâche de
   fond (ADR 0023) recalcule l'empreinte des lots encore entiers dans la rétention et
   inscrit `observation.batch.altered` (ERROR, criticité haute) pour un lot dont le
   contenu ne correspond plus ; l'empreinte scellée fait foi. Un lot refusé n'écrit rien
   et suit le chemin commun des refus (`authz.access.denied`, `abuse.input.rejected`).
6. **Lecture bornée.** Une lecture d'observations ne rend que celles des sources que le
   lecteur voit, sous `<module>.observation.read` évaluée sur le périmètre de la source :
   la correspondance IP → application dévoile la topologie applicative.
7. **Séparation stricte.** Une observation n'écrit jamais dans le référentiel : aucune
   règle, aucun verdict, aucun différentiel, aucune réévaluation ne la lit. Passer d'un
   fait observé à un objet déclaré se fait par un changeset ordinaire. Un lot n'émet
   aucun événement `changes` (spec 022) et n'entre pas dans l'outbox.
8. **Rétention et exploitation.** Chaque source a une rétention réglable (pour la 019 :
   30 jours par défaut, de 1 à 400, propriété du cluster). La table est **partitionnée
   par temps** : la purge détache et supprime les partitions entièrement hors rétention,
   sans `DELETE` de masse ; une observation encore ouverte n'est jamais purgée, et la
   rétention court depuis la fin de l'observation. Index GiST sur (source, IP, période).
   Au-delà de la rétention, l'enquête n'est plus possible : la lecture rend une réponse
   vide avec le motif `retention`, et l'audit ne garde que la trace du lot (empreinte et
   nombres), pas son contenu.

## Alternatives

- **Un changeset par lot, sur une table non versionnée** (Q-019-4 B) : n'évite pas
  d'amender D10 (un changeset est du temps système immuable, que la purge violerait) ;
  sans purge, l'historique croît sans fin (≈ 3,6·10⁷ opérations par an à dix
  clusters) ; une politique d'approbation large retiendrait les lots.
- **IP de pod comme adresse du noyau** (Q-019-3 B) : dépasse D2 et D14.
- **Aucun journal** (Q-019-3 C) : rend l'enquête impossible.
- **Rétention illimitée** : croissance sans fin, et une donnée de topologie gardée sans
  besoin.

## Consequences

- Une seconde voie d'écriture existe hors du pipeline : elle n'a ni approbation, ni
  correction, ni lecture passée par `as_of`, ni événement par ligne. Les garde-fous
  (2 à 7) en bornent la surface ; ils sont vérifiés par des tests de la tâche 019-6
  (lot hors plages refusé, mise à jour hors fermeture refusée par la base, lot audité
  une fois avec son empreinte, lot altéré détecté, lecture hors périmètre vide).
- Deux événements d'audit s'ajoutent au catalogue (`observation.batch.recorded`,
  `observation.batch.altered`), générés dans `docs/spec/evenements-audit.md` et
  `evenements-audit.schema.json` avec le code (tâche 019-6, synchronisation 019-17).
- Le noyau fournit l'inscription d'audit d'un lot ; le module garde sa table, sa route,
  sa validation et sa purge. Le besoin B4 de la 019 (route hors changeset auditée par
  lot) est tenu par cette ADR.
- Les flux constatés (v0.4) réutiliseront la catégorie : un flux constaté est déclaré
  par un changeset qui cite des observations, jamais créé par elles.
