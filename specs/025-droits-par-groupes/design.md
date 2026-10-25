# 025 — Droits par groupes : conception

Statut : **brouillon, à valider**. Exigences : [`requirements.md`](requirements.md) ;
décisions : [`decisions.md`](decisions.md).

## Vue d'ensemble

Le sujet d'une liaison passe de trois sortes (identité, groupe du jeton, émetteur) à une
seule : le groupe kartenn. Une table d'appartenance relie les groupes kartenn à leurs
membres (identités, groupes de l'IdP). La décision résout d'abord les groupes kartenn du
sujet, puis lit les liaisons de ces groupes ; le reste du modèle de la 005 (périmètres,
confinement, prédicat, `ReadScope`) ne change pas.

## Modèle de données — migration `0066_rbac_groups.sql`

- `rbac_group (id, key, label, builtin)` : versionnée (`valid_period`, historique,
  ADR 0017), clé unique parmi les lignes valides (contrainte d'exclusion comme `role`).
  Nommée `rbac_group` : `group` est un mot réservé.
- `group_member (id, group_id, member_kind, identity_id, issuer, group_name)` :
  versionnée ; `member_kind` ∈ (`identity`, `idp_group`), colonnes utiles seulement
  (`CHECK`) ; unicité d'un membre par groupe parmi les lignes valides.
- `binding` : `subject_kind`, `subject_id`, `issuer`, `group_name` retirés au profit de
  `group_id NOT NULL REFERENCES` (par clé logique, comme `role_id`).
- Index : `group_member (identity_id)`, `group_member (issuer, group_name)`,
  `binding (group_id)`, tous partiels sur les lignes valides maintenant si le banc le
  justifie (sinon GiST sur `valid_period` comme l'existant).

## Résolution des groupes (REQ-025-4, 5)

Une requête par décision, en tête de la requête de `visibility_on`
(`crates/kartenn-store-pg/src/rbac.rs`) :

```sql
WITH member_of AS (
  SELECT m.group_id FROM group_member m
  JOIN rbac_group g ON g.id = m.group_id AND g.valid_period @> $at
  WHERE m.valid_period @> $at
    AND ((m.member_kind = 'identity' AND m.identity_id = $identity)
         OR (m.member_kind = 'idp_group' AND m.issuer = $issuer
             AND m.group_name = ANY($groups))))
… FROM binding b JOIN member_of USING (group_id) …
```

- Les groupes de l'IdP viennent toujours du `Subject` (session ou instantané d'un
  changeset en attente) ; l'appartenance est lue à la date de la décision : un membre
  retiré perd ses droits à la requête suivante, même sur un changeset déjà en attente.
- `rbac_read.rs` et `labels_read.rs` (droits effectifs, libellés) prennent la même CTE,
  extraite en une fonction SQL `kartenn_member_of(identity, issuer, groups, at)`
  `STABLE` pour ne pas la recopier.
- Pas de cache en v0.1 (005, K15) ; `bench-rbac` dit si la jointure de plus tient.

## Opérations (REQ-025-1, 2, 8, 10)

- `SetGroup` (crée ou renomme ; clé, libellé), `CloseGroup` (clôt aussi ses liaisons et
  ses appartenances) ; `AddMember`, `RemoveMember` (clôture de la période) ; `Bind`,
  `Unbind` prennent `group` (clé) au lieu de `subject`.
- Annulation (004, K16) : `UndoFacts` gagne groupes et appartenances, sur le modèle des
  rôles et liaisons : groupe créé → `CloseGroup`, d'avant → `SetGroup` ; appartenance
  créée → `RemoveMember`, disparue → `AddMember`. Ordre : groupes, puis appartenances,
  puis liaisons.
- `builtin` : `SetGroup`/`CloseGroup` refusés sur un groupe livré hors synchronisation
  (`core.group.builtin`), comme `core.role.builtin`.

## Règles (REQ-025-6 à 8)

- `core.binding.escalation` : inchangée sur le fond, le sujet devient le groupe.
- `core.group.escalation` (nouvelle, erreur) : pour `AddMember`, `RemoveMember` et
  `CloseGroup`, chaque liaison valide du groupe (à la date de l'opération, liaisons
  écrites par le même changeset comprises) est soumise au contrôle de
  `core.binding.escalation` au nom de l'auteur. Une liaison créée dans le même changeset
  que l'appartenance est déjà contrôlée par sa propre règle.
- `core.group.nested` : un membre `idp_group` ne peut pas nommer un groupe kartenn (la
  forme ne le permet pas ; la règle documente REQ-025-3 pour un import).
- `core.binding.subject` : une opération `Bind` qui nomme une identité ou un émetteur est
  refusée par la forme (400 côté API, `invalid-body`) ; pas de règle à part.
- Publication : `docs/spec/regles-noyau.md` (généré).

## Groupe livré `secours` (REQ-025-9)

- Constante du noyau à côté des rôles livrés ; liaison au rôle administrateur, périmètre
  global, synchronisée par le job de migration dans le changeset système.
- `kartenn_store_pg::breakglass::create` écrit une ligne `group_member` (identité du
  compte, période `[création, valid_until)`) au lieu de la liaison ; l'événement
  `account.breakglass.created` porte `group` au lieu de `role` et `scope`.

## API (REQ-025-11, 006)

- `GET/POST /v1/groups`, `GET/PATCH/DELETE /v1/groups/{id}` ;
  `GET/POST /v1/groups/{id}/members`, `DELETE /v1/groups/{id}/members/{member}` ;
  `/v1/bindings` garde ses routes, avec `group` (clé) pour sujet et un filtre `?group=`.
- `GET /v1/me` et `GET /v1/identities/{id}` : `groups` (`[{key, via}]`, `via` = `direct`
  ou le groupe de l'IdP qui donne l'appartenance) et `bindings` effectifs.
- Simulation (`?simulate=true`, 024) et brouillons (024) : les nouvelles opérations
  entrent dans `Operation` comme les autres.
- Client Python et CLI (023) : `kartenn group …`, `kartenn group member …` ; `bind`
  prend `--group`.

## Interface (REQ-025-12, 021)

- E7 : onglets **Groupes**, **Rôles**, **Identités**, **Comptes de service**,
  **Émetteurs**, **Audit**. Un groupe : membres (identités, groupes de l'IdP), puis ses
  rôles sur périmètres, chaque ligne avec ses actions (retirer, modifier une liaison) et
  l'ajout en pied de liste, en brouillon comme les autres écritures. Une action refusée
  par `core.group.escalation` ou `core.binding.escalation` l'est par `reason`, avec le
  motif « Périmètre plus large que le vôtre ».
- L'onglet Liaisons disparaît. La fiche d'une identité : état, groupes (et par quel groupe
  de l'IdP), droits effectifs, en lecture.

## Migration des données (REQ-025-14)

Dans le job `migrate`, après `0066`, en changeset système :

1. Pour chaque couple (`issuer`, `group_name`) lié : groupe `idp-<émetteur>-<groupe>`
   (clé normalisée), membre `idp_group`, liaisons reportées.
2. Pour chaque identité liée : groupe `compte-<clé>`, membre `identity`, liaisons
   reportées.
3. Liaisons d'émetteur entier : closes à la migration.
4. Sortie de `migrate` : la liste des groupes créés en 2 et des liaisons closes en 3 ;
   audit `rbac.migration.reported`.

Les liaisons de période bornée gardent leur période ; l'historique antérieur reste lisible
sous l'ancienne forme (colonnes archivées dans l'historique de `binding`).

## Erreurs et cas limites

| Cas | Effet |
|---|---|
| Identité sans groupe | aucun droit ; `GET /v1/me` sert `groups: []` |
| Groupe de l'IdP absent du jeton | appartenance sans effet pour cette session |
| Membre retiré pendant qu'un changeset de l'identité attend | revalidation avec les appartenances du moment : refus possible à l'approbation |
| Groupe clos avec un changeset en attente qui le lie | conflit à l'approbation (`core.binding.group_closed`) |
| Administrateur qui se retire de son propre groupe | autorisé si G5 le permet ; il perd ses droits à la requête suivante |

## Alternatives considérées

Voir `decisions.md` (G2 : sujets IdP et kartenn mêlés, IdP seuls ; G6 : imbrication ;
G8 : `core.group.admin`).

## Stratégie de test

- Décision : identité directe, par groupe de l'IdP, sans groupe ; appartenance échue ;
  confinement d'émetteur inchangé (`tests/rbac.rs`).
- Anti-escalade : matrice auteur (global, entité E, entité F) × groupe (global, E, F) ×
  opération (ajout, retrait, clôture, liaison) ; INV-27 étendu à l'appartenance.
- Annulation : groupe, appartenance, liaison, et leur ordre.
- Migration : base de la 005 avec les trois sortes de liaisons → groupes et liste.
- `bench-rbac` : 50 groupes de l'IdP par identité, 1 000 liaisons, 200 groupes kartenn.

## ADR

Aucune : la 005 et D36 restent le cadre ; la décision G1 amende K4 dans le journal de la
005 (renvoi vers la 025).
