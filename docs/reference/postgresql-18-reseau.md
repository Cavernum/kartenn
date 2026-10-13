# PostgreSQL 18 : types réseau, index et contraintes d'exclusion

## Sources (documentation PostgreSQL 18, consultée le 2026-10-13)

- [§8.9 Network Address Types](https://www.postgresql.org/docs/18/datatype-net-types.html)
- [§9.12 Network Address Functions and Operators](https://www.postgresql.org/docs/18/functions-net.html)
- [§65.2 GiST, opérateurs intégrés](https://www.postgresql.org/docs/18/gist.html)
- [§65.3 SP-GiST, opérateurs intégrés](https://www.postgresql.org/docs/18/spgist.html)
- [F.8 btree_gist](https://www.postgresql.org/docs/18/btree-gist.html)
- [CREATE TABLE](https://www.postgresql.org/docs/18/sql-createtable.html) (EXCLUDE, WITHOUT OVERLAPS, PERIOD)
- [§8.17 Range Types](https://www.postgresql.org/docs/18/rangetypes.html)
- [Notes de version 18](https://www.postgresql.org/docs/18/release-18.html) (18.0 publiée le 2025-09-25)

## Ce que dit la documentation

### Types `inet` et `cidr` (§8.9)
- 7 octets (IPv4) ou 19 octets (IPv6) ; `macaddr` 6 octets, `macaddr8` 8 octets.
- « `inet` accepts values with nonzero bits to the right of the netmask, whereas `cidr`
  does not » : `192.168.0.1/24` est valide en `inet`, pas en `cidr`.
- `cidr` : « It is an error to specify a network address that has bits set to the right
  of the specified netmask. » Si la longueur est omise, elle est **déduite des classes
  historiques** : `10` → `10.0.0.0/8`, `128.1` → `128.1.0.0/16`, `192.168` →
  `192.168.0.0/24`.
- Tri : « IPv4 addresses will always sort before IPv6 addresses, including IPv4
  addresses encapsulated or mapped to IPv6 addresses ».
- Affichage `inet` : la longueur est omise pour un hôte unique.

### Opérateurs et fonctions (§9.12)

| Opérateur | Sens |
|---|---|
| `<<`, `<<=` | strictement contenu dans, contenu dans ou égal |
| `>>`, `>>=` | contient strictement, contient ou égal |
| `&&` | l'un contient ou est égal à l'autre |
| `~`, `&`, `\|` | NON, ET, OU bit à bit |
| `inet + bigint`, `inet - bigint`, `inet - inet` | arithmétique d'adresses |

Fonctions utiles : `family`, `masklen`, `network`, `broadcast`, `hostmask`, `netmask`,
`set_masklen`, `inet_merge` (plus petit réseau contenant les deux),
`inet_same_family`, `host`, `text`, `abbrev`.

### Index
- **GiST `inet_ops`** gère `<< <<= >> >>= = <> < <= > >= &&`. « For historical reasons,
  the `inet_ops` operator class is not the default class for types `inet` and `cidr`. »
  Il faut la nommer : `CREATE INDEX … USING GIST (col inet_ops)`.
- **SP-GiST `inet_ops`** gère les mêmes opérateurs.
- **btree_gist** fournit des classes GiST pour des scalaires, dont `inet`, `cidr`, `uuid`,
  `bool` et les `enum`, avec un comportement équivalent au B-tree (égalité et ordre), pas
  l'inclusion réseau. Extension « trusted ».
- ⚠ **Les classes `gist_inet_ops` et `gist_cidr_ops` de btree_gist sont défectueuses.**
  La documentation de PostgreSQL 19
  ([F.8 btree_gist](https://www.postgresql.org/docs/19/btree-gist.html)) indique :
  « index searches may fail to find relevant rows due to approximations used in creating
  the index entries. This is unfixable ». Le défaut existe donc aussi en 18. Une
  contrainte d'exclusion appuyée sur ces classes peut **laisser passer un conflit**.
  En 18, si btree_gist est installé, ce sont elles qui servent par défaut pour `inet`
  et `cidr` en GiST.

### Contraintes d'exclusion (CREATE TABLE)
- La méthode d'accès doit gérer `amgettuple` (pas GIN) ; les opérateurs doivent être
  commutatifs ; « in practice the access method will always be GiST or SP-GiST ».
- Une contrainte d'exclusion accepte une clause `WHERE` (contrainte partielle).
- Exemple de la documentation (§8.17.10), combinant égalité et chevauchement :
  `EXCLUDE USING GIST (room WITH =, during WITH &&)` avec btree_gist.

### Contraintes temporelles (nouveauté de PostgreSQL 18)
- `PRIMARY KEY` / `UNIQUE (…, col WITHOUT OVERLAPS)` : la dernière colonne est un
  intervalle (range ou multirange), **les intervalles vides sont interdits**. Équivaut à
  `EXCLUDE USING GIST (… WITH =, col WITH &&)` ; les colonnes scalaires exigent
  btree_gist, « the expected way to use this feature ».
- `FOREIGN KEY (…, PERIOD col)` : la référence doit être couverte sur **toute** sa
  période par la réunion des périodes référencées ; la cible doit avoir une clé
  `WITHOUT OVERLAPS`. Actions `CASCADE`, `SET NULL`, `SET DEFAULT` et `RESTRICT` :
  « not supported » ; seul `NO ACTION` (le défaut) est possible.

### Types intervalle (§8.17)
- On peut créer un type intervalle sur tout type totalement ordonné :
  `CREATE TYPE … AS RANGE (subtype = …, subtype_diff = …)`. `subtype_diff` est optionnel
  mais l'index GiST est « considerably less efficient » sans lui.
- Convention `[)` par défaut ; bornes infinies ; valeur `empty`.

## Pièges

- `&&` sur `inet` signifie « inclusion dans un sens ou dans l'autre ». Pour deux blocs
  CIDR, c'est exactement le chevauchement (ils sont soit imbriqués, soit disjoints),
  mais ce n'est **pas** un test de chevauchement de plages arbitraires.
- Un index GiST sur `inet` créé sans `inet_ops` prend en 18 la classe par défaut, celle
  de btree_gist si l'extension est installée : elle n'accélère pas l'inclusion et elle est
  **défectueuse** (voir ci-dessus). Toujours nommer `inet_ops` explicitement.
- `UNIQUE (…, prefix, période WITHOUT OVERLAPS)` sur une colonne `cidr` utiliserait cette
  classe défectueuse, et la syntaxe `UNIQUE` ne permet pas de choisir la classe : il faut
  écrire la contrainte sous forme `EXCLUDE` avec `prefix inet_ops WITH =`.
- pg_upgrade vers la 19 **refuse** une base contenant des index `gist_inet_ops` ou
  `gist_cidr_ops` : ne jamais en créer.
- Saisie abrégée « classful » en `cidr` : `10` devient `10.0.0.0/8`. Toujours envoyer
  une valeur déjà analysée et canonique (paramètre typé), jamais le texte de
  l'utilisateur.
- Les clés étrangères temporelles n'offrent que `NO ACTION` : les cascades doivent être
  faites par l'application.

## Implications pour kartenn (propositions pour la spec 002)

1. **Stockage** : `cidr` pour les préfixes, `inet` (longueur /32 ou /128) ou `cidr` hôte
   pour les adresses. Le choix précis va dans un ADR de l'adaptateur PostgreSQL.
2. **Unicité temporelle des préfixes** (D10, D34) :
   `EXCLUDE USING GIST (domain_id WITH =, prefix inet_ops WITH =, valid_period WITH &&)`
   (btree_gist seulement pour `domain_id`, de type `uuid`). Garantit « pas de doublon dans
   un domaine sur des périodes qui se recouvrent ». La forme `UNIQUE … WITHOUT OVERLAPS`
   est à proscrire sur la colonne `cidr` en 18 (classe défectueuse). Les clés `WITHOUT
   OVERLAPS` sur l'identifiant (`id`, `uuid`) restent possibles et nécessaires pour les
   clés étrangères `PERIOD`. À combiner avec
   le temps système (voir [temporel-sql2011-postgresql.md](temporel-sql2011-postgresql.md)) :
   la contrainte ne doit porter que sur les lignes **courantes** côté système, ce qui
   oriente vers un `EXCLUDE … WHERE (…)` partiel ou une table d'historique séparée.
3. **Plages non CIDR** (D34, pas de chevauchement partiel) : type intervalle personnalisé
   sur `inet` (avec `subtype_diff`) et `EXCLUDE USING GIST (domain_id WITH =, plage WITH &&,
   valid_period WITH &&)`. **À prototyper** : vérifier qu'un même index GiST combine
   btree_gist (pour `uuid` uniquement), l'intervalle d'adresses et l'intervalle de temps.
8. **Garde-fou** : un test de migration vérifie qu'aucun index n'utilise
   `gist_inet_ops`/`gist_cidr_ops` (requête sur `pg_index` et `pg_opclass`).

## PostgreSQL 19 (état au 2026-10-13)

En **bêta** (Beta 4 publiée le 2026-09-24) ; date de sortie non annoncée, notes de
version datées « 2026-??-?? ». Sources :
[notes de version 19](https://www.postgresql.org/docs/19/release-19.html),
[btree_gist 19](https://www.postgresql.org/docs/19/btree-gist.html),
[CREATE TABLE 19](https://www.postgresql.org/docs/19/sql-createtable.html),
[UPDATE 19](https://www.postgresql.org/docs/19/sql-update.html).

| Sujet | Apport de la 19 | Intérêt pour kartenn |
|---|---|---|
| `inet_ops` devient la classe GiST par défaut pour `inet`/`cidr` | corrige le piège ci-dessus ; `UNIQUE … WITHOUT OVERLAPS` sur `cidr` devient sûr | fort, mais contournable en 18 avec `EXCLUDE` et `inet_ops` explicite |
| `range_minus_multi()`, `multirange_minus_multi()` | soustraction d'intervalles avec trous | utile pour calculer le découpage « FOR PORTION OF » dans la couche service |
| Section de documentation sur les tables temporelles | documentation | faible |
| Casts `bytea` ↔ `uuid` | — | faible |
| `UPDATE/DELETE … FOR PORTION OF` | **absent** : ajouté pendant le cycle, puis **retiré** le 2026-09-15 ([commit de revert](https://www.postgresql.org/message-id/E1x6QRv-00000000QOT-0jWk@gemulon.postgresql.org)) ; absent de la synopsis d'UPDATE de la doc 19 | aucun en 19 ; à surveiller pour la 20 |
| Tables versionnées système | absent | aucun |
| Clés étrangères temporelles : `CASCADE`, `SET NULL`… | toujours « not supported » | aucun |

Conclusion : la 19 ne change pas l'architecture (le temps système et le découpage
restent à la charge de kartenn) ; elle supprime un piège que l'on sait contourner en 18.
4. **Requêtes d'arbre** (D34, plus petit préfixe englobant) : index GiST `inet_ops` ou
   SP-GiST `inet_ops` sur les préfixes ; à mesurer sur 10⁶ objets (budgets D14).
5. **Suppression** (D33) : cohérent avec `NO ACTION` ; la cascade explicite du changeset
   est calculée par l'application.
6. **Identifiants** : `uuidv7()` (PostgreSQL 18) donne des UUID ordonnés dans le temps,
   favorables aux index B-tree. Option à évaluer pour l'identité des objets.
7. **IPv8 et familles futures** (D5) : hors de `inet`/`cidr` ; l'adaptateur devra alors
   utiliser une autre représentation (entier ou `bytea` de largeur fixe, avec un type
   intervalle personnalisé). C'est le point d'extension à décrire, pas à construire.
