# Kubernetes : objets de calcul et synchronisation en lecture seule

Base de la spec [019](../../specs/019-modele-de-calcul/) (modèle de calcul et connecteur
Kubernetes). Faits de l'API utiles à la cartographie réseau et aux flux. La
documentation de Kubernetes écrit « must », « should » ou « cannot » sans renvoyer à
RFC 2119 et RFC 8174 : les exigences ci-dessous sont des formulations documentées, citées
en anglais, et non des mots-clés normatifs au sens de ces RFC.

## Sources

Documentation officielle (kubernetes.io), versionnée ; consultée le 2026-10-22 (pages de
la version 1.37 pour NetworkPolicy, modifiée le 2026-04-14).

| Document | URL |
|---|---|
| Nodes ; Node status ; API Node v1 | https://kubernetes.io/docs/concepts/architecture/nodes/ ; https://kubernetes.io/docs/reference/node/node-status/ ; https://kubernetes.io/docs/reference/kubernetes-api/cluster-resources/node-v1/ |
| Pods ; API Pod v1 | https://kubernetes.io/docs/concepts/workloads/pods/ ; https://kubernetes.io/docs/reference/kubernetes-api/workload-resources/pod-v1/ |
| Cluster networking ; Dual-stack | https://kubernetes.io/docs/concepts/services-networking/ ; https://kubernetes.io/docs/concepts/services-networking/dual-stack/ |
| Service ; API Service v1 ; Cluster IP allocation ; Virtual IPs | https://kubernetes.io/docs/concepts/services-networking/service/ ; https://kubernetes.io/docs/reference/kubernetes-api/service-resources/service-v1/ ; https://kubernetes.io/docs/concepts/services-networking/cluster-ip-allocation/ ; https://kubernetes.io/docs/reference/networking/virtual-ips/ |
| EndpointSlices ; API EndpointSlice v1 | https://kubernetes.io/docs/concepts/services-networking/endpoint-slices/ ; https://kubernetes.io/docs/reference/kubernetes-api/service-resources/endpoint-slice-v1/ |
| Ingress ; Gateway API | https://kubernetes.io/docs/concepts/services-networking/ingress/ ; https://kubernetes.io/docs/concepts/services-networking/gateway/ |
| Namespaces ; Labels ; Names ; Owners and dependents ; labels connus | https://kubernetes.io/docs/concepts/overview/working-with-objects/namespaces/ ; …/labels/ ; …/names/ ; …/owners-dependents/ ; https://kubernetes.io/docs/reference/labels-annotations-taints/ |
| NetworkPolicy | https://kubernetes.io/docs/concepts/services-networking/network-policies/ |
| API concepts | https://kubernetes.io/docs/reference/using-api/api-concepts/ |
| RBAC ; RBAC good practices ; Service accounts ; Service accounts admin ; Authentication | https://kubernetes.io/docs/reference/access-authn-authz/rbac/ ; https://kubernetes.io/docs/concepts/security/rbac-good-practices/ ; https://kubernetes.io/docs/concepts/security/service-accounts/ ; https://kubernetes.io/docs/reference/access-authn-authz/service-accounts-admin/ ; https://kubernetes.io/docs/reference/access-authn-authz/authentication/ |

## Ce que disent les sources

### Modèle réseau
- « Each pod in a cluster gets its own unique cluster-wide IP address. » ; « Pods can
  communicate with each other directly, without the use of proxies or address
  translation (NAT). » (Cluster networking, « The Kubernetes network model »), sauf
  segmentation volontaire.
- Les agents d'un nœud (kubelet, démons système) joignent tous les pods de ce nœud ; les
  conteneurs d'un pod partagent un espace réseau et se joignent par `localhost` (même
  section).
- Double pile : au plus une adresse IPv4 et une IPv6 par pod, stable depuis v1.23
  (Dual-stack, « Supported Features ») ; plage de services en double pile par
  `--service-cluster-ip-range=<IPv4 CIDR>,<IPv6 CIDR>` sur le serveur d'API (Dual-stack).
- La documentation ne dit pas d'où viennent les adresses de pods : la plage et son
  routage hors du cluster relèvent du plugin réseau et de l'installation.

### Node
- « Two Nodes cannot have the same name at the same time. Kubernetes also assumes that a
  resource with the same name is the same object. » (Nodes, « Node name uniqueness ») ;
  le nom est un sous-domaine DNS ; un Node n'a pas de namespace.
- `status.addresses` : types `Hostname`, `ExternalIP` (« typically [...] externally
  routable »), `InternalIP` (« typically [...] routable only within the cluster ») ;
  leur usage « varies depending on your cloud provider or bare metal configuration »
  (Node status, « Addresses »). « Consumers should assume that addresses can change
  during the lifetime of a Node. » ; plusieurs adresses d'un même type sont possibles
  (API Node v1, `status.addresses`).
- `spec.podCIDRs` : « may contain at most 1 value for each of IPv4 and IPv6 », la première
  égale `spec.podCIDR` (API Node v1) ; l'affectation n'a lieu que « if CIDR assignment is
  turned on » (Nodes, « Node controller »).
- Conditions `Ready`, `DiskPressure`, `MemoryPressure`, `PIDPressure`,
  `NetworkUnavailable` ; `Ready=Unknown` après `node-monitor-grace-period` (Node status,
  « Conditions »).
- Labels : `kubernetes.io/hostname` (peut différer du nom du Node avec
  `--hostname-override`) ; `topology.kubernetes.io/zone` et `region`, posés seulement
  avec un fournisseur d'infrastructure (Labels, annotations, taints).

### Pod
- « Pods are designed as relatively ephemeral, disposable entities » (Pods, « Working
  with Pods »).
- `status.podIPs` : « Pods may be allocated at most 1 value for each of IPv4 and IPv6.
  This list is empty if no IPs have been allocated yet. » ; `podIP` « Routable at least
  within the cluster » ; `hostIPs` : adresses du nœud (API Pod v1, `PodStatus`).
- `spec.nodeName` : nœud chargé du pod, vide avant ordonnancement ; `spec.hostNetwork` :
  « Use the host's network namespace » (API Pod v1).
- `containers[].ports` : `containerPort` (« Number of port to expose on the pod's IP
  address »), `protocol` (TCP par défaut, UDP, SCTP), `name` utilisable par un Service
  (API Pod v1, `ContainerPort`).
- `metadata.ownerReferences` : nom et UID du propriétaire, dans le même namespace, posés
  pour les dépendants des ReplicaSets, DaemonSets, Deployments, Jobs, CronJobs (Owners
  and dependents) ; Pods cite aussi StatefulSet parmi les ressources de workload.

### Service
- Types : `ClusterIP` (défaut, « cluster-internal IP »), `NodePort` (« on each Node's IP
  at a static port »), `LoadBalancer` (composant fourni hors de Kubernetes),
  `ExternalName` (CNAME DNS, « No proxying of any kind ») (Service, « Service types »).
- `clusterIP: None` : service headless ; `clusterIPs` au plus deux entrées (double
  pile) ; `externalIPs` : « not managed by Kubernetes » (API Service v1).
- « Across your whole cluster, every Service `ClusterIP` must be unique. » (Cluster IP
  allocation).
- Ports : `targetPort` vaut `port` par défaut, peut nommer un port de pod ; protocole TCP
  par défaut, UDP, SCTP ; `appProtocol` est « a hint » (Service). Plage NodePort par
  défaut 30000-32767 (`--service-node-port-range`), même numéro sur chaque nœud (Service,
  « NodePort »).
- Le sélecteur d'un Service est un sélecteur d'égalité seulement (Labels).
- `internalTrafficPolicy`, `externalTrafficPolicy` : `Cluster` ou `Local` ; `Local` ne
  route que vers les endpoints du nœud (Virtual IPs).

### EndpointSlice
- « The control plane automatically creates EndpointSlices for any Kubernetes Service
  that has a selector specified. » (EndpointSlices, « EndpointSlice API ») ; rattachés au
  Service par le label `kubernetes.io/service-name`.
- `addressType` immuable ; le type FQDN est déprécié et sans sémantique ; le contrôleur
  ne produit que IPv4 et IPv6 (API EndpointSlice v1).
- Par endpoint : adresses, `nodeName`, `targetRef`, conditions `ready`, `serving`,
  `terminating` ; 100 endpoints par slice par défaut, 1000 au plus.
- « Clients of the EndpointSlice API must iterate through all the existing EndpointSlices
  associated to a Service and build a complete list of unique network endpoints. »
  (EndpointSlices, « Duplicate endpoints »).
- Endpoints : déprécié depuis v1.33 (Service, « Endpoints (deprecated) »).

### Ingress
- « You must have an Ingress controller to satisfy an Ingress. Only creating an Ingress
  resource has no effect. » ; règles : `host` facultatif, `paths` avec `pathType` et un
  backend `service.name` et `service.port` (Ingress).
- « The Kubernetes project recommends using Gateway instead of Ingress. The Ingress API
  has been frozen. » (Ingress) ; Gateway API est une extension par ressources
  personnalisées, hors du cœur de l'API (Gateway API).

### Namespace, labels, noms et UID
- « Names of resources need to be unique within a namespace, but not across namespaces.
  Namespaces cannot be nested » ; label immuable `kubernetes.io/metadata.name` sur tout
  namespace (Namespaces).
- Noms (Names, relue le 2026-11-09, modifiée le 2026-01-27) : un sous-domaine DNS doit
  « contain no more than 253 characters », « contain only lowercase alphanumeric
  characters, '-' or '.' », « start with an alphanumeric character », « end with an
  alphanumeric character » (« DNS Subdomain Names ») ; une étiquette RFC 1123 « contain at
  most 63 characters », « contain only lowercase alphanumeric characters or '-' »,
  « start with an alphabetic character », « end with an alphanumeric character » (« RFC
  1123 Label Names »), avec la note « While RFC 1123 technically allows labels to start
  with digits, the current Kubernetes implementation requires both RFC 1035 and RFC 1123
  labels to start with an alphabetic character » (« RFC 1035 Label Names »).
- Clé de label (Labels, « Syntax and character set », relue le 2026-11-09, modifiée le
  2026-03-15) : « an optional prefix and name, separated by a slash (`/`) » ; le nom
  « must be 63 characters or less, beginning and ending with an alphanumeric character
  (`[a-z0-9A-Z]`) with dashes (`-`), underscores (`_`), dots (`.`), and alphanumerics
  between » ; le préfixe « must be a DNS subdomain: a series of DNS labels separated by
  dots (`.`), not longer than 253 characters in total » ; préfixes `kubernetes.io/` et
  `k8s.io/` « reserved for Kubernetes core components » ; « labels do not provide
  uniqueness ».
- Valeur de label : « must be 63 characters or less (can be empty) », « unless empty,
  must begin and end with an alphanumeric character (`[a-z0-9A-Z]`) », « could contain
  dashes (`-`), underscores (`_`), dots (`.`), and alphanumerics between » (même section).
- Sélecteurs (Labels, « Label selectors ») : égalité (`=`, `==`, `!=`) et ensembles
  (`in`, `notin`, `exists`) ; « all must be satisfied so the comma separator acts as a
  logical AND » ; `!=` et `notin` retiennent aussi « all resources with no labels with
  the `tier` key » ; « The semantics of empty or non-specified selectors are dependent on
  the context ». `matchLabels` : un couple vaut une exigence `In` à une valeur ;
  opérateurs de `matchExpressions` : `In`, `NotIn`, `Exists`, `DoesNotExist` ; « The
  values set must be non-empty in the case of In and NotIn » ; le sélecteur d'un Service
  n'admet que l'égalité (« Resources that support set-based requirements »).
- « Only one object of a given kind can have a given name at a time. However, if you
  delete the object, you can make a new object with the same name. » ; « Every object
  created over the whole lifetime of a Kubernetes cluster has a distinct UID. » (Names) ;
  rien ne garantit l'unicité d'un UID entre clusters.

### NetworkPolicy
- « Creating a NetworkPolicy resource without a controller that implements it will have
  no effect. » (NetworkPolicy, « Prerequisites »).
- Un pod n'est isolé dans une direction que si une politique le sélectionne avec ce type
  dans `policyTypes` (« The two sorts of pod isolation »).
- « Network policies do not conflict; they are additive. » : l'autorisé est l'union des
  politiques, sans ordre (« Additive nature of policies ») ; il faut l'autorisation en
  sortie de la source et en entrée de la destination.
- Pairs : `podSelector`, `namespaceSelector` (ET dans une même entrée, OU entre entrées),
  `ipBlock` (`cidr`, `except`) « should be cluster-external IPs, since Pod IPs are
  ephemeral » ; ports TCP, UDP, SCTP, plage par `endPort`.
- Hors du modèle (« What you can't do ») : viser un Service par son nom, journaliser,
  refuser explicitement (« deny by default, with only the ability to add allow rules »),
  bloquer le trafic de l'hôte du pod, politique globale par défaut.

### API : lister puis surveiller
- Un watch reprend de la dernière `resourceVersion` reçue ; sur `410 Gone`, « clients
  must handle the case by recognizing the status code 410 Gone, clearing their local
  cache, performing a new get or list operation, and starting the watch from the
  resourceVersion that was returned » (API concepts, « Efficient detection of
  changes ») ; l'historique gardé par le serveur est court (quelques minutes par défaut).
- Signets : `allowWatchBookmarks=true` ; « nor can clients assume that the API server
  will send any BOOKMARK event even when requested » (« Watch bookmarks »).
- Pagination : `limit` et `continue` ; jeton qui expire (410) ; même `resourceVersion`
  sur toutes les pages, donc instantané cohérent (« Retrieving large results sets in
  chunks »). Listes en flux (`sendInitialEvents=true`), bêta depuis v1.34
  (« Streaming lists »).
- « Resource versions must be passed unmodified back to the server. » ; ordonnables comme
  entiers croissants pour un même type servi par le serveur d'API, exigé pour la
  certification depuis 1.35 ; pour un serveur d'extension, seule l'égalité est sûre
  (« Resource versions »).

### Sécurité d'accès
- Authentification par certificat client X.509, jeton porteur ou proxy ; jeton de
  ServiceAccount = JWT pour `system:serviceaccount:<ns>:<nom>` (Authentication).
- Jetons liés par TokenRequest, « short-lived, automatically rotating », une heure par
  défaut ; les jetons Secret de longue durée ne sont plus générés d'office (Service
  accounts admin).
- « Permissions are purely additive (there are no "deny" rules). » ; Role et RoleBinding
  par namespace, ClusterRole et ClusterRoleBinding pour le cluster ; le rôle `view`
  exclut les Secrets (RBAC).
- « `list` and `watch` access also effectively allow for users to reveal the Secret
  contents » ; moindre privilège, pas de jokers (RBAC good practices, « privilege
  escalation risks »).

## Pièges

- Le nom d'un objet est réutilisable après suppression, l'UID non : une recréation
  change l'UID d'un objet « identique ».
- `kubernetes.io/hostname` et les adresses d'un Node peuvent changer ou différer du nom.
- `spec.podCIDRs` peut être vide (affectation désactivée) ; la plage de services n'est
  lisible par aucun objet de l'API citée ici : elle se déclare.
- Un pod `hostNetwork` a l'IP de son nœud : son adresse n'identifie pas le pod.
- Les ports déclarés d'un conteneur ne prouvent pas une écoute.
- Un même endpoint peut figurer dans plusieurs EndpointSlices.
- Une NetworkPolicy sans plugin qui la met en œuvre n'a aucun effet ; la traduction
  d'adresses d'un Service peut se faire avant ou après la politique.
- Lire un pod donne ses variables d'environnement littérales : la lecture des pods n'est
  pas anodine même sans accès aux Secrets.

## Implications pour kartenn (propositions, tranchées dans la 019)

- Identifiant de cluster choisi par kartenn ; identité d'un objet par (cluster,
  namespace, sorte, nom), UID en attribut (019/X3).
- Syntaxe des noms et des labels (choix de kartenn, tâche 019-1) : un miroir ne refuse
  jamais ce que le cluster a admis, il contrôle donc la syntaxe documentée sans y
  ajouter. Étiquette (namespace, service) : 63 caractères, minuscules,
  chiffres et `-`, alphanumérique aux deux bouts, premier caractère chiffre admis comme
  le permet RFC 1123 §2.1 (Kubernetes demande une lettre ; RFC 1035 pour les services).
  Sous-domaine (nœud, workload, ingress) : 253 caractères, lu comme « a series of DNS
  labels separated by dots » (la définition du préfixe d'une clé de label) : étiquettes
  non vides, chacune alphanumérique aux deux bouts ; la page ne borne pas la longueur
  d'une étiquette, kartenn non plus. Pas de passage en minuscules : un nom en majuscules ne
  vient pas d'un cluster et est refusé (`kube.name.invalid`). Préfixes réservés admis en
  lecture (`kubernetes.io/metadata.name`, `app.kubernetes.io/name`).
- Boucle : liste paginée, watch avec signets, reliste sur 410 ; `resourceVersion`
  conservée telle quelle, servant à reprendre (019/X8).
- Pods éphémères : pas d'objet versionné par pod, journal d'observation (019/X7, Q-019-3).
- `hostNetwork` : évalué sur les adresses du nœud (019/X12).
- ClusterRole en `get`, `list`, `watch` sur les seules ressources lues, jamais `secrets`,
  jeton lié de courte durée (019/X10) ; D13, D36.
- NetworkPolicy : allow seul ; l'interdiction qui l'emporte de D28 devrait être compilée
  en autorisations restantes ; hors de la 019 (Q-019-10, D1).
- Gateway API : extension facultative, à reprendre par une spec ultérieure.
