---
status: approved
updated: 2026-11-09
---

# 019 — Modèle de calcul et Kubernetes : exigences

**Statut : validée le 2026-11-09 par l'utilisateur (tâche 0b).** Les treize questions sont arbitrées par l'utilisateur
(2026-11-09, [`decisions.md`](decisions.md) § Arbitrages, Q-019-1 à 13) ; les exigences
ci-dessous en suivent les décisions, chacune citant la question qui la fonde, et sont
remises en accord avec les specs 022, 024 à 027 (tâche 0a).

## Résumé

Spec de la v0.3 (D31 amendée le 2026-10-22). Elle prolonge la 009 (machines physiques,
VM, conteneurs, clusters de virtualisation) jusqu'aux charges conteneurisées orchestrées
par Kubernetes, du point de vue du réseau et des flux : **où tourne une charge, quelles
adresses elle utilise, ce qu'elle expose**. Elle ajoute un module `kube` (clusters,
nœuds, namespaces, workloads, services, ingress), un **connecteur** qui synchronise un
cluster en lecture seule, un **journal d'observation** des pods hors de l'historique
bitemporel, et fait des workloads des extrémités de la politique (010) et des flux (011),
visés par leurs classifications, leur namespace et leurs labels. L'écriture dans le
cluster (NetworkPolicy) est une évolution hors de cette spec.

Décisions : [`decisions.md`](decisions.md) (X1 à X16, Q-019-1 à 13) ; D1, D4, D7, D8,
D10, D11, D13, D21 à D23, D28, D30, D31, D33, D36
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md) ; D1, D8, D11, D22 et D31
amendées par la 019) ; ADR 0019, 0023, 0026, 0030, 0031,
[0043](../../docs/adr/0043-observations-hors-referentiel.md) (observations) et
[0044](../../docs/adr/0044-connecteur-kubernetes.md) (connecteur) ; specs 009, 010, 011,
022 (événements), 024 (grammaire des permissions), 025 (droits par groupes), 026 (VLAN
dans le cœur), 027 (paramètres). Référence :
[`docs/reference/kubernetes-calcul.md`](../../docs/reference/kubernetes-calcul.md).

### Ce que Kubernetes impose et ce que kartenn choisit

| Kubernetes impose (réf.) | kartenn choisit |
|---|---|
| nom unique par sorte et par namespace à un instant, réutilisable ; UID distinct pour tout objet créé (§ Namespace, labels, noms et UID) | identité kartenn = (cluster, namespace, sorte, nom) ; UID en attribut (X3) |
| au plus une IP de pod par famille ; pod éphémère (§ Pod) | pas d'objet par pod ; journal à rétention bornée (X7) |
| reprendre un watch par `resourceVersion`, relister sur `410 Gone` (§ API) | lots en changesets, rapprochement à chaque liste (X8, X9) |
| endpoints à dédoublonner sur tous les EndpointSlices d'un Service (§ EndpointSlice) | sélection d'un service = ensemble de workloads (X4) |
| NetworkPolicy additive, sans refus, sans effet sans plugin (§ NetworkPolicy) | aucune écriture dans le cluster (X13) |
| `list`/`watch` sur les Secrets révèle leur contenu (§ Sécurité d'accès) | ClusterRole sans Secrets, refus de démarrer si trop de droits (X10) |

### Vocabulaire

- **Cluster Kubernetes** : objet `kube.cluster`, distinct du cluster de virtualisation
  `machines.cluster` (009).
- **Nœud** : objet `kube.node`, qui désigne la machine (009) sur laquelle il tourne.
- **Workload** : objet `kube.workload`, contrôleur racine d'un ensemble de pods
  (Deployment, StatefulSet, DaemonSet, CronJob, Job, ReplicaSet sans propriétaire, pod
  sans propriétaire).
- **Objet miroir** : objet `kube.*` dont les champs suivis sont écrits par le seul
  connecteur (Q-019-2).
- **Champs suivis** : champs d'un objet Kubernetes versionnés par kartenn (X4).
- **Observation** : ligne du journal des pods, hors référentiel, hors bitemporel et hors
  changesets (Q-019-4, ADR 0043).
- **Lot d'observations** : ensemble d'ouvertures et de fermetures d'observations envoyé
  en une requête, accepté ou refusé en entier, audité avec l'empreinte de son contenu.
- **Groupe du connecteur** : groupe kartenn (025) que désigne un cluster ; ses membres
  comptes de service sont les seuls auteurs admis des champs suivis et des observations
  du cluster.
- **Absent** : état d'un objet miroir disparu du cluster mais cité dans kartenn (Q-019-9).
- **Hyperviseur** : machine `physical` qui héberge une VM ou est membre d'un cluster de
  virtualisation (Q-019-11).
- Conventions EARS, « écriture », « période », sévérités : comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions) ;
  « objet de module », « ancre », « ancrage de domaine » : comme dans l'ADR 0026
  (clause 4) ; machines, interfaces, rattachements : comme dans la 009 ; le VLAN est un
  objet du cœur (026/C1), il n'y a pas de module `vlan`.

## Exigences

### A. Modèle de calcul (009 amendée)

#### REQ-019-1 — Hyperviseurs (Q-019-11)
kartenn doit rendre, à une date, la liste des hyperviseurs (machines `physical` qui
hébergent une VM ou sont membres d'un cluster de virtualisation) et fournir la propriété
`machines.platform` (liste fermée simple, administrable, livrée vide, portée par les
machines, non héritée).

Acceptation :
- Un hôte qui héberge une VM figure dans `GET /v1/machines?hypervisor=true` ; il en sort
  quand la VM change d'hôte.
- Aucun type de machine nouveau ; la table de vérité de REQ-009-5 est inchangée.

#### REQ-019-2 — Compatibilité avec la 009
kartenn doit garder inchangés le comportement, les routes et les données de la 009 pour
un appelant qui n'utilise pas le module `kube`, et démarrer sans ce module.

Acceptation :
- Les tests de la 009 passent avec et sans le module `kube` ; une build sans `kube` ne
  crée pas le schéma `kube`.

#### REQ-019-3 — Trace d'un inventaire antérieur des pods
Où le module `kube` est présent, quand une machine `container` est hébergée par la
machine d'un nœud et que son nom est celui d'un pod observé, kartenn doit le signaler
(`kube.machine.looks_like_pod`, avertissement) sans rien modifier.

### B. Module `kube`

#### REQ-019-4 — Déclaration du module (Q-019-1)
kartenn doit fournir le module `kube`, dépendant de `machines`, avec les types
`kube.cluster`, `kube.node` et `kube.namespace` (parent : cluster), `kube.workload`,
`kube.service` et `kube.ingress` (parent : namespace), et refuser de démarrer sans le
module `machines`.

Acceptation :
- `GET /v1/capabilities` liste `kube` ; une build avec `kube` sans `machines` ne démarre
  pas (message qui nomme la dépendance).

#### REQ-019-5 — Cluster Kubernetes
kartenn doit gérer des clusters Kubernetes munis d'un slug unique à tout instant, de
`core.owner` (obligatoire) et `core.site` (facultatif), d'un domaine des pods et d'un
domaine des services (Q-019-5), de leurs plages de pods (conteneurs) et de services
(réseaux) déclarées par un administrateur, d'une rétention d'observation (1 à 400 jours,
30 par défaut), d'une liste de clés de labels suivies propre au cluster, d'une fenêtre
de lot du connecteur (10 s et 500 opérations par défaut) et du groupe du connecteur
(groupe kartenn, 025) ; tous ces champs s'écrivent par changeset sous
`kube.cluster.edit`.

Acceptation :
- Deux clusters de même slug : refus ; une plage de pods hors du domaine des pods :
  refus (`kube.cluster.range_domain`).
- Les objets d'un cluster héritent de son propriétaire et de son site (arbre du module).
- Le cluster désigne un groupe, jamais une identité ; un groupe inexistant ou clos sur
  la période : refus (`kube.cluster.connector_group`).
- Un changement de fenêtre de lot est pris par le connecteur à son lot suivant, sans
  redémarrage.

#### REQ-019-6 — Nœud
kartenn doit gérer des nœuds, chacun lié à un cluster fixe, nommés comme dans
Kubernetes (sous-domaine DNS, unique dans le cluster à tout instant), avec leurs
adresses par type, leurs plages de pods (au plus une par famille), leur zone, et la
machine `physical` ou `virtual` désignée, facultative.

Acceptation :
- Deux nœuds de même nom vivants dans un cluster : refus (`kube.node.duplicate`).
- Désigner une machine `container` ou `appliance` : refus (`kube.node.machine_kind`).
- Une machine peut être désignée par un seul nœud vivant à la fois.

#### REQ-019-7 — Namespace
kartenn doit gérer des namespaces, liés à un cluster fixe, au nom unique dans le cluster
à tout instant (étiquette DNS RFC 1123), avec leurs labels suivis.

#### REQ-019-8 — Workload (X3)
kartenn doit gérer des workloads, liés à un namespace fixe, identifiés par (sorte, nom)
dans le namespace à tout instant, avec leur UID Kubernetes, leurs labels suivis (ceux du
gabarit de pod), leur compte de service, leurs ports déclarés (port, protocole, nom) et
l'indicateur `hostNetwork`. Sortes : `deployment`, `statefulset`, `daemonset`,
`cronjob`, `job`, `replicaset`, `pod`.

Acceptation :
- Un Deployment supprimé puis recréé sous le même nom (nouvel UID) reste le même
  workload ; son historique montre le changement d'UID.
- Les ports déclarés sont restitués comme « déclarés », jamais comme « en écoute ».

#### REQ-019-9 — Service
kartenn doit gérer des services, liés à un namespace fixe, au nom unique dans le
namespace, avec leur type (`cluster_ip`, `node_port`, `load_balancer`, `external_name`),
l'indicateur headless, leurs ports (port, port cible, protocole TCP, UDP ou SCTP,
`appProtocol`, port de nœud), leurs IP de cluster et adresses externes (adresses du
noyau d'usage `vip`), le nom externe d'un `external_name`, et l'ensemble des workloads
sélectionnés.

Acceptation :
- Un service sélectionnant deux workloads : la fiche du service les liste, la fiche de
  chaque workload liste le service.
- Clore l'adresse d'IP de cluster d'un service vivant : refus, le service listé.

#### REQ-019-10 — Ingress
kartenn doit gérer des ingress, liés à un namespace fixe, avec leur classe, leurs règles
(hôte facultatif, chemin, type de chemin, service et port cibles), leurs hôtes TLS et
leurs adresses (adresses du noyau d'usage `vip`).

Acceptation :
- Une règle qui vise un service absent du namespace : avertissement
  (`kube.ingress.backend_missing`), pas de refus (le cluster l'accepte aussi).

#### REQ-019-11 — Relations et arêtes
kartenn doit déclarer les arêtes `kube.runs_on` (nœud → machine), `kube.node_range`
(nœud → préfixe), `kube.selects` (service → workload), `kube.service_address` (service
→ adresse), `kube.routes_to` (ingress → service), `kube.ingress_address` (ingress →
adresse) ; avec celles de la 009 (`machines.hosted_on`, `hosted_in`, `member_of`), elles
rendent la chaîne « ingress expose service, service expose workload, workload
s'exécute sur nœud, nœud s'exécute sur machine, machine hébergée par hyperviseur ».

Acceptation :
- `GET /v1/graph/edges?object=<workload>` rend ses services ; une lecture
  « chaîne d'exécution » d'un workload rend nœuds, machines et hyperviseurs à la date.

#### REQ-019-12 — Objets miroirs (Q-019-2)
Si un changeset modifie un champ suivi d'un objet `kube.*` et que son auteur n'est pas
un compte de service membre du groupe du connecteur du cluster de l'objet, alors
kartenn doit refuser l'écriture (`kube.object.managed`) ; les propriétés portées par ces
objets restent écrites sous les droits ordinaires.

Acceptation :
- Un administrateur renomme un workload : refus ; il pose un environnement sur le même
  workload : accepté.
- Une personne ajoutée au groupe du connecteur renomme un workload : refus (seul un
  compte de service est admis).

### C. Adressage

#### REQ-019-13 — Plages de pods des nœuds (X6)
Quand le connecteur lit les plages de pods d'un nœud, kartenn doit les tenir comme
réseaux du domaine des pods, sous un conteneur de plage de pods du cluster, rattachés au
nœud, et signaler une plage hors de tout conteneur déclaré (`kube.node.range_outside`,
avertissement, la plage n'est pas créée).

Acceptation :
- Un nœud `10.64.3.0/24` dans le conteneur `10.64.0.0/16` : le réseau est créé et le
  nœud le cite ; le nœud disparaît : le réseau est clos avec lui (cascade).

#### REQ-019-14 — Adresses de services et d'ingress
kartenn doit tenir les IP de cluster dans le domaine des services et les adresses
externes dans le domaine désigné par le cluster pour elles, comme adresses d'usage
`vip`, créées et closes par le connecteur avec leur service ou leur ingress.

#### REQ-019-15 — Adresses de pods (Q-019-3)
kartenn ne doit créer aucune adresse du noyau pour l'IP d'un pod ; une adresse de pod
doit se résoudre en classification par l'arbre d'adressage (plage du nœud) et en
workload par le journal d'observation.

#### REQ-019-16 — Journal d'observation (Q-019-4, X7, ADR 0043)
kartenn doit tenir, par cluster, un journal des pods observés (namespace, nom, UID,
workload, nœud, IP par famille, `hostNetwork`, période observée), hors référentiel,
écrit par lots par le connecteur hors changeset, et purgé au-delà de la rétention du
cluster, avec les garde-fous suivants :
- **droit propre** : écrire exige `kube.observation.record`, lire
  `kube.observation.read`, chacun sur le périmètre du cluster ; un lot n'est accepté que
  d'un compte de service membre du groupe du connecteur du cluster ;
- **validation à l'entrée** : un lot dont une IP est hors des plages de pods du cluster
  et des adresses de ses nœuds, ou qui cite un workload ou un nœud d'un autre cluster,
  est refusé en entier ;
- **ajout seul** : un lot n'ouvre que des observations et ne ferme que des périodes
  ouvertes ; aucune autre mise à jour ni suppression ligne à ligne n'est permise, ni par
  l'API ni par les droits PostgreSQL de l'application ;
- **audit par lot** : chaque lot accepté inscrit `observation.batch.recorded` au journal
  scellé (012) avec l'empreinte SHA-256 de son contenu ; une vérification périodique
  inscrit `observation.batch.altered` pour un lot dont le contenu stocké ne correspond
  plus à l'empreinte ;
- **séparation** : une observation n'écrit rien dans le référentiel et n'émet aucun
  événement `changes` ;
- **rétention** : la purge supprime des partitions entières, jamais une observation
  encore ouverte.

Acceptation :
- « Qui avait `10.64.3.17` le 2026-11-02 à 10 h ? » rend le pod, son workload et son
  nœud ; après la rétention, la même question rend une réponse vide et le motif
  `retention`.
- Le journal n'a pas de lecture `as_of` ; aucune observation n'entre dans un verdict.
- Droit propre : un lot du compte du connecteur du cluster A sur la route du cluster B,
  ou d'une personne qui a `kube.observation.record` : refus (403), rien n'est écrit.
- Validation : un lot dont une ligne porte `192.0.2.10`, hors des plages du cluster :
  refus (422, `kube.observation.invalid`, la ligne nommée), aucune ligne du lot écrite.
- Ajout seul : réécrire l'IP ou le workload d'une observation, rouvrir une période
  fermée, supprimer une ligne, par l'API ou en SQL sous le rôle de l'application :
  refus.
- Audit : un lot accepté produit un et un seul `observation.batch.recorded`, dont
  l'empreinte se recalcule du contenu stocké ; une ligne modifiée hors de l'application
  (propriétaire du schéma) est détectée à la vérification suivante
  (`observation.batch.altered`, le lot nommé).
- Lecture bornée : un lecteur qui n'a `kube.observation.read` que sur le périmètre du
  cluster A ne voit aucune observation du cluster B.
- Séparation : un lot ne crée, ne modifie ni ne clôt aucun objet du référentiel, et
  aucun abonné au flux d'événements ne reçoit de `changes` pour lui.
- Rétention : un pod vivant depuis plus longtemps que la rétention garde son
  observation ouverte ; la purge ne supprime que des partitions dont toutes les
  observations sont fermées depuis plus que la rétention de leur cluster.

#### REQ-019-17 — Workloads d'une adresse
kartenn doit rendre, pour une adresse ou un réseau à une date dans la rétention, les
workloads observés, et pour un workload, les nœuds et adresses observés.

### D. Synchronisation (Q-019-6)

#### REQ-019-18 — Connecteur
kartenn doit fournir un connecteur, processus distinct et client de l'API REST, livré
dans l'image unique (sous-commande `kube-sync`), qui synchronise un ou plusieurs
clusters déclarés.

Acceptation :
- Le connecteur n'ouvre aucune connexion à PostgreSQL ; ses écritures sont des
  changesets et des lots d'observations visibles dans l'audit sous son compte.

#### REQ-019-19 — Liste puis surveillance
Pour chaque type suivi, le connecteur doit lister par pages, puis surveiller à partir de
la `resourceVersion` de la liste avec signets, reprendre après une coupure à la dernière
version reçue, et relister sur `410 Gone` (réf. § API).

Acceptation :
- Faux serveur d'API qui coupe le watch puis rend `410` : le connecteur reliste, et
  kartenn converge vers l'état du faux serveur sans doublon.

#### REQ-019-20 — Lots et idempotence (X8)
Le connecteur doit regrouper les changements d'un cluster en changesets par fenêtre
(celle du cluster, REQ-019-5 : 10 s ou 500 opérations par défaut), idempotents (`Idempotency-Key` dérivée
du cluster et de la fenêtre), en ne transmettant que les champs suivis.

Acceptation :
- Un déploiement qui ne change que l'image et le nombre de réplicas : aucun changeset.
- Rejouer une fenêtre après une panne : aucun doublon.

#### REQ-019-21 — Rapprochement (X9)
À chaque liste complète, le connecteur doit clore dans kartenn les objets du cluster
absents de la liste et sans dépendant, passer **absents** ceux qui ont des dépendants, et
créer ou mettre à jour les autres, retrouvés par leur identifiant externe.

#### REQ-019-22 — Objets absents (Q-019-9)
Quand un objet miroir cité par un dépendant disparaît du cluster, kartenn doit le
marquer absent (date d'absence), lever l'anomalie `kube.object.absent` avec les
dépendants, et lever l'absence si un objet de même identité réapparaît.

Acceptation :
- Workload cité par un flux actif, supprimé du cluster : le workload est absent, le flux
  reste, l'anomalie nomme le flux ; recréé, il redevient présent et l'anomalie se ferme.

#### REQ-019-23 — Nœuds et machines (X9)
Le connecteur doit désigner la machine d'un nœud par ses adresses `InternalIP` (port de
lecture de la 009), sinon par son nom chez le propriétaire du cluster ; si la
correspondance est ambiguë ou absente, il ne doit rien lier et kartenn doit lever
`kube.node.unmatched` (avertissement). Il ne doit créer de machine que si l'option
`create_node_machines` du cluster est active.

#### REQ-019-24 — Dates
Le connecteur doit dater la création d'un objet par son `creationTimestamp`, et une
modification ou une suppression par l'instant d'observation.

#### REQ-019-25 — Mode de repli
Où le watch est indisponible, le connecteur doit synchroniser par listes complètes
périodiques (5 min par défaut) et le signaler dans ses métriques.

### E. Sécurité du connecteur (X10)

#### REQ-019-26 — Lecture seule côté cluster
Le connecteur ne doit envoyer au cluster que des requêtes `get`, `list` et `watch`, et
des revues d'accès sur ses propres droits.

#### REQ-019-27 — Droits minimaux
kartenn doit livrer un manifeste de ClusterRole qui n'accorde que `get`, `list`, `watch`
sur `nodes`, `namespaces`, `pods`, `services`, `endpointslices`, `ingresses`,
`deployments`, `replicasets`, `statefulsets`, `daemonsets`, `jobs`, `cronjobs`, et
aucune sous-ressource, ni `secrets`, ni `configmaps`.

#### REQ-019-28 — Contrôle des droits au démarrage
Au démarrage, le connecteur doit interroger ses droits effectifs sur le cluster et
refuser de démarrer s'il peut écrire une ressource ou lire les `secrets`, sauf option
explicite `allow_excess_rights`, consignée dans ses logs et dans l'audit de kartenn.

Acceptation :
- Faux serveur d'API qui accorde `list secrets` : refus de démarrer, message qui nomme
  le droit en trop.

#### REQ-019-29 — Identifiants
Le connecteur doit lire ses identifiants (jeton lié ou certificat client du cluster ;
secret de son compte kartenn) dans des fichiers relus à leur renouvellement, refuser un
serveur d'API dont la CA n'est pas configurée, et ne jamais écrire un identifiant dans
ses logs ni ses traces.

#### REQ-019-30 — Confinement côté kartenn (X10, ADR 0044)
kartenn doit livrer, avec le module, un rôle `kube_connector` qui n'accorde que la
lecture des clusters, l'écriture des objets `kube.*` (hors clusters), des réseaux et
adresses et des propriétés de son périmètre, et l'écriture des observations ; ce rôle
doit se lier au groupe du connecteur d'un cluster (025/G1 : aucune liaison nominale) sur
le périmètre des domaines désignés du cluster. Si un changeset d'un compte membre d'un
groupe de connecteur porte une autre opération que `kube.sync`, kartenn doit le refuser
(`kube.connector.direct`).

Acceptation :
- Le compte du connecteur du cluster A écrit un workload du cluster B, même quand les
  deux clusters partagent un domaine : refus (`kube.object.managed`).
- Le même compte pose directement un axe sur une adresse de son domaine
  (`AssignProperty` hors `kube.sync`) : refus (`kube.connector.direct`).
- `GET /v1/roles` liste `kube_connector` comme rôle livré ; aucune liaison ne nomme le
  compte du connecteur.

#### REQ-019-31 — Minimisation
Le connecteur ne doit garder, transmettre ou journaliser que les champs suivis ; les
variables d'environnement, annotations et autres champs d'un objet lu ne doivent sortir
d'aucune de ses sorties.

### F. Politique et flux (amendements de la 010 et de la 011)

#### REQ-019-32 — Porteurs d'axes (X11 ; amende REQ-010-29)
kartenn doit admettre `kube.cluster`, `kube.namespace`, `kube.workload` et
`kube.service` comme porteurs de propriétés, et un axe de la politique porté par le
workload, résolu par affectation propre, sinon par héritage namespace puis cluster,
sinon par la valeur héritée de l'arbre d'adressage sur la plage de pods ; l'explication
dit la source.

Acceptation :
- Namespace `{prod}`, workload sans valeur : le workload résout `{prod}` (source :
  namespace) ; workload `{rec}` : `{rec}` (remplacement, D26).

#### REQ-019-33 — Axe namespace (Q-019-37 à 39)
Le module doit livrer l'axe `kube.namespace` (référence externe, système `kube` que le
module déclare), posé par `kube.sync` sur chaque workload (identifiant
`<id du cluster>/<nom>`, libellé `<slug du cluster>/<nom>`), utilisable dans une
contrainte « même valeur » (REQ-010-8) ; toute autre affectation ou tout autre retrait
de cet axe doit être refusé (`kube.namespace.managed`).

#### REQ-019-34 — Labels vers axes (Q-019-7, Q-019-33 à 36)
kartenn doit tenir comme paramètres du module (`kube.settings.*`, 027/P4, P5) des
correspondances (clé de label, valeur de label) → valeur d'axe, et, par namespace, les
valeurs d'axe admises, écrites par changeset sous `kube.settings.edit` (`apply`,
`approval.self`) sans permission propre (types `kube.label_mapping`,
`kube.admitted_values`, `kube.tracked_label`) ; quand un workload porte un label
correspondant, le serveur (`kube.sync`, à l'`upsert` du workload) doit poser la valeur
d'axe si elle est admise dans son namespace, avec la provenance
`module:kube#label:<clé>`, et sinon l'ignorer en levant `kube.label.not_admitted` ; un
changeset qui écrit un paramètre doit re-dériver de même les workloads concernés, dans sa
transaction.

Acceptation :
- `tier=db` → `policy.exposure: interne` admise dans `payments` : posée ; la même
  correspondance dans un namespace où `interne` n'est pas admise : ignorée, anomalie.
- Une valeur posée par label ouvre un flux : le différentiel de politique le montre.
- Une équipe qui n'a que `policy.settings.*` crée l'axe et ses valeurs mais ne peut
  pas écrire de correspondance (refus, `kube.settings.edit` nommée) ; l'inverse
  aussi (REQ-027-5).
- `interne` ajoutée aux valeurs admises de `payments` après coup : le changeset des
  paramètres pose la valeur sur les workloads `tier=db` du namespace, sans attendre le
  connecteur.

#### REQ-019-35 — Extrémités logiques (Q-019-8 ; amende REQ-010-13, REQ-011-2)
La politique et le registre de flux doivent accepter comme extrémité un workload, un
namespace (chacun de ses workloads) ou un service (ses workloads sélectionnés pour une
source dans le cluster, ses adresses `vip` pour une source hors du cluster), et rendre
un verdict par paire d'extrémités logiques ; un workload `hostNetwork` s'évalue sur les
adresses des machines de ses nœuds.

Acceptation :
- Workload A vers workload B, service `db-sql` : un verdict, sur les axes résolus
  des deux workloads, quel que soit le nombre de pods.
- Namespace de 3 workloads vers un workload : 3 verdicts et une synthèse.

#### REQ-019-36 — Réévaluation (amende REQ-011-11)
Quand un changeset modifie un axe d'un workload, d'un namespace ou d'un cluster, la
sélection d'un service, ou l'état absent d'une extrémité, le registre de flux doit
réévaluer les flux touchés dans la même transaction.

#### REQ-019-37 — Absence et flux
Un flux dont une extrémité est absente doit garder son évaluation et porter
l'avertissement `flow.endpoint.absent` jusqu'à la clôture ou au retour de l'extrémité.

### G. Lectures, droits, API, interface

#### REQ-019-38 — Listes et lectures datées
kartenn doit lister clusters, nœuds, namespaces, workloads, services et ingress avec les
conventions de la 006 (objets désignés par `{id}`, clé naturelle en filtre, 006/H3 :
`GET /v1/kube-clusters?slug=` ; filtres par cluster, namespace, nom, sorte, label suivi,
machine, état absent ; lectures à `valid_at` et `as_of`).

#### REQ-019-39 — Permissions et rôles livrés (X16)
kartenn doit contrôler le module par `kube.<type>.read|edit|apply|approval.self` pour
chacun des six types (grammaire de la 024, M7), sur le périmètre hérité du cluster ; par
`kube.observation.read` et `kube.observation.record` pour le journal ; par
`kube.settings.edit|apply|approval.self`, globales, pour les paramètres du module.
Les rôles livrés doivent recevoir : lecteur, éditeur et approbateur, les
`kube.<type>.read` ; l'approbateur, en plus, les `kube.<type>.apply` ; aucun des trois,
`kube.observation.read`.

Acceptation :
- Le catalogue généré (`docs/spec/permissions.md`) liste ces permissions ;
  `catalogue_problems` n'en signale aucune ; aucune ne figure parmi les anciens noms.
- Un lecteur lit les workloads de son périmètre, pas le journal d'observation.

#### REQ-019-40 — Plugin d'interface et événements
Où le shell web est livré, kartenn doit fournir un plugin `kube` : vues des clusters,
namespaces, workloads et services ; chaîne d'exécution d'un workload ; onglet
« Workloads » sur une adresse et un réseau ; maquettes validées avant le développement
(comme 017/U14). Les objets `kube.*` écrits par un changeset doivent figurer dans les
événements `changes` (022) et se filtrer par `types` ; un lot d'observations ne doit
produire aucun événement.

Acceptation :
- Un abonné filtré sur `types=kube.workload` reçoit un événement `changes` par lot du
  connecteur qui touche un workload visible, aucun pour un lot qui ne touche que des
  services, aucun pour un lot d'observations.

### H. Budgets

#### REQ-019-41 — Budgets (X14)
kartenn doit respecter, sur le jeu de référence augmenté (10 clusters, 2·10³ nœuds,
10⁴ namespaces, 10⁵ workloads, 5·10⁴ services, 10⁴ ingress, 3·10⁶ observations) :
lecture d'un workload < 50 ms ; page filtrée < 150 ms ; workloads d'une IP à une date
< 50 ms ; verdict workload × workload 16 × 16 < 300 ms ; lot de 500 opérations du
connecteur < 2 s (p95 serveur) ; et, côté connecteur, un délai d'un changement du
cluster à kartenn < 60 s au p95 et une liste initiale d'un cluster de 10⁴ pods < 5 min.

## Non-objectifs

- Écrire dans un cluster : NetworkPolicy générée ou appliquée, étiquetage, annotations
  (Q-019-10, évolutions par specs ultérieures).
- Lire les NetworkPolicy et comparer déclaré et appliqué (évolution 1 de Q-019-10).
- Gateway API, CRD, maillage de services, identités de charge de travail.
- Inventaire complet du cluster (volumes, ConfigMap, quotas, images) ; conteneurs d'un
  pod comme objets.
- Traduction d'adresses des services (NodePort, LoadBalancer) : avec le NAT, v0.4.
- Autres orchestrateurs ; plateformes de virtualisation lues par connecteur (X1).
- Flux constatés (sorte `observed`) : le journal les prépare, sans les livrer (Q-019-13).

## Questions ouvertes

Aucune : Q-019-1 à 13 sont arbitrées ([`decisions.md`](decisions.md) § Arbitrages).
Restent à relire par l'utilisateur à la validation (tâche 0b) : le confinement du
connecteur côté kartenn (X10, ADR 0044), la lecture des observations hors des rôles
livrés (X16) et les chiffres de X14. Le client Kubernetes (Q-019-12) revient à
l'arbitrage avec sa fiche, à la tâche 9.
