---
status: approved
updated: 2026-10-17
---

# 012 — Journalisation, observabilité et audit : tâches

Chaque tâche est un commit, laisse le build vert et nomme ce qu'elle implémente. Les
événements d'authentification et de requêtes HTTP sont émis par la 005 et la 006 ; la 012
en fournit le mécanisme et les teste avec des appels directs.

## P1 — Bibliothèque syslog (L24)

- [x] 1. Crate `syslog-sender` : `Facility`, `Severity`, `Message`, validation des champs
  (RFC 5424 §6), données structurées et échappement (§6.3), BOM, formats RFC 5424 et
  RFC 3164, longueur maximale avec troncature sur une frontière UTF-8 — REQ-012-21, 29 —
  verify : exemples de RFC 5424 §6.5, aller-retour par `syslog_rfc5424`, `proptest` sur
  l'échappement et la troncature — un champ réduit à `-` est refusé (il serait lu
  comme absent : trouvé par les tests de propriétés) ; hors du `workspace-hack` (hakari)
- [x] 2. Cadrage (comptage d'octets, fin de ligne, datagramme) et transports tokio :
  socket local, UDP, TCP, TLS rustls (TLS 1.2 minimum, sans 0-RTT), écriture par lot,
  reconnexion avec attente bornée — REQ-012-21 — verify : serveurs TCP et TLS de test
  (certificat généré), coupure et reconnexion ; essai contre le collecteur de devenv —
  `Sender::close` envoie le `close_notify` TLS (sans lui, le récepteur voit une fin
  tronquée : trouvé par le test TLS) ; un message de deux lignes reçu comme un seul
  enregistrement par le collecteur

## P2 — Modèle et stockage de l'audit

- [x] 3. `kartenn_core::audit` : `AuditEvent`, `Severity`, `Criticality`, `Actor`,
  `Outcome` ; catalogue constant (nom, sévérité, criticité, champs, exigences, OCSF, ECS),
  publié dans `docs/spec/evenements-audit.md` par `KARTENN_BLESS` ; JCS (RFC 8785) et
  empreinte d'un lot ; `Secret<T>` — REQ-012-4, 5, 10, 28, 30 — verify : tests du
  catalogue, vecteurs de RFC 8785, affichage masqué — 36 événements, noms à trois parties
  (`changeset.state.<état>`) ; la forme canonique n'accepte que des entiers (aucun
  événement ne porte de flottant)
- [x] 4. Migration `0023_audit.sql` : schéma `audit`, tables partitionnées par criticité
  puis par mois, droits `INSERT, SELECT` du rôle applicatif, création des partitions par
  la procédure de maintenance, `audit.instance_key` — REQ-012-11, 12, 16 — verify :
  `UPDATE`/`DELETE` refusés sous `SET ROLE` ; partitions des mois à venir présentes —
  `kartenn-store-pg/tests/audit_schema.rs` ; attribution vérifiée aussi par la base
  (`event_attributed`) ; clé HMAC de 244 bits aléatoires (deux UUID v4)
- [x] 5. Port `AuditJournal`, `RequestContext`, `MemoryJournal` ; `PgAuditJournal` seul et
  `WriteTx::audit` dans la transaction ; erreur `RequestError::Audit` — REQ-012-2, 9, 11
  — verify : tests des deux adaptateurs ; échec fermé (droit `INSERT` retiré : changeset
  non appliqué) — `kartenn-service/tests/audit_journal.rs` : l'échec est simulé par un
  déclencheur qui refuse l'insertion (les tests tournent avec le rôle propriétaire) ; une
  écriture et son événement dans la même transaction disparaissent ensemble ; le
  changeset non appliqué se vérifie avec ses événements (tâche 6)

## P3 — Événements émis

- [x] 6. Écritures et administration : `changeset.<état>` dans la transaction du journal,
  `authz.denied`, politiques d'approbation, déclarations, imports, annulation,
  pseudonymisation, migration — REQ-012-8, 9 — verify : un test par ligne du tableau
  concernée (`MemoryJournal` et PostgreSQL) — `kartenn-service/tests/audit_events.rs` ; les événements
  passent par `Journal::audit`, `Store::audit` (refus sans autre écriture) et
  `close_pending` ; `submit_in` reçoit le contexte, `submit` en déduit un ; l'auteur
  système agit comme `system` ; les refus de droits en revue (valider son propre
  changeset, retirer celui d'un autre) sont aussi audités ; l'annulation produit un
  changeset ordinaire, audité à sa soumission
- [ ] 7. Lectures : `RequestContext` dans les lectures du store et du service,
  `data.objects.read` (`low`) ; attribut `sensitive` des déclarations (amendement 003 :
  migration, opérations, droits) et `data.sensitive.read` (`high`) ; consultation de
  l'audit (`audit.events.read`, permission dédiée) — REQ-012-17, 18 — verify : tests ;
  amendement de la 003 écrit avant le code
- [ ] 8. Agrégation des échecs d'authentification par minute et par (source, empreinte)
  — REQ-012-19 — verify : 1 000 échecs donnent au plus 11 événements, total exact

## P4 — Intégrité, export, rétention

- [ ] 9. Scellement par lots chaînés, signature Ed25519 facultative, `audit.seal.created`
  — REQ-012-13 — verify : transaction longue scellée au tour suivant ; signature vérifiée
- [ ] 10. Vérification : `kartenn-server audit verify`, vérification planifiable,
  `audit.chain.broken` — REQ-012-14 — verify : modification, ajout et suppression
  détectés, lot désigné
- [ ] 11. Port `AuditSink`, relais par sortie avec enregistrement des livraisons ;
  `FileSink`, `SyslogSink` (`syslog-sender`), `OtlpSink` (envoi acquitté) ; événements
  `sys.output.unavailable` / `available` — REQ-012-15, 26 — verify : coupure de 5 min
  puis reprise sans trou ; métrique de retard
- [ ] 12. Purge par criticité, conditionnée au scellement et à l'export — REQ-012-16 —
  verify : partition non exportée conservée ; vérification de la chaîne après purge

## P5 — Logs techniques et télémétrie

- [ ] 13. Crate `kartenn-telemetry` : couche de capture, table des sévérités, files
  bornées par sortie, compteurs d'abandons, masquage, neutralisation, bornes de taille
  — REQ-012-4, 5, 22, 28, 29 — verify : tests unitaires ; sortie bloquée sans effet sur
  les autres
- [ ] 14. Sorties `stdout` (texte, JSON), `file`, `journald`, `syslog` — REQ-012-3, 20, 21
  — verify : même événement sur chaque sortie (sévérité, nom, horodatage)
- [ ] 15. OpenTelemetry : traces, métriques, logs sans double export, échantillonneur et
  appelants de confiance, Baggage ignoré, filtres anti-boucle, métriques de REQ-012-25 —
  REQ-012-6, 7, 24, 25 — verify : test d'intégration contre un récepteur OTLP de test
  (un seul export par événement, `trace_id` commun)
- [ ] 16. Configuration (`figment`), validation au démarrage, défaut sûr ; sous-commande
  `kartenn-server run` (télémétrie, relais, scellement), arrêt propre — REQ-012-31, 34 —
  verify : configurations invalides refusées ; `SIGTERM` sous charge
- [ ] 17. Filtres à chaud : table `observability_override`, relecture, retour à
  l'échéance, opération d'administration auditée — REQ-012-27 — verify : test sur deux
  instances
- [ ] 18. Écart d'horloge, crochet de panique — REQ-012-32, 33 — verify : horloge simulée
  décalée ; panique journalisée avec `trace_id`

## P6 — Vérifications d'ensemble et synchronisation

- [ ] 19. Test des dépendances (`cargo metadata`) : `kartenn-core` et `kartenn-service` sans
  crate de sortie — REQ-012-2 — verify : test
- [ ] 20. Test de bout en bout des secrets témoins sur toutes les sorties — REQ-012-28 —
  verify : aucun témoin trouvé
- [ ] 21. Banc : `bench-core` avec trois configurations de télémétrie, volume de l'audit
  des lectures mesuré — REQ-012-23 — verify : surcoûts sous 5 % et 10 % ; volume reporté
  dans la conception
- [ ] 22. Synchroniser `docs/spec/` (configuration, événements, schéma `audit`,
  invariants), ADR 0019 → accepté, D35 précisé, statut `implemented` — verify : rapport
  spec-keeper sans écart
