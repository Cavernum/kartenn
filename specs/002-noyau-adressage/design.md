---
status: implemented
updated: 2026-10-17
---

# 002 — Noyau d'adressage : conception

## Vue d'ensemble

Le noyau d'adressage vit dans `kartenn-core` (types et règles purs), est orchestré par
`kartenn-service` (validation dans la transaction du changeset) et persiste via
`kartenn-store-pg` (schéma, contraintes de dernier rempart). Voir
[`docs/spec/architecture.md`](../../docs/spec/architecture.md).

Trois choix structurent cette conception :

1. **Familles d'adresses** : un contrat unique, une représentation interne uniforme
   (`u128` + famille + longueur), l'analyse IPv4/IPv6 déléguée à `std::net`, déjà
   conforme à nos règles strictes (vérifié, voir § Familles).
2. **Règles de validation** : un catalogue de règles identifiées (`core.*`), évaluées par
   la couche service à la date du changeset, doublées de contraintes PostgreSQL pour les
   invariants exposés à la concurrence.
3. **Ancres héritées comme propriétés du noyau** : l'entité propriétaire et le site d'un
   réseau sont des propriétés déclarées par le noyau (`core.owner`, `core.site`), et
   non des colonnes. Elles bénéficient ainsi de la résolution expliquée, du différentiel
   et du droit sur l'effet de la spec 003. Cela tranche la question ouverte de
   REQ-002-38 (ADR 0014).

La 002 dépend de la 004 pour les mécanismes temporels (périodes, versions, temps
système) et de la 003 pour la résolution des propriétés : les tâches les ordonnent.

## Changements

### Familles d'adresses (REQ-002-1 à 7, 31)

**Représentation.** Une valeur d'adresse est `(famille, bits: u128)` ; un préfixe est
`(famille, bits: u128, longueur: u8)`. Les opérations (inclusion, comparaison, arithmétique)
travaillent sur les bits, alignés à gauche sur la largeur de la famille.

**Contrat de famille** (`kartenn-core`, trait `AddressFamily`) :

| Élément | IPv4 | IPv6 |
|---|---|---|
| Identifiant | `ipv4` | `ipv6` |
| Largeur | 32 | 128 |
| Analyse d'adresse | `std::net::Ipv4Addr::from_str` | `std::net::Ipv6Addr::from_str` |
| Forme canonique | `Display` de `Ipv4Addr` | `Display` de `Ipv6Addr` (RFC 5952) |
| Adresses non attribuables d'un réseau | réseau et diffusion si longueur ≤ 30 | IID nul si non point à point |
| Longueurs point à point | 31, 32 | 127, 128 |
| Registre à usage spécial | IANA IPv4 | IANA IPv6 |

Les adresses non attribuables (méthode `reserved_in_network`), les blocs jamais
attribuables (`unassignable`) et le registre (`special_registry`) sont des méthodes du
trait, sans effet par défaut : une famille ajoutée n'en hérite aucun comportement
d'IPv4 ou d'IPv6. Les règles propres à une famille nommée par une RFC (adresse IPv4
mappée dans IPv6, ULA, `core.addr.*` d'IPv6) la désignent par son identifiant ; aucune ne
suppose une largeur. Les `CHECK` SQL de longueur d'hôte supposent 32 et 128 bits : le
stockage d'une autre famille est la question Q-ouv-4.

Le contrat est limité à des largeurs ≤ 128 bits ; une famille de 64 bits (type
draft-thain-ipv8) entre dans ce cadre. Une famille plus large exigerait d'élargir la
représentation : limite documentée, non-objectif.

**Analyse vérifiée.** Essai du 2026-10-14 (rustc 1.98.1) : `Ipv4Addr::from_str` refuse
`010.0.0.1`, `01.2.3.4`, `1.2.3.04`, `10.1`, `0x0a.0.0.1`, `10.0.258`, `256.0.0.1` et une
espace en tête ; `Ipv6Addr::from_str` accepte `2001:DB8::1`, la forme longue, la forme
mixte et `0:0:0:0:0:0:0:1`, refuse `fe80::1%eth0`, `fe80::1%2`, `2001:db8::1::2`,
`12345::1` et `::ffff:010.0.0.1`. `Display` produit `2001:db8::1:0:0:1` et
`2001:db8:0:1:1:1:1:1` (un groupe nul isolé non compressé). Ce comportement est figé par
des tests de non-régression, car il conditionne REQ-002-2 à 4.

**Préfixes.** Analyse maison au-dessus de la famille : `adresse/longueur`, longueur
décimale sans zéro de tête, sans espace, dans `0..=largeur`. Bits d'hôte non nuls → erreur
avec suggestion (préfixe aligné). Pas de longueur implicite.

**Zones.** Le refus de `%` est garanti par `std::net` ; un message dédié est produit en
détectant `%` avant l'analyse (REQ-002-4).

**IPv4-mapped.** L'analyse IPv6 accepte `::ffff:a.b.c.d` (c'est une forme RFC 4291
légitime), puis la règle `core.addr.ipv4_mapped` refuse l'écriture en citant l'adresse
IPv4 équivalente (`Ipv6Addr::to_ipv4_mapped`).

**Famille de test.** Une famille fictive de 64 bits, compilée seulement pour les tests,
passe la batterie de tests de contrat (REQ-002-1).

### Objets du noyau et traçabilité des attributs (REQ-002-48)

| Objet | Attribut | Exigé par |
|---|---|---|
| tous | UUID v7 | REQ-002-39 |
| tous | période de validité, version système | D10 (spec 004) |
| Entité | slug | REQ-002-47 |
| Entité | parent | REQ-002-8 |
| Site | slug, parent | REQ-002-47, REQ-002-9 |
| Domaine | slug | REQ-002-11, REQ-002-47 |
| Préfixe | domaine | REQ-002-12 |
| Préfixe | valeur (famille, bits, longueur) | REQ-002-5, 14, 15 |
| Préfixe | nature (conteneur, réseau) | REQ-002-13, 16 |
| Préfixe | point à point | REQ-002-19, 23, 29 |
| Réseau | anycast | REQ-002-46, 49 |
| Plage | domaine, début, fin | REQ-002-21, 22 |
| Plage | usage | REQ-002-24, 28 |
| Adresse | domaine, valeur | REQ-002-25, 26 |
| Adresse | usage | REQ-002-27, 28, 46 |
| Domaine, préfixe | `core.owner` (propriété du noyau) | REQ-002-11, 38 |
| Réseau | `core.site` (propriété du noyau) | REQ-002-20 |

### Ancres comme propriétés du noyau (REQ-002-11, 20, 38) — ADR 0014

Le noyau déclare deux propriétés (spec 003) :

| Clé | Type | Porteurs | Héritage | Obligation | Permission |
|---|---|---|---|---|---|
| `core.owner` | référence → entité | domaine, préfixe | oui, arbre d'adressage | domaines (tous) | `prop.core.owner.write` |
| `core.site` | référence → site | préfixe | non | — | `prop.core.site.write` |

- REQ-002-11 (domaine avec propriétaire) devient l'obligation de `core.owner` sur les
  domaines ; REQ-002-38 est la résolution expliquée de `core.owner` (REQ-003-14) ;
  plages et adresses héritent via leur réseau (REQ-003-11).
- REQ-002-20 : la règle `core.prefix.site_on_container` refuse `core.site` sur un
  conteneur, y compris lors d'un changement de nature.
- **Gain de sécurité** : insérer un conteneur ou clore un préfixe qui changerait le
  propriétaire résolu d'objets existants exige `prop.core.owner.write` sur ces objets
  (REQ-003-18). Un prestataire ne peut pas s'approprier un sous-arbre par une
  manœuvre de structure.
- **Performance** : le RBAC (005) lit le propriétaire résolu à chaque requête ; il
  s'appuie sur la résolution matérialisée de la 003.
- **Effet sur la spec système** : `data.md` décrit ces deux ancres comme attributs du
  préfixe ; à corriger lors de la synchronisation (mode *sync*).

**Mise en œuvre (002-22)** :
- Opérations `CreateEntity` et `CreateSite` (slug, parent facultatif), `MoveEntity` et
  `MoveSite` (nouveau parent, ou racine, sur la période du changeset, par le découpage
  de la 004), clôture par `Close`. Sur l'état final, toute version sur la période :
  parent existant sur toute la période de l'enfant (sinon
  `changeset.operation.no_version`), pas de cycle (`core.anchor.cycle`). **Limite** : la
  recherche de cycle suit tous les liens parent qui recoupent la période, quelle que soit
  leur date ; elle peut refuser un déplacement qui ne forme de cycle à aucun instant (des
  liens A → B et B → A à des dates disjointes). Prudente, jamais laxiste ; à affiner si
  le cas se présente. Les deux arbres sont sérialisés par un verrou chacun (ADR 0015). Clôture d'une
  entité ou d'un site : refusée tant que des enfants ou des affectations s'y réfèrent
  après la date (`core.anchor.in_use`, objets listés).
- `core.owner` et `core.site` sont les déclarations du module `core`, synchronisées
  depuis le code comme celles d'un module (003/P12) : sans synchronisation, aucune
  obligation de propriétaire ne s'applique (bases de test).
- `core.prefix.site_on_container` : vérifiée sur chaque version finale d'un préfixe qui
  reçoit `core.site` ou change de nature.
- REQ-002-12 : les opérations de modification d'un préfixe, d'une plage ou d'une adresse
  acceptent un champ `domain` pour le refuser explicitement (`core.domain.immutable`) ;
  le déplacement se fait par clôture puis création.
- L'héritage le long des arbres d'entités et de sites est maintenu depuis la tâche
  003-18 (003/P16).
- **Renommage** (REQ-002-47) : l'opération `Rename` change le slug d'un domaine, d'une
  entité ou d'un site sur la période du changeset, l'UUID restant ; le slug est unique
  par type sur toute période (contrainte d'exclusion, `core.slug.duplicate`).

### Catalogue des règles (REQ-002-35 à 37)

Chaque règle a un identifiant stable, une sévérité, une source et les exigences qu'elle
sert. Le catalogue est généré depuis le code et publié (documentation et API).

| Identifiant | Sévérité | Exigence | Source |
|---|---|---|---|
| `core.addr.syntax` | erreur | REQ-002-2, 3 | RFC 6943 §3.1.1, RFC 4291 §2.2 |
| `core.addr.zone` | erreur | REQ-002-4 | RFC 4007 §6 |
| `core.addr.ipv4_mapped` | erreur | REQ-002-31 | 002/A4 |
| `core.prefix.syntax` | erreur | REQ-002-5 | RFC 4632 §3.1 |
| `core.prefix.unaligned` | erreur | REQ-002-5 | 002/design (convention, cidr PostgreSQL) |
| `core.prefix.duplicate` | erreur | REQ-002-15 | D34 |
| `core.prefix.network_not_leaf` | erreur | REQ-002-16, 17 | 002/A2 |
| `core.prefix.not_empty_for_nature` | erreur | REQ-002-17 | 002/A2 |
| `core.prefix.p2p_length` | erreur | REQ-002-19 | RFC 3021, RFC 6164 |
| `core.prefix.site_on_container` | erreur | REQ-002-20 | 002/A7 |
| `core.prefix.anycast_network` | erreur | REQ-002-49 | 002/A13 |
| `core.anycast.address_usage` | erreur | REQ-002-49 | 002/A13 |
| `core.range.bounds` | erreur | REQ-002-21 | 002/design |
| `core.range.outside_network` | erreur | REQ-002-21 | 002/A2 |
| `core.range.overlap` | erreur | REQ-002-22 | D34 |
| `core.range.reserved_bounds` | erreur | REQ-002-23 | RFC 1122 §3.2.1.3, RFC 4291 §2.6.1 |
| `core.address.outside_network` | erreur | REQ-002-25 | 002/A2 |
| `core.address.duplicate` | erreur | REQ-002-26 | D4 |
| `core.address.range_usage` | erreur | REQ-002-28 | 002/A8 |
| `core.address.not_assignable` | erreur | REQ-002-29 | RFC 1122, RFC 4291, registres IANA |
| `core.public.overlap` | erreur | REQ-002-32, 46 | 002/A3, A11, A13 |
| `core.domain.immutable` | erreur | REQ-002-12 | 002/A1 |
| `core.anchor.cycle` | erreur | REQ-002-8, 9 | 002/design |
| `core.anchor.in_use` | erreur | REQ-002-10 | D33 |
| `core.slug.syntax` | erreur | REQ-002-47 | 002/A9 |
| `core.slug.duplicate` | erreur | REQ-002-47 | 002/A9 |
| `core.external_id.duplicate` | erreur | REQ-002-41 | 002/A9 |
| `core.addr.documentation` | avertissement | REQ-002-35 | RFC 5737, 3849, 9637 |
| `core.addr.shared_space` | avertissement | REQ-002-35 | RFC 6598 |
| `core.prefix.ipv6_long_subnet` | avertissement | REQ-002-35 | RFC 7136 §5 |
| `core.prefix.ula_global_id` | avertissement | REQ-002-35 | RFC 4193 §3.2 |
| `core.public.anycast_shared` | avertissement | REQ-002-46 | 002/A11, A13 |

**Heuristique ULA** (`core.prefix.ula_global_id`, question ouverte de la 002) : le Global
ID (40 bits) est jugé non aléatoire s'il est nul, s'il vaut au plus `0xFFFF` (numérotation
séquentielle), si ses 5 octets sont identiques, ou s'ils forment une suite croissante ou
décroissante de pas 1. Ce n'est qu'un avertissement : les faux négatifs sont acceptés.

### Exécution de la validation (REQ-002-14 à 32)

La validation est appelée par le pipeline d'écriture (architecture, § Pipeline) :

1. **Règles locales**, sur chaque objet seul : syntaxe, alignement, bornes, slug.
2. **Règles globales**, dans la transaction, à la période visée, par les lectures du port
   d'écriture (`WriteTx` : réseaux englobants, préfixes contenus, couverture,
   chevauchements, publics d'autres domaines ; implementation.md, I4).
   Elles voient l'état **après** application de toutes les opérations du changeset, ce
   qui permet de créer en une fois un conteneur, un réseau et ses adresses.
3. **Résolution des propriétés et différentiel** (003), dont `core.owner` et `core.site`.

Toutes les violations d'erreur sont collectées (pas d'arrêt à la première) et renvoyées
ensemble ; les avertissements produisent des anomalies.

**Parent déduit** (REQ-002-14) : plus petit préfixe du domaine qui contient strictement
l'objet à la date : `prefix >> $valeur` (ou `>>=` pour une plage ou une adresse), tri par
longueur décroissante, premier résultat. Servi par l'index GiST `inet_ops`.

**Plages** (REQ-002-21 à 24, 28) : une plage est couverte, sur toute sa période, par les
réseaux de son domaine qui contiennent ses deux bornes ; ses bornes évitent les positions
réservées de ce réseau ; les adresses qu'elle couvre respectent son usage, que la plage ou
l'adresse soit créée en premier. Clore un réseau revérifie ses plages comme ses adresses.
Une plage n'a pas de contrôle d'espace public propre : son réseau est déjà unique entre
domaines.

**Espace public** (REQ-002-32, 46) : chaque préfixe, plage et adresse reçoit à l'écriture
un indicateur `public` calculé depuis le registre à usage spécial (entrée la plus
spécifique, Globally Reachable ou absence d'entrée), avec la version du registre utilisée.
La règle `core.public.overlap` cherche les objets publics d'autres domaines qui
chevauchent la valeur ; un chevauchement est toléré si et seulement si les deux objets
sont anycast (adresse d'usage « anycast » ou réseau marqué anycast, A11, A13). Ainsi une
adresse anycast de A ne se heurte pas au réseau anycast de B qui la contient. Entre
réseaux, la tolérance exige en plus la même valeur : ce sont deux occurrences du même
réseau. Les conteneurs n'entrent pas dans l'unicité (A3) : seuls les réseaux et les
adresses sont comparés, les plages l'étant à travers leur réseau. La règle
`core.public.anycast_shared` avertit pour chaque chevauchement toléré ; la règle
`core.anycast.address_usage` refuse une adresse non anycast dans un réseau anycast
(REQ-002-49).

### Concurrence (INV-2, INV-5, INV-6) — ADR 0015

Les règles globales lisent l'état pour décider : deux changesets concurrents pourraient
chacun passer la validation et produire ensemble un doublon. Dispositif :

- **verrou consultatif de transaction par domaine** (`pg_advisory_xact_lock`), pris dans
  un ordre déterministe pour tous les domaines touchés par le changeset ;
- **verrou « espace public »** supplémentaire pour tout changeset qui écrit un objet
  public ;
- **contraintes PostgreSQL** comme dernier rempart (§ Stockage).

À 50 écritures/s soutenues (D14), la sérialisation par domaine est acceptable ; les
lectures ne prennent aucun verrou.

### Stockage PostgreSQL (REQ-002-15, 22, 26, 32, 34, 39 à 41)

Schéma conceptuel. Les colonnes temporelles (`valid_period`, `system_period`) et le
découpage courant/historique relèvent de la 004.

| Table | Colonnes principales | Contraintes |
|---|---|---|
| `entity` | `id uuid`, `slug`, `parent_id` | slug unique par période |
| `site` | `id`, `slug`, `parent_id` | idem |
| `addressing_domain` | `id`, `slug` | idem |
| `prefix` | `id`, `domain_id`, `prefix cidr`, `nature`, `point_to_point`, `public`, `registry_version` | `EXCLUDE USING GIST (domain_id WITH =, prefix inet_ops WITH =, valid_period WITH &&)` ; `CHECK (NOT point_to_point OR nature = 'network')` |
| `ip_range` | `id`, `domain_id`, `range inetrange`, `usage`, `public`, `registry_version` | `EXCLUDE USING GIST (domain_id WITH =, range WITH &&, valid_period WITH &&)` |
| `address` | `id`, `domain_id`, `address inet`, `usage`, `public`, `registry_version` | `EXCLUDE USING GIST (domain_id WITH =, address inet_ops WITH =, valid_period WITH &&)` ; exclusion partielle publique non anycast : `EXCLUDE USING GIST (address inet_ops WITH =, valid_period WITH &&) WHERE (public AND usage <> 'anycast')` |
| `external_id` | `source`, `external_id`, `object_id`, `object_type` | unique `(source, external_id)` |
| `anomaly` | `id`, `rule_id`, `object_id`, `message`, `opened_at`, `opened_by` (changeset), `closed_at`, `acknowledged_by`, `acknowledged_at`, `motive`, `acked_changeset` (dernier changeset de l'objet à l'acquittement) | une anomalie ouverte par (règle, objet) |
| `special_registry` | `registry_version`, `block`, `name`, `rfc`, `terminated`, `globally_reachable` | données de référence, chargées par le job de migration ; seul le drapeau Globally Reachable sert en SQL (`kartenn_is_public`), les autres restent dans le registre du code |

- `inetrange` : type intervalle sur `inet` **sans** `subtype_diff`, avec
  `CHECK (family(lower(range)) = family(upper(range)))` — amendement issu du
  [prototype 001](../../docs/spikes/001-gist-exclusions.md), validé (A12).
- `prefix.anycast boolean NOT NULL DEFAULT false` avec
  `CHECK (NOT anycast OR nature = 'network')` (REQ-002-49, A13).
- `domain_id WITH =` utilise btree_gist sur `uuid` (fiable) ; jamais de btree_gist sur
  `inet`/`cidr` (ADR 0004).
- Les contraintes s'appliquent aux versions courantes côté système (dispositif exact :
  spec 004, partiel `WHERE` ou table courante).
- Le chevauchement public entre **réseaux** de domaines différents reste vérifié par
  l'application seule (une exclusion `&&` partielle sur les réseaux publics est possible
  et sera ajoutée si le prototype le confirme).
- Valeurs transmises à PostgreSQL : toujours des paramètres typés issus de la forme
  canonique, jamais le texte saisi (formes abrégées acceptées par `cidr`).

### Registres à usage spécial (REQ-002-33, 34)

- Source : fichiers CSV publiés par l'IANA, versionnés dans le dépôt
  (`crates/kartenn-core/data/iana/`), avec leur date « Last Updated ».
- Compilés dans le binaire (structure de recherche par préfixe le plus long) et chargés
  dans `special_registry` par le job de migration (`maintenance::deploy`), pour les
  requêtes SQL.
- Mettre à jour = remplacer les fichiers et publier une version ; `registry::public_drift`
  liste les préfixes, plages et adresses dont l'indicateur `public` change. La
  réécriture de l'indicateur par un changeset système n'est pas faite : elle viendra
  avec l'ordonnanceur des jobs (006).

### Anomalies (REQ-002-35, 36)

- Une anomalie est (règle, objet, message, changeset d'ouverture). Elle est créée ou
  mise à jour à chaque changeset qui réévalue la règle sur l'objet, et close quand elle
  ne se déclenche plus. Acquittée, elle reste masquée jusqu'au changeset suivant de
  l'objet. Liste filtrable par règle, domaine (l'objet ou ce que contient le domaine) et
  entité (l'entité, ses descendantes et les objets dont le `core.owner` résolu en est une).
- Seul un changeset qui **réévalue** une règle sur un objet peut clore son anomalie : les
  règles du noyau sur les objets qu'il crée (et modifie, 004-8), toutes les règles sur
  ceux qu'il clôt, l'écart de valeur attendue sur les objets contrôlés (003). Affecter une
  propriété à un objet ne clôt donc pas son anomalie « bloc de documentation ».
- Acquitter : motif, auteur, date, version de l'objet acquittée. Une modification
  ultérieure de l'objet rouvre l'anomalie si la règle se déclenche encore.

### Identifiants (REQ-002-39 à 41, 47)

- UUID v7 générés par l'application (crate `uuid`, fonction v7), pour pouvoir les
  renvoyer et les référencer à l'intérieur d'un même changeset.
- Slug : `^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$`, unique par type à tout instant ;
  modifiable (nouvelle version du même UUID).
- Résolution de clé naturelle → UUID dans le port de lecture, à la date demandée.

### Lecture : arbre, occupation, prochain libre (REQ-002-42 à 44)

- **Enfants directs** d'un préfixe P : objets du domaine contenus dans P dont le parent
  déduit est P (préfixes contenus, puis exclusion de ceux qui ont un ancêtre
  intermédiaire contenu dans P).
- **Occupation d'un réseau** : capacité selon la famille, la longueur et le point à point
  (REQ-002-43) ; adresses déclarées + taille des plages DHCP et réservées ; calcul en
  `u128` (un /64 IPv6 dépasse `u64`).
- **Occupation d'un conteneur** : somme des tailles des enfants directs / taille du
  conteneur (pas de double comptage grâce à l'arbre).
- **Prochain préfixe libre** de longueur L dans un conteneur : parcours des enfants
  triés par adresse, premier trou aligné sur L (alignement : multiple de 2^(largeur−L)).
- **Prochaine adresse libre** : premier entier attribuable du réseau qui n'est ni déclaré,
  ni dans une plage DHCP ou réservée, ni non attribuable (REQ-002-29).

### Performance (REQ-002-45)

- Index : ceux des contraintes d'exclusion GiST (`domain_id`, valeur `inet_ops`,
  `valid_period`) sur `prefix`, `address`, `ip_range`, et `slug` sur les ancres ; clés
  primaires `(id, valid_period)`. Aucun index autonome supplémentaire : les budgets
  tiennent sans (A14).
- Toutes les requêtes ci-dessus sont des recherches indexées ou des parcours bornés par
  un préfixe ; aucun parcours complet de table.
- Jeu synthétique (crate de bench) : 10⁵ réseaux, 10⁶ adresses, profondeur ≥ 6, IPv4 et
  IPv6, 20 domaines dont 5 réutilisent `10.0.0.0/8`, une part d'espace public.
- La mesure combinée avec la résolution matérialisée de la 003 conditionne le
  passage en `approved` de la conception de la 003.
- Banc `kartenn-bench core` (script `bench-core`, A14) : budgets de REQ-002-45 et de
  REQ-004-38 sur ce jeu avec trois ans d'historique, bloquants par le code de sortie. La
  lecture d'un objet mesurée est `kartenn_store_pg::read::object` (attributs et valeurs
  résolues). Résultats et marges dans A14 ; la page filtrée (102 ms pour 150) est la
  plus proche de son seuil.

## Erreurs et cas limites

| Cas | Comportement | REQ |
|---|---|---|
| `010.0.0.1`, `10.1`, `1.2.3.4 ` | `core.addr.syntax`, forme attendue citée | 2 |
| `fe80::1%eth0` | `core.addr.zone` | 4 |
| `::ffff:192.0.2.1` | `core.addr.ipv4_mapped`, suggestion `192.0.2.1` | 31 |
| `192.168.1.5/24` | `core.prefix.unaligned`, suggestion `192.168.1.0/24` | 5 |
| `10.0.0.0/08`, `10.0.0.0/33` | `core.prefix.syntax` | 5 |
| Préfixe `/0` | Accepté comme conteneur (route par défaut d'un domaine) | 5, 13 |
| Réseau `/32` ou `/128` non point à point | Accepté ; capacité 1 ; l'adresse unique est attribuable | 19, 43 |
| Conteneur, réseau et adresses créés dans le même changeset | Accepté : les règles globales voient l'état final | 14, 25 |
| Clore un réseau qui contient des adresses | Les adresses n'ont plus de réseau : `core.address.outside_network` les liste (D33) ; cascade explicite possible (004) | 25 |
| Redimensionner un réseau vers une taille qui capte un réseau voisin | `core.prefix.network_not_leaf` | 16, 18 |
| Deux changesets concurrents créent le même préfixe | Le second attend le verrou du domaine puis échoue en `core.prefix.duplicate` ; à défaut, l'exclusion PostgreSQL rejette | 15 |
| Mise à jour du registre IANA qui rend un bloc public | Migration : recalcul de `public`, rapport des objets dont l'unicité devient globale et des conflits éventuels (anomalies) | 32, 34 |
| Occupation d'un IPv6 /0 | Calcul en `u128`, pas de dépassement (taille 2¹²⁸ représentée à part) | 43 |

## Alternatives considérées

- **Crate tierce d'analyse IP (ipnet, ipnetwork, cidr)** : rejetée pour l'analyse, car
  `std::net` est déjà stricte comme exigé (vérifié) ; une crate pourra servir pour des
  utilitaires si un besoin apparaît, en revue de dépendances.
- **Représentation par famille (`Ipv4Addr`/`Ipv6Addr` en enum fermé)** : rejetée, car
  ajouter une famille modifierait le cœur ; `u128` + famille garde le contrat ouvert.
- **Propriétaire et site en colonnes** : rejeté (ADR 0014) : pas de résolution expliquée
  partagée, pas de droit sur l'effet en cas de manœuvre de structure.
- **Isolation SERIALIZABLE** au lieu de verrous par domaine : rejetée pour l'instant :
  erreurs de sérialisation à rejouer côté client et coûts difficiles à prévoir sur des
  prédicats d'inclusion ; à réévaluer si les verrous deviennent un goulet.
- **`UNIQUE … WITHOUT OVERLAPS` sur `prefix`** : rejeté en PostgreSQL 18 (btree_gist
  défectueux sur `cidr`, ADR 0004).

## Risques et migration

- **Couplage avec 003 et 004** : la 002 ne peut être livrée seule ; les tâches
  imbriquent les trois specs. Risque de dérive : chaque tâche cite ses REQ.
- **Type `inetrange`** : la combinaison de btree_gist (uuid), d'un intervalle d'adresses
  et d'un intervalle de temps dans un même GiST est **prouvée** par le prototype 001 ; le
  repli (vérification applicative seule, sous verrou de domaine) n'est plus nécessaire.
- **Registre IANA** : une mise à jour peut faire basculer des objets en « public » et
  créer des conflits a posteriori ; traité par migration et anomalies.
- **Comportement de `std::net`** : une évolution future de la bibliothèque standard
  changerait nos règles ; figé par des tests de non-régression.
- **Pas de données existantes** : aucune migration de données en v0.1.

## Stratégie de test

| Exigences | Type de test |
|---|---|
| 1 à 7, 31 | Tests unitaires de familles ; tests de propriétés (aller-retour analyse ↔ canonique, ordre, inclusion) ; batterie de contrat commune aux familles, dont la famille fictive de 64 bits ; tests de non-régression `std::net` |
| 8 à 29, 32, 46, 47 | Tests unitaires par règle (un cas valide, un cas refusé par identifiant) ; tests d'intégration PostgreSQL (changesets réels) |
| 15, 22, 26, 32 | Tests de concurrence : deux transactions simultanées, une seule réussit |
| 33, 34 | Tests sur les registres embarqués (entrée la plus spécifique : `192.0.0.9`) ; test de mise à jour du registre par migration |
| 35, 36 | Tests d'intégration des anomalies : création, acquittement, réouverture |
| 38, 20 | Tests de résolution de `core.owner` et `core.site` (avec la 003), dont le refus d'une manœuvre de structure sans droit |
| 39 à 41 | Tests d'unicité et de résolution de clé naturelle |
| 42 à 44 | Tests de propriétés : occupation ≤ capacité ; le prochain libre est valide et minimal |
| 45 | Banc d'essai CI sur le jeu synthétique ; seuils D14 bloquants |
| 48 | Revue : la table de traçabilité ci-dessus est à jour |

## ADR

- [`docs/adr/0014-ancres-proprietes-du-noyau.md`](../../docs/adr/0014-ancres-proprietes-du-noyau.md)
- [`docs/adr/0015-serialisation-ecritures-par-domaine.md`](../../docs/adr/0015-serialisation-ecritures-par-domaine.md)
