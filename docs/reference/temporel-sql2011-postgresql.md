# Modèle temporel : SQL:2011 et support réel de PostgreSQL 18

Base des décisions D8 et D10 (bitemporel natif).

## Sources

| Document | Consulté |
|---|---|
| K. Kulkarni, J.-E. Michels (IBM), « [Temporal features in SQL:2011](https://sigmodrecord.org/publications/sigmodRecord/1209/pdfs/07.industry.kulkarni.pdf) », *SIGMOD Record* 41(3), sept. 2012. Le texte de la norme ISO/IEC 9075:2011 est payant : cet article de ses auteurs en est la présentation de référence. | 2026-10-13 |
| [PostgreSQL 18, CREATE TABLE](https://www.postgresql.org/docs/18/sql-createtable.html) et [notes de version 18](https://www.postgresql.org/docs/18/release-18.html) | 2026-10-13 |

## Ce que décrit SQL:2011

### Modèle de période
- « SQL:2011 has adopted a closed-open period model » : une période inclut son début et
  exclut sa fin, `[début, fin)`. La fin doit être strictement après le début.

### Tables à période applicative (application time)
- L'utilisateur fournit les dates de début et de fin.
- Clés `WITHOUT OVERLAPS` et clés étrangères avec `PERIOD`.
- `UPDATE … FOR PORTION OF période FROM … TO …` : une ligne qui ne recouvre que
  partiellement la période visée est **découpée** en deux ou trois lignes contiguës ;
  seule la partie dans la période est modifiée. Même logique pour `DELETE`.

### Tables versionnées système (system time)
- Colonnes de début et de fin `GENERATED ALWAYS AS ROW START/END` : l'utilisateur ne
  peut **ni les fixer ni les modifier**.
- `INSERT` fixe le début à l'horodatage de la transaction, qui reste constant pendant
  toute la transaction ; la fin prend la valeur maximale du type.
- `UPDATE` et `DELETE` ne touchent que les lignes **courantes** ; une copie historique
  est insérée automatiquement ; `DELETE` ne supprime rien, il clôt la période.
- Les lignes historiques forment une **chaîne contiguë, sans trou**, et sont immuables.
- Les contraintes ne s'appliquent qu'aux lignes courantes : les lignes historiques ont
  été validées quand elles étaient courantes.
- Requêtes : `FOR SYSTEM_TIME AS OF t`, `FROM t1 TO t2` (fermé-ouvert),
  `BETWEEN t1 AND t2` (fermé-fermé). Par défaut, `AS OF CURRENT_TIMESTAMP`.

### Bitemporel
- Les deux à la fois : le temps applicatif dit quand un fait était vrai dans le monde
  réel, le temps système dit quand la base l'a enregistré. La norme ne nomme pas ces
  tables ; « bitemporel » est le terme de la littérature.

## Ce que fait réellement PostgreSQL 18

| Fonction SQL:2011 | PostgreSQL 18 |
|---|---|
| Clés `WITHOUT OVERLAPS` sur une colonne intervalle | **Oui** (nouveauté de la v18) |
| Clés étrangères `PERIOD` | **Oui**, avec `NO ACTION` seulement |
| `PERIOD FOR` (période déclarée sur deux colonnes) | Non : on utilise une colonne de type intervalle (`tstzrange`) |
| `UPDATE/DELETE … FOR PORTION OF` | **Absent** en 18 ; ajouté puis retiré pendant le cycle de la 19 (voir la note PostgreSQL) |
| Tables versionnées système (`WITH SYSTEM VERSIONING`, `FOR SYSTEM_TIME AS OF`) | **Absent** |

## Pièges

- Le bitemporel complet n'existe **pas** clés en main dans PostgreSQL 18 : le temps
  système et le découpage `FOR PORTION OF` sont à la charge de kartenn.
- L'horodatage du temps système doit être **celui de la transaction**, identique pour
  toutes les lignes d'un même changeset ; `now()` en PostgreSQL renvoie justement
  l'heure de début de transaction.
- Avec plusieurs instances (D9), l'horloge de référence du temps système doit être celle
  de la base (un seul primaire en écriture), jamais celle des serveurs d'application.
- Les intervalles vides sont interdits dans `WITHOUT OVERLAPS` : une période applicative
  de durée nulle doit être refusée par la validation.

## Implications pour kartenn (propositions pour les specs 002 et 004)

1. **Convention** : toutes les périodes en `[début, fin)`, type `tstzrange`, fin infinie
   pour « en cours ». Le temps applicatif est `valid_period`, le temps système
   `system_period` (`valid_at` et `as_of` dans l'API, D10).
2. **Temps système fait par kartenn**, dans la même transaction que le changeset (D11) :
   l'horodatage système est celui de la transaction ; aucune ligne historique n'est
   modifiée ensuite ; une suppression clôt la période (D33). Deux mises en œuvre à
   comparer dans un ADR :
   - **table courante plus table d'historique** par type d'objet (contraintes simples
     sur la table courante, requêtes `as_of` sur l'union) ;
   - **table unique** avec `system_period` et contraintes partielles
     `EXCLUDE … WHERE (upper_inf(system_period))`.
   Critères : budgets D14 (« à la date X » au plus ×2), simplicité des contraintes,
   partitionnement.
3. **Découpage `FOR PORTION OF`** : implémenté dans la couche service (les opérations
   du changeset sur une portion de période produisent les lignes découpées), avec des
   tests de propriétés : contiguïté, absence de trou et de recouvrement.
4. **Contraintes** : seules les lignes courantes côté système sont contraintes ; c'est
   exactement la règle SQL:2011, et elle évite de contraindre l'historique.
5. **Immutabilité de l'historique** : droits PostgreSQL (le rôle applicatif n'a pas
   `UPDATE`/`DELETE` sur l'historique) en plus de la discipline applicative ; à
   préciser dans la spec 004.
