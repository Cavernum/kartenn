---
status: accepted
date: 2026-10-17
---

# 0019 — Journal d'audit en base, en échec fermé, scellé par lots chaînés

## Context

kartenn doit journaliser et attribuer chaque action (spec 012), sans perte silencieuse
et de façon vérifiable après coup. Les transports courants (OTLP, écrivains non
bloquants) abandonnent les événements quand ils saturent. Les instances sont multiples et
sans état (D9) : un chaînage strict événement par événement les sérialiserait toutes.

## Decision

- Les événements d'audit sont écrits dans PostgreSQL, schéma `audit`, dans la transaction
  de l'action quand elle écrit, sinon avant la réponse. Si l'écriture échoue, l'action
  échoue.
- Le rôle applicatif n'a que `INSERT` et `SELECT` sur ce schéma. La purge (rétention par
  criticité) passe par le rôle propriétaire, seulement pour des événements scellés et
  exportés.
- Une tâche scelle régulièrement les événements validés en lots : SHA-256 du lot chaîné
  au sceau précédent, signature Ed25519 facultative. L'appartenance de chaque événement à
  un lot est enregistrée, pour qu'un événement validé tardivement ne crée pas de trou.
- Un relais par sortie exporte les événements au moins une fois, par séquence
  croissante, avec enregistrement des livraisons. Les sceaux sont exportés comme des
  événements, ce qui les ancre hors du système.

## Alternatives

- Audit seulement vers syslog ou OTLP : pertes possibles sous charge.
- Chaînage à l'écriture : verrou global entre instances.
- Base d'audit séparée : plus d'atomicité avec l'action sans transaction distribuée.
- Arbre de Merkle avec preuves d'inclusion : utile pour un journal public, inutile ici.

## Consequences

- Une insertion par action auditée, y compris chaque requête de lecture (criticité
  `low`) ; volume à surveiller et rétention par criticité.
- Une panne de la base empêche les actions, ce qu'elle faisait déjà ; une panne d'une
  sortie d'audit n'empêche rien, elle retarde l'export.
- L'intégrité est vérifiable par une commande ; un attaquant maître de la base peut
  réécrire les lots non encore exportés : le délai de scellement et d'export (une minute
  environ) borne cette fenêtre.

## Précisions de l'implémentation (2026-10-17)

- Le scellement et chaque relais lisent à partir d'un plancher (`pass_floor`) et vident
  leur retard par lots successifs à chaque tour : leur coût suit le retard, pas la taille
  du journal, et l'export suit tout débit (un lot par tour le bornait à 500 événements/s
  par sortie). Un événement validé tardivement reste couvert (amendement 3 de
  l'ADR 0018).
- Volume mesuré : environ 8 Kio par lecture auditée d'une page de 100 objets, dont
  5,8 Kio dans l'index GIN par objet ; 3,3 Gio par jour à 5 pages par seconde. Volume
  accepté, réglable : identifiants enregistrés par lecture (`audit.read_ids_max`) et
  rétention `low` (012/L25).

## Amendement — 2026-11-11, écriture hors changeset auditée dans sa transaction (019/Q-019-22)

Un ajout au port PostgreSQL du journal, ratifié par l'utilisateur (019/Q-019-22, option A
contre une outbox) : `PgAuditJournal::record_in(conn, event)` inscrit un événement dans la
transaction de l'appelant, comme `WriteTx::audit` le fait pour un changeset. Une écriture
qui n'est pas un changeset (un lot du journal d'observations, ADR 0043, décision 5) est
ainsi auditée dans sa transaction : l'événement est validé avec elle ou annulé avec elle,
jamais l'un sans l'autre ; un échec de l'inscription fait échouer l'écriture (échec
fermé). Le port `AuditJournal`, hors transaction, ne change pas : il garde les refus et
les lectures. Le schéma `audit` et ses droits ne changent pas.

Source : `crates/kartenn-store-pg/src/audit_journal.rs` (`record_in`), appelée par
`crates/kartenn-kube/src/rest.rs` (`observation.batch.recorded`). Test : `kartenn-kube`
`observations::route::batch_is_audited_once_with_hash`.
