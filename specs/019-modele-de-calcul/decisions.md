# 019 — Modèle de calcul et Kubernetes : journal des décisions

**Statut : validée le 2026-11-09 par l'utilisateur (tâche 0b).** Arbitrages rendus le 2026-11-09 (Q-019-1 à 13, toutes
sur la recommandation, Q-019-4 avec garde-fous) ; X1 à X16, requirements et design remis
en accord le 2026-11-09 (tâche 0a) avec les arbitrages et avec les specs 022, 024 à 027
livrées depuis le brouillon du 2026-10-22. D1, D8, D11, D22 et D31 sont amendées
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; ADR
[0043](../../docs/adr/0043-observations-hors-referentiel.md) et
[0044](../../docs/adr/0044-connecteur-kubernetes.md) rédigées (statut `proposed`).
Le sujet (machines physiques, hyperviseurs, VM, conteneurs, pods et nœuds Kubernetes,
interaction avec Kubernetes) a été retenu par l'utilisateur pour la v0.3 ; D31 est amendée
en conséquence (passerelles, routes et NAT, puis flux constatés, ticketing et campagnes de
revue passent en v0.4). Les choix réversibles sont de Claude ; les choix structurants
(contrat, schéma, sécurité, dépendance) et toute contradiction avec D1 à D37 ont été
arbitrés par l'utilisateur (§ Arbitrages).

Cadre : D1 (déclaratif, connecteurs), D4 (domaines), D7 (REST, frontière unique ;
connecteurs hors processus), D8 et D10 (bitemporel), D11 (changesets), D13 et D36
(identités, RBAC), D14 et D2 (budgets, volumétrie), D20 à D30 (politique et flux), D31
(feuille de route), D33 (suppression) ; ADR 0023 (tâches de fond), 0026 (contrat de
module), 0030 (instantané de politique), 0031 (évaluations de flux matérialisées) ; specs
009 (machines), 010 (politique), 011 (flux). Référence :
[`docs/reference/kubernetes-calcul.md`](../../docs/reference/kubernetes-calcul.md), notée
« réf. » ci-dessous avec sa section.

## Arbitrages

Chaque question porte le constat, les options, la recommandation de Claude et la
décision de l'utilisateur (2026-11-09). Les questions 1 à 4 conditionnent toute la spec.

### Q-019-1 — Où vit le modèle Kubernetes
- Constat : la 009 a déjà les types `physical`, `virtual`, `container`, `appliance`, les
  clusters de virtualisation et l'hébergement (M2, M5, M6). Kubernetes ajoute des objets
  qui n'ont pas d'interfaces (namespaces, workloads, services, ingress) et une source
  externe qui fait foi pour eux.
- Options : (A) nouveau module `kube`, dépendant de `machines`, pour les objets
  Kubernetes ; la 009 ne reçoit que des ajouts (hyperviseur, lecture des nœuds) ;
  (B) étendre `machines` (nouveaux `kind`, namespaces et services dans le même schéma) ;
  (C) module générique `compute` (plateformes d'orchestration abstraites), Kubernetes
  comme premier adaptateur.
- Recommandation : **A**. Le contrat de module (ADR 0026) est fait pour ça ; une build
  sans Kubernetes garde la 009 intacte ; C abstrait avant d'avoir un second cas.
- Décision : **A**, module `kube` dépendant de `machines` (l'utilisateur, 2026-11-09).

### Q-019-2 — Qui fait foi pour les objets Kubernetes (D1, D23)
- Constat : D1 fait de kartenn la source de vérité, sans découverte ni pilotage actif
  « pour l'instant », et prévoit des connecteurs d'import ou de synchronisation. Pour un
  namespace, un workload ou un service, la vérité est l'API du cluster : un objet déclaré
  à la main dans kartenn serait faux dès le déploiement suivant.
- Options : (A) **objets miroirs** : les objets `kube.*` sont écrits par le seul
  connecteur, par changesets, et lus par tous ; les **classifications** (axes de la
  politique) restent déclarées dans kartenn (X5) ; (B) kartenn reste déclaratif pour tout :
  le connecteur ne fait que comparer et lever des anomalies (pas d'écriture) ; (C) mixte
  par champ (le connecteur écrit, un humain peut surcharger).
- Recommandation : **A**, comme précision de D1 (une source qui fait foi pour sa propre
  portion) et non comme découverte : rien n'est deviné, tout est lu d'une API déclarative.
  B laisse le référentiel toujours en retard ; C rend l'origine d'un champ illisible.
- Décision : **A**, objets miroirs écrits par le seul connecteur, classifications
  déclarées dans kartenn ; amende D1 (l'utilisateur, 2026-11-09).

### Q-019-3 — Granularité des pods et de leurs adresses
- Constat : un pod est éphémère (réf. § Pod) ; son IP change à chaque recréation. Un
  cluster de 10⁴ pods qui se renouvellent une fois par jour écrirait 3,6·10⁶ adresses par
  an : à dix clusters et trois ans d'historique, 10⁸ versions, dix fois le plafond de D2,
  et 10⁴ changesets par jour et par cluster dans le budget de 50 écritures/s (D14).
- Options : (A) l'unité stable est le **workload** (contrôleur propriétaire des pods) ;
  pas d'objet pod ni d'adresse du noyau par pod ; les IP de pods vont dans un **journal
  d'observation** à rétention bornée (30 jours par défaut), hors historique bitemporel,
  qui répond à « qui avait l'IP X à l'instant T » ; (B) chaque IP de pod devient une
  adresse du noyau (nouvel usage `ephemeral`) ; (C) aucune trace des IP de pods : seules
  les plages de pods (cluster, nœud) sont connues.
- Recommandation : **A**. B dépasse D2 et D14 ; C rend impossible l'enquête sur un flux
  constaté (« quelle application avait cette IP ? »), besoin de la sorte `observed` (D30
  amendée).
- Décision : **A**, le workload pour unité stable, les IP de pods au journal
  d'observation (l'utilisateur, 2026-11-09).

### Q-019-4 — Le journal d'observation hors changesets (D8, D11)
- Constat : avec Q-019-3 A, le journal reçoit des milliers de lignes par heure. Le faire
  passer par le pipeline des changesets (validation, différentiel, approbation) coûterait
  sans rien contrôler : une observation ne se refuse pas.
- Options : (A) une **catégorie « observations »**, distincte du référentiel : écrite par
  lots via une route dédiée sous un droit propre, non bitemporelle (période observée
  seulement), purgée à échéance, auditée par lot ; (B) un changeset par lot, sur une
  table non versionnée ; (C) pas de journal (revient à Q-019-3 C).
- Recommandation : **A**, à graver dans une ADR (« Observations : hors référentiel, hors
  changesets »), qui servira aussi aux flux constatés (sorte `observed`).
- Enjeux (développés à la demande de l'utilisateur, 2026-11-09) : A abandonne, pour ce
  journal seul, l'approbation, le bitemporel (ni correction ni `as_of`) et l'événement
  par ligne, et ouvre une seconde voie d'écriture. B n'évite pas d'amender : un
  changeset est du temps système immuable (D10), que la purge à échéance violerait, et
  sans purge l'historique croît sans fin (≈ 3,6·10⁷ opérations par an à 10 clusters) ;
  une politique d'approbation large retiendrait les lots. Le journal ne fonde aucun
  verdict, mais il sert à l'attribution en enquête : un connecteur compromis pourrait
  faire porter un flux à la mauvaise application ; d'où les garde-fous ci-dessous.
- Décision : **A avec garde-fous** (l'utilisateur, 2026-11-09) ; amende D8 et D11 (les
  observations n'en relèvent pas) ; ADR « Observations : hors référentiel, hors
  changesets », qui vaudra aussi pour les flux constatés (sorte `observed`) et toute
  télémétrie future. Garde-fous, à graver dans l'ADR et dans X7 :
  - un droit propre (`kube.observations.write` ou équivalent ; nommé
    `kube.observation.record` à la remise en accord, l'action `write` étant retirée
    par la 024, X7), réservé au compte de service du connecteur et borné à son cluster ;
  - validation à l'entrée : une IP hors des plages de pods ou de nœuds du cluster est
    refusée ;
  - ajout seul : droits de base limités à l'insertion et à la fermeture de période,
    jamais de réécriture ;
  - chaque lot inscrit à l'audit scellé (012) avec l'empreinte de son contenu, pour
    qu'une altération ultérieure se détecte ;
  - lecture filtrée par le périmètre du cluster (la correspondance IP → workload dévoile
    la topologie applicative) ;
  - séparation stricte : une observation n'écrit jamais dans le référentiel ; passer
    d'un fait observé à un objet déclaré se fait par un changeset ;
  - limite assumée : au-delà de la rétention (30 jours par défaut, de 1 à 400), l'enquête
    n'est plus possible ; l'audit garde la trace du lot, pas son contenu ;
  - exploitation : table partitionnée par temps (purge par détachement de partition),
    index GiST sur (IP, période).

### Q-019-5 — Domaine d'adressage des pods et des services
- Constat : les plages de pods et de services se recouvrent souvent d'un cluster à l'autre
  (plages par défaut réutilisées) ; certaines installations routent pourtant les IP de pods
  dans l'adressage de l'entreprise (réf. § Modèle réseau).
- Options : (A) un domaine par cluster, désigné à la déclaration du cluster, qui reçoit
  les plages de pods et de services ; un même domaine peut être désigné si les pods sont
  routés dans l'adressage de l'entreprise ; (B) toujours le domaine de l'infrastructure ;
  (C) un domaine partagé par tous les clusters.
- Recommandation : **A** (D4 : un domaine par espace qui peut se recouvrir), avec un
  domaine par cluster proposé par défaut.
- Décision : **A**, un domaine par cluster, désigné à sa déclaration (l'utilisateur, 2026-11-09).

### Q-019-6 — Forme du connecteur (D7)
- Constat : D7 range les connecteurs parmi les clients de l'API REST, hors processus ;
  l'ADR 0023 permet aussi des tâches de fond dans le serveur.
- Options : (A) **processus distinct**, client REST de kartenn, livré dans la même image
  (sous-commande `kartenn kube-sync`, D35 amendée : une seule image), un processus par
  cluster ou par groupe de clusters ; (B) tâche de fond du serveur qui ouvre la
  connexion au cluster ; (C) import périodique seulement (liste complète toutes les N
  minutes, sans watch).
- Recommandation : **A**, avec watch (réf. § API, watch). B fait porter les identifiants
  des clusters par chaque instance du serveur et contredit D7 ; C laisse le référentiel
  en retard de N minutes et coûte une liste complète à chaque passe. Le mode C reste le
  repli du connecteur quand le watch est indisponible.
- Décision : **A**, processus distinct client REST, list puis watch, liste périodique en repli (l'utilisateur, 2026-11-09).

### Q-019-7 — Les labels Kubernetes et les axes de la politique (D21, D23, D28)
- Constat : une règle doit viser un workload par ses labels et son namespace. D21 a
  rejeté les étiquettes libres comme classification ; un développeur qui peut modifier
  les labels de son namespace pourrait, si les labels fondaient un verdict, s'ouvrir un
  flux.
- Options : (A) **correspondance label → valeur d'axe** déclarée dans kartenn par un
  administrateur, **bornée par namespace** (valeurs admises) ; une valeur hors des
  valeurs admises est ignorée et signalée ; le namespace lui-même est un axe (référence
  externe) ; les règles restent par attributs (D28) ; (B) les labels ne donnent qu'un
  **attendu** (règle d'inférence, D23) ; la classification se déclare dans kartenn sur le
  namespace ou le workload ; (C) sélecteurs de labels bruts dans les règles de la 010.
- Recommandation : **A**. Elle garde le modèle de règles de la 010 et la liste fermée de
  D21 ; la borne par namespace empêche l'escalade. B est sûr mais oblige à recopier
  chaque classification ; C réintroduit l'étiquette libre et contourne l'obligation par
  axe (REQ-010-21).
- Décision : **A**, correspondance label → valeur d'axe déclarée dans kartenn, bornée par namespace (l'utilisateur, 2026-11-09).

### Q-019-8 — Unité de verdict pour un workload (D22)
- Constat : D22 évalue chaque paire d'adresses. Les adresses d'un workload sont
  interchangeables et changent sans cesse.
- Options : (A) une extrémité « workload » (ou namespace, service) est **une extrémité
  logique** : un verdict par paire d'extrémités logiques, sur les axes résolus du
  workload ; (B) développer le workload en IP de pods observées à la date (instable,
  dépend du journal) ; (C) développer en plages de pods des nœuds où il tourne.
- Recommandation : **A**, comme précision de D22 pour les workloads ; les extrémités
  adresse, préfixe, interface et machine gardent la règle de D22.
- Décision : **A**, extrémité logique ; amende D22 pour les workloads, namespaces et services (l'utilisateur, 2026-11-09).

### Q-019-9 — Objet disparu du cluster mais cité dans kartenn
- Constat : la 011 refuse la clôture d'une extrémité d'un flux actif (REQ-011-3) ; le
  connecteur doit pourtant refléter une suppression dans le cluster.
- Options : (A) le connecteur clôt un objet sans dépendant ; un objet cité passe à l'état
  **absent** (date d'absence, anomalie `kube.object.absent`), et un humain clôt en
  cascade ou attend son retour (recréation de même nom) ; (B) cascade automatique (les
  flux disparaissent en silence) ; (C) refus (le connecteur reste bloqué).
- Recommandation : **A**.
- Décision : **A**, état « absent » pour un objet cité, clôture pour un objet sans dépendant (l'utilisateur, 2026-11-09).

### Q-019-10 — Interactions avec le cluster (NetworkPolicy)
- Constat : traduire les flux autorisés en NetworkPolicy est tentant, mais D1 exclut le
  pilotage actif et la 011 exclut la génération de règles de pare-feu. La NetworkPolicy
  est une union d'autorisations sans refus explicite, sans journalisation, et sans effet
  si le plugin réseau ne la met pas en œuvre (réf. § NetworkPolicy).
- Options : (A) **hors de la 019** ; évolution 1 : lire les NetworkPolicy et lever des
  écarts déclaré/appliqué ; évolution 2 : produire des manifestes à appliquer par la
  chaîne de déploiement de l'utilisateur (kartenn n'écrit jamais dans le cluster) ;
  chacune par une spec ultérieure ; (B) écriture directe dans le cluster, sur option ;
  (C) aucune interaction, jamais.
- Recommandation : **A**. L'écriture directe exige des droits d'écriture sur le cluster
  (rupture de « lecture seule par défaut ») et une nouvelle décision sur D1.
- Décision : **A**, hors de la 019 ; lecture des NetworkPolicy puis production de manifestes par des specs ultérieures, kartenn n'écrivant jamais dans le cluster (l'utilisateur, 2026-11-09).

### Q-019-11 — Hyperviseur
- Constat : un hyperviseur est une machine `physical` qui héberge des VM ou est membre
  d'un cluster de virtualisation ; la 009 sait déjà le dire (M5, M6).
- Options : (A) pas de nouveau type : rôle **dérivé** (lecture « hyperviseurs ») et
  propriété facultative `machines.platform` (technologie, liste fermée administrable,
  livrée vide) ; (B) nouveau `kind` `hypervisor` (migration de `kind`, table de vérité de
  M5 à refaire).
- Recommandation : **A** (M2 : le type porte des invariants, le reste est propriété).
- Décision : **A**, rôle dérivé et propriété facultative `machines.platform` (l'utilisateur, 2026-11-09).

### Q-019-12 — Dépendance : client Kubernetes en Rust
- Constat : lister, surveiller (watch), reprendre après `410 Gone` et paginer demandent un
  client qui suive les conventions de l'API (réf. § API).
- Options : (A) un crate client Kubernetes établi, évalué par `crate-scout` (licence
  compatible AGPL-3.0, maintenance, `unsafe`, dépendances) ; (B) client minimal écrit sur
  le client HTTP déjà présent (list, watch, pagination en JSON, types écrits à la main pour
  les seuls champs lus).
- Recommandation : **A** si l'évaluation est bonne, sinon B ; décision à la tâche 2.
- Décision : **A** si l'évaluation `crate-scout` est bonne, sinon B ; la fiche revient à l'arbitrage à la tâche 2 (nouvelle dépendance) (l'utilisateur, 2026-11-09). Remise en accord : la crate du connecteur naît à la tâche 9 de [`tasks.md`](tasks.md), qui porte la fiche (la tâche 2 est celle des ajouts à la 009).
- Fiche (`crate-scout`, 2026-11-11) : `kube` 5.0.0 (`kube-client`, `kube-runtime`) et `k8s-openapi` 0.29.0, Apache-2.0 (compatible AGPL-3.0), maintenus, aucun `unsafe` ; pagination, reprise du watch sur `410`, signets et relecture du jeton depuis un fichier déjà écrits ; 56 crates tierces de plus, aucune seconde pile TLS ou HTTP (rustls, ring, hyper déjà présents). Réserves : compilation de `k8s-openapi` non mesurée ; repli sur les racines système si aucune CA n'est donnée.
- Décision (fiche) : **A**, `kube` et `k8s-openapi` (l'utilisateur, 2026-11-11), aux conditions suivantes, tenues par la tâche 9 : `default-features = false` (ni aws-lc-rs ni racines du système), une seule version d'API de `k8s-openapi`, refus de démarrer sans CA ou avec `accept_invalid_certs`, temps de compilation mesuré sous `k-heavy` ; au-delà d'environ deux minutes pour `k8s-openapi` seul, la tâche s'arrête et revient à l'arbitrage (B reste le repli).
- Mesure (tâche 9, 2026-11-11) : `k8s-openapi` 0.29.0 seul, feature `v1_33`, profil dev (dépendances en `opt-level = 2`), `--all-features`, sous `k-heavy` avec un autre build en cours : 59 s (rapport `--timings`), sous la limite ; `kube` sans ses features par défaut (`client`), pile TLS montée par le connecteur sur rustls et ring.

### Q-019-13 — Ce qui glisse avec passerelles, routes et NAT
- Constat : l'amendement de D31 déplace « passerelles, routes, NAT » en v0.4. La 010 et
  la 011 placent aussi en « v0.3 » les flux constatés (sorte `observed`), les connecteurs
  de ticketing et les campagnes de revue.
- Options : (A) ils glissent aussi en v0.4 ; la 019 réserve seulement ce que les flux
  constatés devront lire (journal d'observation) ; (B) ils restent en v0.3 avec le
  calcul.
- Recommandation : **A** (une v0.3 consacrée au calcul) ; les mentions de la 010 et de la
  011 sont corrigées après l'arbitrage.
- Décision : **A**, flux constatés, connecteurs de ticketing et campagnes de revue glissent en v0.4 ; mentions de la 010 et de la 011 à corriger (l'utilisateur, 2026-11-09).

### Q-019-14 — Identifiants des objets du noyau créés par un module (tâche 3a)
- Constat : le pipeline tire les identifiants des opérations du noyau qu'un module
  planifie (ADR 0026 clause 6) après le plan du module. Les données du module ne peuvent
  donc pas citer les objets créés dans le même changeset : un nœud ne cite pas les
  réseaux de ses plages de pods, un service ne cite pas ses adresses `vip` (tâche 4).
- Options : (A) le pipeline rend au module les identifiants créés par un crochet par
  défaut (`ModuleHandler::planned`), amendement additif de la clause 6 de l'ADR 0026 ;
  le nœud reçoit une colonne des plages observées (`pod_cidrs cidr[]`) pour que la règle
  `kube.node.range_outside` reste au serveur ; (B) le connecteur crée lui-même les
  réseaux par des `create_prefix` à locaux avant `kube.sync`, contre la table
  « Opérations » du design ; (C) expédients (opération mise en file après les créations,
  `range_ids` réécrits dans `check`), écartés.
- Recommandation : **A**.
- Décision : **A**, crochet `planned`, amendement additif de la clause 6 de l'ADR 0026,
  colonne `kube.node.pod_cidrs` (l'utilisateur, 2026-11-10).

### Q-019-15 — Domaine des pods d'un cluster peuplé (revue de la tâche 3b)
- Constat : changer `pod_domain_id` d'un cluster qui a des nœuds ou des namespaces
  vivants laisserait leurs plages et leurs objets ancrés dans l'ancien domaine.
- Options : (A) refuser le changement tant que des objets vivent
  (`kube.operation.invalid`) : le domaine des pods se fixe dès que le cluster est
  peuplé, on le vide ou on en déclare un autre ; (B) un vrai ré-ancrage, sous les droits des deux
  domaines et leurs deux verrous (ADR 0015).
- Recommandation : **A**.
- Décision : **A**, le refus livré par la tâche 3b reste la règle ; pas de ré-ancrage
  (l'utilisateur, 2026-11-10).

### Q-019-16 — Objets du noyau possédés par un objet de module (revue de la tâche 3c)
- Constat : un nœud clos par la cascade de son cluster laisse vivants les réseaux de ses
  plages de pods ; la référence `node` → `prefix` en refus ne joue que quand le préfixe
  se clôt. Les adresses `vip` des services (tâche 4) ont le même besoin.
- Options : (A) une politique déclarative sur la référence (objet du noyau possédé) :
  quand la source se clôt, en cascade ou non, le noyau clôt la cible ; (B) un crochet
  `ModuleHandler::closing` appelé par la cascade, impératif, à écrire par module ;
  (C) refuser la clôture d'un cluster dont les nœuds citent des réseaux, contraire au
  design.
- Recommandation : **A**.
- Décision : **A**, amendement additif de l'ADR 0026 (clause des références)
  (l'utilisateur, 2026-11-10).

### Q-019-17 — Adresses `vip` partagées (revue de la tâche 4c)
- Constat : les ingress d'une classe reçoivent l'IP du service LoadBalancer de leur
  contrôleur, et deux services peuvent partager une IP ; une adresse `vip` possédée par
  un seul objet fait refuser le second porteur (`core.address.duplicate`), et le lot
  entier du connecteur avec lui.
- Options : (A) possession partagée : une adresse déjà possédée par un objet `kube.*`
  vivant du même cluster est réutilisée, elle se clôt avec son dernier porteur ; une
  adresse sans porteur `kube.*` du cluster n'est jamais adoptée ; (B) réutiliser sans
  posséder (l'adresse peut rester sans porteur) ; (C) adresses d'ingress hors du noyau,
  contraire à X6 et REQ-019-14.
- Recommandation : **A**.
- Décision : **A** (l'utilisateur, 2026-11-10) ; tâche 4e. Le changement des domaines
  des services et des adresses externes d'un cluster peuplé suit la règle de Q-019-15
  (refus, tâche 4f ; Claude, 2026-11-11).
- Précision (tâche 4e, 2026-11-11) : dans un même lot, deux porteurs nouveaux d'une IP
  ou une passation demandent que le pipeline partage une création du noyau entre les
  plans d'un changeset ; amendement additif 10 de l'ADR 0026 (créations partageables,
  tâche 4e0) plutôt qu'un repli sur le connecteur (l'utilisateur, 2026-11-11).

### Q-019-18 — Auteur des écritures des miroirs (revue de la tâche 5a)
- Constat : appliquer `kube.connector.direct` demande qu'une règle distingue les
  opérations envoyées par l'auteur de celles que `kube.sync` planifie ; un membre d'un
  groupe de connecteur par un groupe d'émetteur (`idp_group`) échappait à la règle ; un
  `Close` à la main d'un miroir encore vivant dans le cluster laissait le miroir faux,
  contre X5.
- Décisions (l'utilisateur, 2026-11-11, toutes sur la recommandation) :
  - `CheckContext::operations` (les seules opérations de l'auteur) ratifié, amendement
    additif 11 de l'ADR 0026 ;
  - `kube.connector.direct` reconnaît tout membre du groupe, direct ou par un groupe
    d'émetteur, selon la résolution du RBAC ; `kube.object.managed` exige toujours un
    compte de service membre direct ;
  - un `Close` direct d'un miroir hors `kube.sync` est refusé (`kube.object.managed`) ;
    la restauration qui annule un lot reste permise (X5) ; retirer un objet passe par le
    cluster, ou par la clôture du cluster en cascade.

### Q-019-19 — Forme des lignes de la section `kube.objects` (revue de la tâche 5c)
- Constat : les lignes de la section de différentiel sont lues par la 010, la 011 et par
  l'API (`GET` des sections d'un changeset) ; une première version rendait la ligne de la
  table sans ses colonnes de versionnement (`to_jsonb`), qui fait de chaque migration du
  schéma `kube` un changement du contrat, et des noms de colonnes (`namespace_id`,
  `range_ids`) et du format d'instant de la session (`absent_since`) le contrat.
- Options : A. colonnes brutes de la table ; B. JSON explicite par type, une liste fermée
  de champs aux noms stables.
- Décision (l'utilisateur, 2026-11-11) : B. Chaque type (nœud, namespace, workload,
  service, ingress) rend son identité, ses champs suivis et `absent_since` en RFC 3339
  normalisé (UTC, comme le reste de REST), sous les noms du design « Section de
  différentiel » ; une migration ne change plus le contrat.

## Synthèse
- X1 périmètre : hôte physique, hyperviseur, VM, cluster d'hyperviseurs (009) ; cluster,
  nœud, namespace, workload, service, ingress Kubernetes ; pod en observation seulement ;
  conteneur d'un pod non modélisé.
- X2 module `kube` dépendant de `machines` ; la 009 reçoit des ajouts, rien ne migre de
  force.
- X3 workload = contrôleur racine des pods (chaîne `ownerReferences`), identité stable
  (cluster, namespace, sorte, nom), UID en attribut.
- X4 champs suivis : seuls les champs utiles au réseau et aux flux sont versionnés ;
  réplicas, image, état ignorés ; labels suivis par paramètres du module et par cluster.
- X5 objets miroirs écrits par le connecteur (compte membre du groupe que désigne le
  cluster) ; classifications déclarées dans kartenn.
- X6 adressage : plages de pods (cluster, nœud) et de services comme préfixes ; IP de
  service et adresses externes comme adresses `vip` du noyau ; IP de pods au journal.
- X7 journal d'observation des pods hors référentiel (ADR 0043) : période observée,
  rétention 30 jours, purge par partition ; garde-fous de Q-019-4 (droit propre, IP
  validées, ajout seul, audit par lot avec empreinte, lecture bornée, séparation).
- X8 connecteur hors processus, list puis watch, reprise sur `410 Gone`, lots en
  changesets ; fenêtre de lot propriété du cluster.
- X9 rapprochement : clé naturelle, identifiant externe, nœud ↔ machine par adresse puis
  nom ; objets absents.
- X10 sécurité : lecture seule par défaut, RBAC minimal côté cluster, jetons à durée
  limitée, vérification de ses propres droits au démarrage ; compte de service kartenn
  membre d'un groupe, rôle `kube_connector` sur les domaines du cluster (ADR 0044).
- X11 politique : workload, namespace et service comme porteurs d'axes et extrémités ;
  labels → axes bornés par namespace, paramètres `kube.settings.*`.
- X12 flux : nouvelles extrémités, réévaluation sur changement d'axe ou de sélection.
- X13 interactions avec le cluster : évolutions hors de la 019.
- X14 budgets et volumétrie de référence.
- X15 ordre de travail.
- X16 permissions (grammaire de la 024), rôles livrés, événements.

## X1 — Périmètre
- Décision : la 019 décrit le calcul **du point de vue du réseau et des flux** :
  où tourne une charge, quelles adresses elle utilise, ce qu'elle expose. Objets :
  hôte physique, hyperviseur, VM et cluster d'hyperviseurs (existants, 009) ; cluster
  Kubernetes, nœud, namespace, workload, service, ingress (nouveaux). Le pod n'est connu
  que par le journal d'observation (X7) ; le conteneur d'un pod n'est pas modélisé : il
  partage l'espace réseau du pod (réf. § Pod), ses ports déclarés sont résumés sur le
  workload, à titre informatif. Le conteneur hors Kubernetes reste la machine `container`
  de la 009.
- Pourquoi : la politique raisonne sur des extrémités et des services ; ni l'image, ni le
  nombre de réplicas, ni l'état d'un conteneur ne changent un verdict.
- Rejeté : inventaire complet du cluster (ConfigMap, volumes, quotas) ; Gateway API dès
  la 019 (réf. § Ingress : recommandée à la place de l'Ingress, mais extension hors du
  cœur de l'API ; à reprendre par une spec ultérieure).
- Décidé par : Claude

## X2 — Module `kube` et ajouts à la 009
- Décision : module `kube` (crate `kartenn-kube`), dépendant de `machines`
  (Q-019-1) et d'aucun autre module : le VLAN est un objet du cœur (026/C1, C2), il n'y
  a plus de module `vlan`. Feature `kube` de
  `kartenn-server`, active par défaut comme les autres modules métier. Types : `kube.cluster`, `kube.node` (parent : cluster), `kube.namespace`
  (parent : cluster), `kube.workload`, `kube.service`, `kube.ingress` (parent :
  namespace). Ajouts à la 009, tous additifs : propriété `machines.platform` (Q-019-11),
  lecture des hyperviseurs, lecture « machine d'une adresse ou d'un nom » pour le
  rapprochement des nœuds. Les types et la table de vérité de M5 ne changent pas : un
  nœud est un objet `kube.node` qui **désigne** une machine `physical` ou `virtual`.
- Migration : aucune donnée de la 009 ne migre de force. Une machine `container` hébergée
  par une machine désignée comme nœud, dont le nom correspond à un pod observé, est
  signalée (`kube.machine.looks_like_pod`, avertissement) avec la suggestion de la clore :
  c'est la trace d'un inventaire antérieur qui modélisait les pods en conteneurs. Un
  `machines.cluster` qui servait à regrouper les nœuds d'un cluster Kubernetes reste un
  cluster de virtualisation ; le `kube.cluster` peut le citer (`machines_cluster`,
  facultatif) pour l'affichage.
- Rejeté : nœud comme nouveau `kind` de machine (un nœud est un rôle d'une machine, qui
  peut changer de cluster) ; ouvrir `member_cluster_id` aux VM pour les nœuds (mélange
  deux sens de « cluster » dans une même colonne).
- Décidé par : l'utilisateur (2026-11-09, Q-019-1 et Q-019-11) ; liste des types,
  migration et feature par défaut : Claude

## X3 — Workload : unité stable
- Décision : un workload est le **contrôleur racine** d'un pod, atteint en
  remontant `metadata.ownerReferences` (le contrôleur, `controller: true`) : Deployment
  (par son ReplicaSet), StatefulSet, DaemonSet, CronJob (par son Job), Job sans
  propriétaire, ReplicaSet sans propriétaire ; un pod sans propriétaire est son propre
  workload, de sorte `pod` (réf. § Pod, ownerReferences). Identité kartenn : (cluster,
  namespace, sorte, nom), identifiant externe `kube:<cluster>/<namespace>/<sorte>/<nom>`
  (REQ-002-41) ; l'UID Kubernetes est un attribut. Un objet supprimé puis recréé sous le
  même nom (nouvel UID, réf. § Noms et UID) reste le même workload kartenn : une version
  nouvelle note le changement d'UID.
- Précision (tâche 019-1) : la remontée s'arrête à un Deployment, un StatefulSet, un
  DaemonSet ou un CronJob, même quand il a lui-même un contrôleur (un StatefulSet créé par
  un opérateur est le workload) ; un Job ou un ReplicaSet n'est workload que sans
  contrôleur ; un pod dont le contrôleur est son `v1 Node` (pod miroir d'un pod statique)
  est son propre workload, comme un pod nu. Une sorte est reconnue par son groupe d'API et
  son `kind`. Un contrôleur de sorte inconnue au-dessus d'un ReplicaSet, d'un Job ou d'un
  pod (autre que son nœud), un
  propriétaire introuvable ou d'un autre UID, une chaîne trop longue ne désignent aucun
  workload : le domaine rend une erreur (`ForeignController`, `Missing`, `TooDeep`) et le
  connecteur en décide (tâche 10).
  Décidé par : Claude, revue de la 019-1, 2026-11-09.
- Pourquoi : les flux citent une application, pas une instance ; une recréation par
  « supprimer puis appliquer » ne doit pas rompre les flux.
- Rejeté : identité par UID (chaque recréation casserait les flux) ; ReplicaSet comme
  workload d'un Deployment (un ReplicaSet par version déployée).
- Décidé par : l'utilisateur (2026-11-09, Q-019-3 : le workload pour unité stable) ;
  remontée et identité : Claude

## X4 — Champs suivis
- Décision : seuls sont versionnés les champs qui servent au réseau ou aux
  flux : pour un workload, sorte, nom, labels **suivis**, compte de service, ports
  déclarés, `hostNetwork` ; pour un service, type, sélecteur, ports, IP de cluster, IP
  externes, workloads sélectionnés ; pour un ingress, classe, règles (hôte, chemin, service
  et port), hôtes TLS, adresses ; pour un nœud, nom, adresses, plages de pods, machine
  désignée, zone ; pour un namespace, nom et labels suivis. Labels suivis : les clés
  citées par une correspondance d'axe (X11), celles de la liste du module (paramètre
  `kube.settings`, 027/P4 : global, en changeset, sous `kube.settings.edit`) et celles
  de la liste propre du cluster (`tracked_label_keys`, propriété du `kube.cluster`
  écrite par changeset sous `kube.cluster.edit`, comme sa fenêtre de lot, X8) ; les
  autres sont ignorés. Réplicas, images, états, annotations : ignorés.
- Pourquoi : un déploiement change l'image et souvent un label de version ; versionner
  ces champs ferait une version par déploiement (10⁵ workloads × un déploiement par
  semaine = 1,5·10⁷ versions en trois ans).
- Rejeté : copie complète de l'objet (historique et volume) ; tous les labels (les labels
  de version et de hachage changent à chaque déploiement).
- Décidé par : Claude ; labels suivis en paramètres du module : l'utilisateur
  (2026-11-09, paramètres génériques, pas d'action propre)

## X5 — Objets miroirs
- Décision (Q-019-2 = A) : les champs suivis des objets `kube.*` ne s'écrivent que par
  un **compte de service membre du groupe du connecteur** que désigne leur cluster
  (`connector_group`, groupe kartenn, 025/G1, G2 : aucun contrôle nominal ;
  `kube.object.managed`, erreur pour tout autre auteur, personne comprise même membre du
  groupe) ; les propriétés portées par ces objets (axes, `core.owner` hérité du cluster)
  restent écrites par les humains. Chaque
  lot du connecteur est un changeset ordinaire : validé, journalisé, annulable par un
  administrateur (l'annulation est réécrite au lot suivant si le cluster n'a pas changé).
- Pourquoi : D1 (cohérence bloquante) s'applique au miroir ; l'audit sait qui a écrit.
- Rejeté : écriture directe en base par le connecteur (contourne D7 et l'audit) ;
  désignation d'un compte nominal par le cluster (contraire à 025/G1).
- Décidé par : l'utilisateur (2026-11-09, Q-019-2, amende D1) ; désignation par groupe :
  Claude (remise en accord avec la 025)

## X6 — Adressage
- Décision (Q-019-5 = A) : à la déclaration d'un `kube.cluster`,
  l'administrateur désigne le **domaine des pods** et le **domaine des services** (un
  domaine propre par défaut, D4). Les plages de pods du cluster sont des **conteneurs**
  (REQ-002-13) déclarés par l'administrateur ; la plage de pods d'un nœud
  (`spec.podCIDRs`, au plus une par famille, réf. § Node) est un **réseau** que le
  connecteur crée sous le conteneur et rattache au nœud ; la plage de services (paramètre
  du plan de contrôle, non lisible par l'API, réf. § Service) est déclarée par
  l'administrateur comme réseau. Les IP de cluster d'un service et ses adresses externes
  (LoadBalancer, `externalIPs`) sont des **adresses du noyau** d'usage `vip`, créées par le
  connecteur dans le domaine désigné et référencées par le service (refus de clôture). Les
  IP de pods ne sont jamais des adresses du noyau (Q-019-3) : une adresse de pod se résout
  en classification par l'arbre (la plage du nœud) et, au journal, en workload.
- Pourquoi : la plage d'un nœud change rarement (à l'ajout ou au retrait d'un nœud) ;
  l'IP d'un service est stable pendant la vie du service (réf. § Service).
- Rejeté : plage d'usage DHCP pour les pods (le noyau contrôlerait des adresses qu'il ne
  connaît pas) ; IP de service en propriété texte (ni unicité ni arbre).
- Décidé par : l'utilisateur (2026-11-09, Q-019-5) ; formes de l'adressage : Claude

## X7 — Journal d'observation des pods
- Décision (Q-019-3 = A, Q-019-4 = A avec garde-fous ; catégorie « observations » de
  l'[ADR 0043](../../docs/adr/0043-observations-hors-referentiel.md), D8 et D11
  amendées) : table `kube.pod_observation` : cluster, namespace, nom et UID du pod,
  workload, nœud, IP (une ligne par IP, au plus une par famille), `hostNetwork`, période
  observée `[début, fin)`, lot qui l'a ouverte, lot qui l'a fermée ; hors référentiel,
  hors bitemporel (pas de `as_of`, aucune correction), hors changesets ; écrite par lots
  par le connecteur (`POST /v1/kube-clusters/{id}/observations`) ; purgée par une tâche
  de fond (ADR 0023) au-delà de la **rétention** du cluster (30 jours par défaut, de 1
  à 400). Lue par « qui avait l'IP X à T » et par « où tourne le workload W » ; elle
  **ne fonde aucun verdict**.
- Garde-fous (Q-019-4) :
  - **droit propre** : `kube.observation.record` pour écrire, `kube.observation.read`
    pour lire ; noms admis par `catalogue_problems` (la ressource `observation` n'est
    pas une action) et absents de `RENAMED_PERMISSIONS` ; `write`, action retirée par la
    024 (M7), n'est pas employée. `record` n'est accordé que par le rôle
    `kube_connector` (X10) ; aucun des deux n'entre dans les rôles livrés du lecteur, de
    l'éditeur ou de l'approbateur (X16) ;
  - **borné au cluster** : un lot n'est accepté que d'un compte de service membre du
    groupe du connecteur du cluster de la route, et `record` s'évalue sur le périmètre
    du cluster (ses domaines désignés, X10) ;
  - **validation à l'entrée** : le lot est refusé en entier (422,
    `kube.observation.invalid`, lignes nommées) si une IP est hors des plages de pods du
    cluster (conteneurs déclarés) et des adresses de ses nœuds, ou si un workload ou un
    nœud cité n'est pas du cluster ;
  - **ajout seul** : un lot ne fait qu'ouvrir des observations et fermer la période
    d'observations ouvertes (borne haute posée une fois) ; les droits PostgreSQL du rôle
    de l'application se limitent à l'insertion et à la pose de cette borne, gardée par un
    déclencheur ; toute autre mise à jour et toute suppression ligne à ligne sont
    refusées ; seule la purge supprime, par partition entière ;
  - **audit par lot** : chaque lot accepté inscrit dans sa transaction l'événement
    `observation.batch.recorded` au journal scellé (012) : cluster, nombres
    d'ouvertures et de fermetures, empreinte SHA-256 du contenu canonique du lot ; une
    tâche de fond recalcule l'empreinte des lots encore entiers dans la rétention et
    inscrit `observation.batch.altered` (ERROR) pour un lot altéré ;
  - **lecture bornée** au périmètre du cluster : la correspondance IP → workload dévoile
    la topologie applicative ;
  - **séparation stricte** : une observation n'écrit jamais dans le référentiel, aucune
    règle, aucun verdict ni différentiel ne la lit ; un lot n'émet aucun événement
    `changes` (022) ; passer d'un fait observé à un objet déclaré se fait par un
    changeset ;
  - **limite assumée** : au-delà de la rétention, l'enquête n'est plus possible (réponse
    vide, motif `retention`) ; l'audit garde la trace du lot, pas son contenu ;
  - **exploitation** : partitions par temps (purge par partition entière, une
    observation encore ouverte jamais purgée, rétention comptée depuis la fin de
    l'observation), index GiST sur (cluster, IP, période).
- Pourquoi : enquête et futur audit des flux constatés, sans peser sur l'historique ; le
  journal sert à l'attribution, d'où les garde-fous contre un connecteur compromis.
- Volume : 10 clusters × 10⁴ pods renouvelés chaque jour × 30 jours = 3·10⁶ lignes.
- Rejeté : rétention illimitée (croissance sans fin) ; agrégation par plage de nœud
  seulement (perd le workload, utile à l'enquête) ; acceptation partielle d'un lot (une
  empreinte ne couvrirait plus ce qui a été envoyé).
- Décidé par : l'utilisateur (2026-11-09, Q-019-3, Q-019-4 et ses garde-fous) ; noms des
  droits et des événements, refus du lot entier, détail des partitions : Claude

## X8 — Synchronisation
- Décision (Q-019-6 = A, [ADR 0044](../../docs/adr/0044-connecteur-kubernetes.md)) :
  par type suivi, le connecteur **liste** (pages par
  `limit` et `continue`), puis **surveille** à partir de la `resourceVersion` de la liste,
  avec signets (`allowWatchBookmarks`) ; sur `410 Gone`, il reliste (réf. § API). La
  `resourceVersion` est rendue telle quelle au serveur et ne sert qu'à reprendre ; le
  connecteur ne l'ordonne pas (l'ordre n'est garanti que pour les types du serveur
  d'API, réf. § API). Il projette chaque objet sur ses champs suivis (X4), compare à
  l'état de kartenn (lu par l'API), regroupe les différences par fenêtre (10 s ou
  500 opérations par défaut ; propriété du `kube.cluster`, écrite par changeset, lue par
  le connecteur au démarrage et à chaque lot) en un changeset idempotent (`Idempotency-Key` dérivée du
  cluster et de la fenêtre). Une liste complète **rapproche** : tout objet de kartenn
  absent de la liste est clos ou marqué absent (X9). Mode de repli : liste complète
  périodique (Q-019-6 C). Dates : `valid_from` d'une création = `creationTimestamp` ;
  une modification ou une suppression prend l'instant d'observation.
- Pourquoi : délai de quelques secondes, coût proportionnel aux changements.
- Décidé par : l'utilisateur (2026-11-09, Q-019-6) ; fenêtre, idempotence et dates :
  Claude

## X9 — Rapprochement et objets absents
- Décision : un objet `kube.*` se retrouve par son identifiant externe (X3). Un
  nœud désigne une machine par ses adresses (`status.addresses` d'InternalIP, réf.
  § Node) via le port de la 009, sinon par son nom (nom d'hôte, même propriétaire que le
  cluster) ; une correspondance ambiguë ou absente ne lie rien et lève
  `kube.node.unmatched` (avertissement). Le connecteur ne crée pas de machine, sauf
  option du cluster (`create_node_machines`). Un objet disparu sans dépendant est clos ;
  un objet cité (flux, référence d'un autre module) passe **absent** (Q-019-9) ; il
  redevient présent si un objet de même identité réapparaît.
- Décidé par : l'utilisateur (2026-11-09, Q-019-9 : objets absents) ; rapprochement des
  nœuds : Claude

## X10 — Sécurité du connecteur
- Décision ([ADR 0044](../../docs/adr/0044-connecteur-kubernetes.md)) :
  - **Côté cluster** : compte de service dédié, ClusterRole livrée qui n'accorde que
    `get`, `list`, `watch` sur `nodes`, `namespaces`, `pods`, `services`,
    `endpointslices`, `ingresses` et les contrôleurs (`deployments`, `replicasets`,
    `statefulsets`, `daemonsets`, `jobs`, `cronjobs`) ; jamais `secrets` (`list` et
    `watch` révèlent le contenu, réf. § Sécurité d'accès), ni `configmaps`, ni
    sous-ressources (`pods/exec`, `pods/log`). Au démarrage, le connecteur interroge ses
    propres droits (revue d'accès de l'API d'autorisation) et **refuse de démarrer** s'il
    peut écrire quoi que ce soit ou lire les secrets, sauf option explicite consignée dans
    les logs et l'audit.
  - **Identifiants côté cluster** : jeton lié à durée limitée (TokenRequest) ou
    certificat client, lus d'un fichier relu à chaque renouvellement ; jamais un jeton
    sans échéance ; CA du serveur d'API obligatoire, pas d'option « non vérifié ».
  - **Côté kartenn** : un compte de service par connecteur (D13, secret généré, 005/K8),
    membre direct d'un groupe kartenn propre au cluster (025/G2) que le cluster désigne
    (`connector_group`) ; aucune liaison nominale (025/G1, G3). Le groupe est lié au rôle
    livré `kube_connector` (clé de rôle sans point) sur le périmètre des **domaines
    désignés** du cluster (pods, services, adresses externes). Pour que ce périmètre
    couvre le cluster, `kube.node`, `kube.namespace` et `kube.workload` sont ancrés sur
    le domaine des pods, `kube.service` sur celui des services, `kube.ingress` sur celui
    des adresses externes, à défaut des services (ancrage de domaine, ADR 0026 clause 4 ;
    le cluster lui-même n'est pas ancré). Contenu du rôle : `kube.cluster.read`,
    `kube.{node,namespace,workload,service,ingress}.{read,edit,approval.self}`,
    `core.domain.read`, `core.prefix.{read,edit,approval.self}`,
    `core.address.{read,edit,approval.self}`, `prop.*.edit`, `prop.*.approval.self`,
    `kube.observation.record`. Deux clusters pouvant partager un domaine (Q-019-5),
    deux règles d'erreur bornent le compte à son cluster et à la synchronisation :
    `kube.object.managed` (X5) et `kube.connector.direct` (un changeset d'un compte
    membre d'un groupe de connecteur ne porte que des opérations `kube.sync`, donc ce
    que le module planifie). Le rôle est **livré par le module** : ajout additif au
    contrat de module (ADR 0026 clause 8), porté par l'ADR 0044. Identifiants lus d'un
    fichier, comme `url_file` (D35 amendée).
  - **Minimisation** : un pod lu porte ses variables d'environnement en clair (hors
    références de secrets) ; le connecteur ne garde en mémoire que les champs suivis et
    n'envoie rien d'autre à kartenn ni aux logs.
  - **Lecture seule par défaut** : aucune fonction du connecteur n'écrit dans le cluster.
- Pourquoi : moindre privilège (réf. § Sécurité d'accès) ; un connecteur compromis ne
  doit rien pouvoir modifier.
- Décidé par : Claude ; forme du connecteur (processus distinct, lecture seule) :
  l'utilisateur (2026-11-09, Q-019-6, Q-019-10). Le confinement côté kartenn (groupe,
  périmètre par domaines, `prop.*` borné par `kube.object.managed` et
  `kube.connector.direct`, ancrage de domaine de tous les types sauf le cluster) :
  validé par l'utilisateur le 2026-11-09 (tâche 0b).

## X11 — Politique : workloads porteurs d'axes
- Décision (Q-019-7 = A) : `kube.cluster`, `kube.namespace`,
  `kube.workload`, `kube.service` sont **porteurs** de propriétés (ADR 0026 clause 5) ;
  un axe peut avoir pour porteur le workload (amendement de REQ-010-29). Valeur d'un axe
  pour un workload : affectation propre, sinon héritage namespace → cluster le long de
  l'arbre du module (ADR 0026 clause 4), sinon défaut hérité de l'arbre d'adressage sur la
  plage de pods (comme REQ-010-31). Axe livré `kube.namespace` (référence externe,
  système `kube`), posé par le connecteur, pour « même namespace » (contrainte « même
  valeur », REQ-010-8). Correspondance label → axe : (clé de label, valeur de label) →
  valeur d'axe, avec, par namespace, les valeurs d'axe **admises** : ce sont des
  **paramètres du module** (`kube.settings.*`, 027/P4 et P5 : permissions globales, en
  changeset, sans action propre), écrits par les opérations `kube.set_label_mapping` et
  `kube.set_admitted_values` sous `kube.settings.edit` (`apply`, `approval.self`) ; les
  axes et leurs valeurs restent des paramètres de la politique (`policy.settings.*`,
  REQ-027-6). Une valeur hors des valeurs admises est ignorée et lève
  `kube.label.not_admitted`. La valeur ainsi posée est **déclarée**, provenance
  « connecteur, label `<clé>` » ; elle peut ouvrir des flux : le différentiel de
  politique la montre (REQ-010-18).
- Rejeté : sélecteurs de labels bruts dans les règles (Q-019-7 C) ; une permission propre
  `kube.label_mapping.admin` (les paramètres d'un module relèvent de
  `<module>.settings.*`, 027/P4).
- Décidé par : l'utilisateur (2026-11-09, Q-019-7 ; correspondances en paramètres
  génériques du module, sans action propre) ; composition de la valeur : Claude

## X12 — Flux : nouvelles extrémités
- Décision (Q-019-8 = A, D22 amendée) : extrémités ajoutées à la 010 (REQ-010-13) et à la
  011 (REQ-011-2) : workload, namespace (chacun de ses workloads), service (ses
  workloads sélectionnés quand la source est dans le cluster ; ses adresses `vip` quand
  elle est hors du cluster, sans traduction tant que le NAT n'est pas modélisé, v0.4). Un
  workload `hostNetwork` s'évalue sur les adresses de ses nœuds. Réévaluation (REQ-011-11)
  étendue : changement d'axe d'un workload, de namespace, de sélection d'un service,
  passage à l'état absent.
- Décidé par : l'utilisateur (2026-11-09, Q-019-8) ; liste des déclencheurs de
  réévaluation : Claude

## X13 — Interactions avec le cluster
- Décision (Q-019-10 = A) : hors de la 019. La NetworkPolicy est notée en
  référence pour préparer les évolutions : union d'autorisations (pas de refus, donc
  l'interdiction qui l'emporte de D28 doit être **compilée** en autorisations restantes),
  isolation seulement des pods sélectionnés, sélecteurs de pods et de namespaces, blocs
  d'adresses pour l'extérieur, effet subordonné au plugin réseau (réf. § NetworkPolicy).
- Décidé par : l'utilisateur (2026-11-09, Q-019-10)

## X14 — Budgets et volumétrie de référence
- Décision : jeu de référence du noyau (10⁶ objets) augmenté de 10 clusters,
  2·10³ nœuds, 10⁴ namespaces, 10⁵ workloads, 5·10⁴ services, 10⁴ ingress, 3·10⁶ lignes
  d'observation. Budgets : lecture d'un workload avec ses services et sa classification
  < 50 ms ; page filtrée < 150 ms ; « qui avait l'IP X à T » < 50 ms ; verdict workload ×
  workload, 16 × 16, < 300 ms (comme REQ-011-19) ; délai d'un changement du cluster à
  kartenn < 60 s au p95 hors liste initiale ; liste initiale d'un cluster de 10⁴ pods
  < 5 min ; lot du connecteur de 500 opérations < 2 s.
- Décidé par : Claude ; chiffres validés par l'utilisateur le 2026-11-09 (tâche 0b),
  mesurés au banc (tâche 16).

## X15 — Ordre de travail
- Décision : arbitrages et remise en accord, puis domaine pur et module (sans connecteur, testable
  par des changesets écrits à la main), puis connecteur contre un faux serveur d'API,
  puis amendements de la 010 et de la 011, puis interface et bancs. Voir
  [`tasks.md`](tasks.md).
- Décidé par : Claude

## X16 — Permissions, rôles livrés, événements
- Décision (remise en accord avec la 022, la 024, la 025 et la 027) :
  - **Permissions des types** (024/M7, ADR 0026 amendement 5,
    [`docs/spec/permissions.md`](../../docs/spec/permissions.md)) : pour chacun de
    `kube.cluster`, `kube.node`, `kube.namespace`, `kube.workload`, `kube.service`,
    `kube.ingress`, les actions `read`, `edit`, `apply` et `approval.self`
    (`kube.workload.read`, `kube.workload.edit`…), sur le périmètre hérité du cluster
    (ancres `core.owner`, `core.site`, ancrage de domaine, X10). Aucun nom de ressource
    n'est un nom d'action ; aucun ne figure dans `RENAMED_PERMISSIONS`. Un champ suivi
    reste réservé au connecteur par `kube.object.managed`, quelle que soit la permission.
  - **Observations** : `kube.observation.read` et `kube.observation.record` (X7).
  - **Paramètres** : `kube.settings.edit`, `kube.settings.apply`,
    `kube.settings.approval.self`, globales (027/P4, P5) : correspondances label → axe,
    valeurs admises par namespace, liste des labels suivis du module ; ils se lisent
    sous `kube.cluster.read` (le connecteur les lit avec son cluster). La fenêtre de lot
    et les clés suivies propres à un cluster sont des champs du `kube.cluster` (X4, X8).
  - **Rôles livrés** (K12, ADR 0026 clause 8) : le lecteur, l'éditeur et l'approbateur
    reçoivent `kube.<type>.read` des six types ; l'approbateur reçoit en plus
    `kube.<type>.apply`, comme `machines.*.apply` ; `kube.observation.read` est
    **exclu** des trois (la correspondance IP → workload dévoile la topologie
    applicative) et s'accorde par un rôle personnalisé lié sur le périmètre d'un
    cluster ; `*` couvre tout. Le module livre en plus `kube_connector` (X10).
  - **Événements** (022, ADR 0026 clause 10) : aucun type d'événement propre ; les objets
    `kube.*` écrits par un changeset (lot du connecteur ou écriture humaine) figurent
    dans les événements `changes` et se filtrent par `types` (`kube.workload`…), sous
    les mêmes droits que leur lecture ; un lot d'observations n'en produit aucun (X7).
- Pourquoi : la grammaire `read|write|propose` du brouillon a été remplacée par la 024 ;
  les rôles livrés lisent les modules ; un lot de 500 opérations reste un seul événement
  `changes` (REQ-022-5).
- Décidé par : Claude (remise en accord, 2026-11-09) ; l'exclusion de la lecture des
  observations des rôles livrés, le rôle livré par un module et l'audit des lots tenu par
  le noyau : validés par l'utilisateur le 2026-11-09 (tâche 0b)

## Choix structurants
État au 2026-11-09, après les arbitrages et la remise en accord ; tous validés par
l'utilisateur le 2026-11-09 (tâche 0b).
1. **Décisions de cadrage amendées** : D1 (Q-019-2, objets miroirs), D8 et D11
   (Q-019-4, observations hors bitemporel et hors changesets), D22 (Q-019-8, extrémité
   logique), D31 (Q-019-13) ; D21 tenue par Q-019-7 A (valeurs d'axes en liste fermée).
2. **Schéma** : schéma `kube` (six types, table d'observation non versionnée,
   partitionnée) ; colonne d'état `absent_since` ; aucune modification du schéma du
   noyau ; propriété `machines.platform` (009) ; colonne `kube.node.pod_cidrs` (Q-019-14,
   2026-11-10).
3. **Contrat REST** (additif, 006/H12) : collections `/v1/kube-clusters`,
   `/v1/kube-nodes`, `/v1/kube-namespaces`, `/v1/workloads`, `/v1/kube-services`,
   `/v1/ingresses`, identifiées par `{id}`, la clé naturelle en filtre (006/H3) ; route
   d'observations ; extrémités nouvelles des verdicts et des flux ; lignes de la section
   `kube.objects` en JSON explicite par type (Q-019-19, 2026-11-11).
4. **Sécurité** : X10 (droits côté cluster, refus de démarrer, identifiants ;
   confinement côté kartenn par groupe, domaines et `kube.connector.direct`), X5
   (écriture réservée au connecteur), X7 (garde-fous du journal), X11 (labels bornés
   par namespace), X16 (lecture des observations hors des rôles livrés).
5. **Contrat de module** : rôle livré par un module (ADR 0044) ; inscription d'audit
   d'un lot d'observations par le noyau, deux événements d'audit nouveaux (ADR 0043) ;
   identifiants créés rendus au module par `ModuleHandler::planned`, amendement additif de
   la clause 6 de l'ADR 0026 (Q-019-14, 2026-11-10) ;
   référence possédante, amendement additif de l'ADR 0026 (Q-019-16, 2026-11-10).
6. **Dépendance** : client Kubernetes (Q-019-12), fiche à la tâche 9.
7. **ADR** : [0043](../../docs/adr/0043-observations-hors-referentiel.md)
   (observations hors référentiel) et
   [0044](../../docs/adr/0044-connecteur-kubernetes.md) (connecteur Kubernetes),
   statut `proposed`, acceptées à la tâche 17.
