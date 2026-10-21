---
description: Implémente une seule tâche d'une spec (ex. /tache 003-17), la vérifie, la commite et s'arrête
argument-hint: <NNN-M>
---

Tâche à réaliser : **$ARGUMENTS** (spec `NNN`, tâche `M` de `specs/NNN-*/tasks.md`).
Une seule tâche : ne commence pas la suivante. La tâche se livre **telle qu'écrite** :
un écart à la spec n'est jamais une décision de l'agent. Pas de plan à faire valider :
la spec l'est déjà ; on itère sur du code qui compile et des tests qui tournent.

1. **Contexte minimal.** `grep -n` la ligne de la tâche dans `specs/NNN-*/tasks.md` ; ses
   pointeurs (`où :`) donnent les fichiers, symboles et sections à lire d'abord. Puis les
   REQ qu'elle cite dans `requirements.md` et `design.md` (`k-ctx` pour situer, puis
   lecture des plages utiles), et les arbitrages déjà écrits qui la concernent (`grep -n`
   du numéro de tâche dans la spec). Ne relis pas les specs entières ni l'historique git au-delà
   de `git log --oneline -10`.
2. **Arbitrage, avant comme pendant l'implémentation.** Arrête-toi dès que la tâche :
   contredit une décision (D1…D37) ou une spec validée ; demande un choix **structurant**
   que la spec ne tranche pas ; ou ne peut être tenue entièrement (une clause, une REQ
   citée, un cas du `verify`). Est structurant ce qui coûte cher à défaire ou se voit de
   l'extérieur : contrat REST ou OpenAPI, schéma et migrations, format de configuration,
   sémantique de sécurité (authn, RBAC, audit, secrets), nouvelle dépendance, effet
   irréversible sur des données. Expose alors le problème, les options et ta
   recommandation en 10 lignes au plus, sans cocher ni commiter (le code en cours reste
   dans l'arbre de travail). Ne contourne pas l'obstacle par un cas refusé (400,
   `Unsupported`…), un comportement simplifié ou un paramètre ignoré que la spec ne prévoit
   pas. Les autres choix (découpage interne, nommage privé, structures et algorithmes
   internes, organisation des tests, messages de log) se tranchent sans demander : prends
   l'option la plus simple qui tient la spec, et note-la à la clôture.
3. **Implémentation** de toute la tâche, selon les compétences du projet (coding-rust),
   avec les tests qu'elle nomme dans son `verify`. Un test d'intégration est un module de
   `tests/it/` déclaré dans `tests/it/main.rs`, jamais un nouveau fichier `tests/*.rs`
   (ADR 0027, `docs/practices/rust-build.md` §2.5). Lis ciblé : outil `LSP` (définitions,
   références, symboles, type au survol) plutôt qu'une lecture pour trouver un symbole,
   `k-ctx` pour le plan d'un fichier, puis des plages de 80 lignes au plus.
4. **Vérification** : `k-check` sur les crates touchés (`-p <crate>`), puis `k-check`
   complet avant le commit, ou bien le sous-agent `test-runner`. Corrige jusqu'au vert.
   Relis ensuite la ligne de la tâche clause par clause : chacune a son code et son test.
   Sinon, retour à l'étape 2.
5. **Clôture** : coche la tâche dans `tasks.md`, avec une note courte (fichiers, tests)
   sur le format des tâches déjà cochées, sans rubrique d'écarts, et une ligne `choix :`
   pour les choix non structurants tranchés en route (s'il y en a) ; puis un commit atomique en
   Conventional Commits, avec le style des messages récents et `(task NNN-M)` dans le titre.
6. **Passation**, puis arrêt. 4 lignes au plus : ce qui est fait, le commit, la tâche
   suivante ; et, seulement s'il y en a un, un risque constaté hors du périmètre de la
   tâche. Termine par :
   « Tâche terminée — `/compact` (ou `/clear`) avant la suivante. »
