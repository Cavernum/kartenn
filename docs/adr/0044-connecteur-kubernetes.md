---
status: proposed
date: 2026-11-09
---

# 0044 — Connecteur Kubernetes : processus distinct, lecture seule, droits vérifiés

## Context

La spec 019 reflète dans kartenn les objets d'un cluster Kubernetes (nœuds, namespaces,
workloads, services, ingress) comme **objets miroirs** que seul le connecteur du cluster
écrit (D1 amendée, Q-019-2), et alimente le journal d'observation des pods (ADR 0043).
D7 range les connecteurs parmi les clients de l'API REST, hors processus ; l'ADR 0023
permet aussi des tâches de fond dans le serveur. Lister, surveiller (watch), reprendre
après `410 Gone` et paginer suivent les conventions de l'API Kubernetes
([`docs/reference/kubernetes-calcul.md`](../reference/kubernetes-calcul.md) § API). Un
compte qui peut lire les `secrets` d'un cluster en lit le contenu par `list` et `watch`
(même note, § Sécurité d'accès). Côté kartenn, les droits ne se lient qu'à des groupes
(025/G1 à G3) ; les périmètres sont l'entité, le site, le domaine et le préfixe (D12,
ADR 0026 clause 4).

## Decision

1. **Processus distinct, client REST** (Q-019-6 A). Le connecteur est la sous-commande
   `kartenn kube-sync` de l'image unique (D35 amendée), crate `kartenn-kube-sync`. Il
   n'ouvre aucune connexion à PostgreSQL ; ses écritures sont des changesets
   (opération `kube.sync`) et des lots d'observations, par l'API `/v1`, avec un client
   tiré du contrat OpenAPI. Un processus sert un ou plusieurs clusters.
2. **List puis watch, liste périodique en repli** (X8). Par type suivi : liste par pages
   (`limit`, `continue`), puis watch depuis la `resourceVersion` de la liste avec
   signets ; reprise à la dernière version reçue après une coupure ; reliste sur
   `410 Gone`. La `resourceVersion` est rendue telle quelle et jamais ordonnée. Chaque
   liste complète rapproche l'état de kartenn (clôture, absence, X9). Sans watch, listes
   complètes périodiques (5 min par défaut), signalées dans les métriques.
3. **Lots idempotents, fenêtre du cluster.** Les différences, projetées sur les seuls
   champs suivis (X4), sont regroupées par fenêtre en un changeset dont
   l'`Idempotency-Key` dérive du cluster, du type et de la fenêtre. La fenêtre (10 s ou
   500 opérations par défaut) est une propriété du `kube.cluster`, écrite par
   changeset, et non un réglage local du connecteur.
4. **Lecture seule côté cluster.** Le connecteur n'envoie au cluster que `get`, `list`,
   `watch` et des revues d'accès sur ses propres droits. kartenn livre un manifeste de
   ClusterRole limité à `get`, `list`, `watch` sur les ressources lues, sans
   sous-ressource, sans `secrets` ni `configmaps`. Au démarrage, le connecteur interroge
   ses droits effectifs et **refuse de démarrer** s'il peut écrire une ressource ou lire
   les `secrets`, sauf option explicite `allow_excess_rights`, consignée dans ses logs et
   dans l'audit de kartenn.
5. **Identifiants.** Jeton lié à durée limitée ou certificat client pour le cluster,
   secret du compte de service pour kartenn, lus de fichiers relus à leur
   renouvellement ; CA du serveur d'API obligatoire ; aucun identifiant dans les logs ni
   les traces. Le connecteur ne garde, ne transmet ni ne journalise que les champs
   suivis (minimisation : variables d'environnement et annotations ne sortent pas).
6. **Confinement côté kartenn, par groupe.** Le connecteur s'authentifie par un compte de
   service (D13, 005/K8), membre direct d'un groupe kartenn propre au cluster (025/G2) ;
   le cluster désigne ce groupe (`connector_group`). Le groupe est lié au rôle livré
   `kube_connector` sur le périmètre des **domaines désignés** du cluster (pods,
   services, adresses externes) ; aucune liaison nominale. Pour que ce périmètre couvre
   le cluster, les types `kube.node`, `kube.namespace` et `kube.workload` déclarent un
   ancrage de domaine sur le domaine des pods, `kube.service` sur celui des services,
   `kube.ingress` sur celui des adresses externes (à défaut, des services) (ADR 0026
   clause 4). Comme deux clusters peuvent partager un domaine (Q-019-5), deux règles
   d'erreur bornent le connecteur à son cluster et à la synchronisation :
   `kube.object.managed` (un champ suivi ne s'écrit que par un compte de service membre
   du groupe du connecteur du cluster de l'objet) et `kube.connector.direct` (un
   changeset d'un tel compte ne porte que des opérations `kube.sync`, donc seulement ce
   que le module planifie : objets miroirs, réseaux des plages de pods, adresses `vip`,
   axe `kube.namespace`, valeurs d'axes tirées des labels admis).
7. **Rôle livré par un module.** Le contrat de module (ADR 0026, clause 8) s'étend d'un
   ajout additif : un module peut livrer un rôle, synchronisé comme les rôles livrés du
   noyau (K12), dont les permissions sont les siennes ou celles du noyau. `kube_connector`
   contient `kube.cluster.read`, `kube.{node,namespace,workload,service,ingress}.{read,
   edit,approval.self}`, `core.domain.read`, `core.prefix.{read,edit,approval.self}`,
   `core.address.{read,edit,approval.self}`, `prop.*.edit`, `prop.*.approval.self` et
   `kube.observation.record`. Une politique d'approbation peut toujours retenir un lot
   (par exemple une valeur de label qui ouvre des flux) : il attend alors sa décision.
8. **Aucune écriture dans le cluster.** Aucune fonction du connecteur n'écrit dans un
   cluster ; la lecture des NetworkPolicy et la production de manifestes sont des
   évolutions, par des specs ultérieures (Q-019-10).

## Alternatives

- **Tâche de fond du serveur** (Q-019-6 B) : chaque instance porterait les identifiants
  de tous les clusters, contre D7.
- **Import périodique seul** (Q-019-6 C) : retard de N minutes et liste complète à
  chaque passe ; gardé comme repli.
- **Liaison au compte du connecteur** : contraire à 025/G1 (pas de droit nominal).
- **Périmètre « objet de module »** (lier le rôle au cluster lui-même) : n'existe pas
  (ADR 0026 clause 4) ; l'ajouter changerait le RBAC du noyau pour un seul module.
- **Écriture directe dans le cluster, sur option** (Q-019-10 B) : rompt « lecture seule
  par défaut » et exige une nouvelle décision sur D1.

## Consequences

- Le connecteur se teste contre un faux serveur d'API Kubernetes (liste, watch,
  coupure, `410`, signets, revue d'accès, droits en trop) et un vrai serveur kartenn de
  test ; un essai contre un vrai cluster reste manuel et documenté dans
  `docs/exploitation/`.
- Le client Kubernetes est une nouvelle dépendance, arbitrée à la tâche 019-9 sur la
  fiche de `crate-scout` (Q-019-12).
- Le rôle `kube_connector`, le groupe du connecteur et sa liaison se créent à la tâche
  019-11 ; `docs/spec/permissions.md` les montre une fois générés.
- `prop.*` dans le rôle n'est borné que par le périmètre et par `kube.connector.direct` :
  le connecteur ne remplace jamais une valeur de provenance humaine (019, design
  § Politique et flux).
