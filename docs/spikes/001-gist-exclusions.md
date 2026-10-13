# Prototype 001 — Contraintes d'exclusion GiST

- Tâche : 002-6. Exigences : REQ-002-15, 22, 26, 32, 45. Date : 2026-10-14.
- Environnement : PostgreSQL 18.6 (devenv), poste de développement, jeu de référence
  `kartenn-bench gen 42` : 20 domaines, 192 219 préfixes, 9 965 plages, 996 717 adresses.
- Scripts : [`001-gist-exclusions/`](001-gist-exclusions/), rejouables dans l'ordre.
- Les mesures sont indicatives (une machine de développement, pas de répétitions) ; les
  bancs de la P6 font foi.

## Résultats

### (a) Unicité temporelle des préfixes — **confirmé**

`EXCLUDE USING GIST (domain_id WITH =, prefix inet_ops WITH =, valid_period WITH &&)` :

- un seul index GiST combine `gist_uuid_ops` (btree_gist), `inet_ops` et `range_ops`
  (vérifié dans `pg_index`) ;
- comportement : même préfixe, même domaine, périodes qui se recouvrent → refus ; autre
  domaine, période disjointe, préfixe imbriqué → acceptés ;
- chargement de 192 219 préfixes sous contrainte : 3,8 s ; index 20 Mo (table 19 Mo).

### (e) Recherche du parent déduit — **confirmé**

`domain_id = $d AND prefix >>= $adresse AND valid_period @> now() ORDER BY masklen DESC
LIMIT 1`, servie par le même index : **2 000 recherches en 70 ms, soit 35 µs chacune**
(7,4 candidats examinés en moyenne). Largement sous les budgets D14.

### (b) Plages sans chevauchement — **confirmé, avec un amendement**

Type intervalle sur `inet` et `EXCLUDE USING GIST (domain_id WITH =, r WITH &&,
valid_period WITH &&)`. Quatre variantes comparées :

| Variante | `subtype_diff` | Chargement de 9 965 plages | 2 000 sondes `@>` (LATERAL) |
|---|---|---|---|
| A | aucun | **0,17 s** | **26 ms** (2 000 recherches d'index) |
| B | PL/pgSQL avec `EXCEPTION` | **échec : erreur interne PostgreSQL** | — |
| C | SQL, `float8` depuis les octets | 2,5 s | 51 ms |
| D | SQL, `numeric` exact | 6,4 s | 50 ms |

Comportement de la variante A : chevauchement, inclusion et chevauchement IPv6 refusés ;
plage adjacente, autre domaine et période disjointe acceptés.

Constats :
1. **`subtype_diff` n'apporte rien à cette échelle** et coûte au chargement ; la
   conception prévoyait de l'utiliser : amendement proposé ci-dessous.
2. **Erreur interne** avec la variante B : `ERROR: lock BufferContent is not held` lors
   d'insertions dans l'index, avec une fonction `subtype_diff` contenant un bloc
   `EXCEPTION` (sous-transaction pendant la maintenance de l'index). Règle : aucune
   fonction appelée par un type ou un index ne doit contenir de bloc `EXCEPTION`. (Le
   comportement ressemble à un défaut de PostgreSQL ; reproduction minimale à isoler
   avant tout signalement en amont.)
3. **`ANALYZE` échoue** sur les variantes C et D : depuis PostgreSQL 17, les opérations de
   maintenance s'exécutent avec un `search_path` restreint, et une fonction SQL qui en
   appelle une autre sans la qualifier par son schéma n'est plus trouvée. Règle : toute
   fonction utilisée par un type, un index ou une contrainte qualifie ses appels ou fixe
   son `search_path`.
4. **PostgreSQL accepte une plage mixte** `[10.0.0.1,::1]` (IPv4 est trié avant IPv6) :
   une contrainte `CHECK (family(lower(r)) = family(upper(r)))` est nécessaire.
5. Un premier essai de sonde par jointure de hachage a pris 8,6 s : le planificateur
   n'utilisait pas l'index pour la condition `@>` ; les requêtes du port de stockage
   doivent forcer la forme `LATERAL` (recherche indexée par élément), comme pour le parent.

### (c) Adresses : unicité par domaine et unicité publique partielle — **confirmé**

`EXCLUDE (domain_id WITH =, address inet_ops WITH =, valid_period WITH &&)` et
`EXCLUDE (address inet_ops WITH =, valid_period WITH &&) WHERE (public AND usage <> 'anycast')` :

- adresse publique dupliquée dans un autre domaine → refus ; adresse privée dupliquée
  dans un autre domaine → acceptée ; même adresse publique anycast dans deux domaines →
  acceptée ;
- anycast dans un domaine et hôte dans un autre : **non détecté** par la contrainte
  partielle, comme prévu par la conception (règle applicative `core.public.overlap`) ;
- chargement incrémental de 996 717 adresses sous les deux contraintes : **105 s**
  (≈ 105 µs par ligne) ; index 96 Mo (par domaine) et 17 Mo (public, 275 262 lignes) ;
- insertion unitaire dans la table chargée : 100 insertions en 13 ms (0,13 ms chacune) ;
- construire les contraintes **après** un chargement en masse est plus lent
  (93 s + 62 s) que l'insertion incrémentale : on garde les contraintes pendant les
  imports (lots de changesets, 004/C3).

### (d) Concurrence — **confirmé**

Deux transactions insèrent le même préfixe dans le même domaine : la seconde attend la
validation de la première (2 s), puis échoue sur la contrainte d'exclusion. Le dernier
rempart fonctionne sous concurrence ; le verrou de domaine (ADR 0015) donnera en plus un
message applicatif riche.

## Conséquences pour la conception

1. **Amendement proposé à 002/design** (§ Stockage) : le type `inetrange` est créé **sans
   `subtype_diff`**, avec `CHECK (family(lower(r)) = family(upper(r)))`. À réévaluer si
   le nombre de plages dépasse 10⁵.
2. **Règles pour toutes les migrations** (à reporter dans la conception de la 004, aide
   de migration) : pas de bloc `EXCEPTION` dans une fonction appelée par un type, un index
   ou une contrainte ; appels qualifiés par schéma ou `SET search_path` sur ces fonctions.
3. **Requêtes du port de stockage** : recherches par élément en `LATERAL` pour les
   prédicats d'inclusion (`>>=`, `@>`) ; vérifier les plans dans les tests (002-24,
   003-15).
4. Budget d'import : la part base de données d'un import de 10⁶ adresses est d'environ
   2 min sur 15 ; marge confortable, à confirmer avec le pipeline complet (004-23).
