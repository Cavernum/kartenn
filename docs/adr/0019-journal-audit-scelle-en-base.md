---
status: proposed
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
