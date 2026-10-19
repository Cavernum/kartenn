---
description: Implémente une seule tâche d'une spec (ex. /tache 003-17), la vérifie, la commite et s'arrête
argument-hint: <NNN-M>
---

Tâche à réaliser : **$ARGUMENTS** (spec `NNN`, tâche `M` de `specs/NNN-*/tasks.md`).
Une seule tâche : ne commence pas la suivante. La tâche se livre **telle qu'écrite** :
un écart à la spec n'est jamais une décision de l'agent.

1. **Contexte minimal.** `grep -n` la ligne de la tâche dans `specs/NNN-*/tasks.md`, puis
   les REQ qu'elle cite dans `requirements.md` et `design.md` (`k-ctx` pour situer, puis
   lecture des plages utiles), et les arbitrages déjà écrits qui la concernent (`grep -n`
   du numéro de tâche dans la spec). Ne relis pas les specs entières ni l'historique git au-delà
   de `git log --oneline -10`.
2. **Arbitrage, avant comme pendant l'implémentation.** Arrête-toi dès que la tâche :
   contredit une décision (D1…D37) ou une spec validée ; demande un choix que la spec ne
   tranche pas ; ou ne peut être tenue entièrement (une clause, une REQ citée, un cas du
   `verify`). Expose le problème, les options et ta recommandation en 10 lignes au plus,
   sans cocher ni commiter (le code en cours reste dans l'arbre de travail). Ne contourne
   pas l'obstacle par un cas refusé (400, `Unsupported`…), un comportement simplifié ou un
   paramètre ignoré que la spec ne prévoit pas.
3. **Implémentation** de toute la tâche, selon les compétences du projet (coding-rust),
   avec les tests qu'elle nomme dans son `verify`. Lis ciblé (`k-ctx`, `grep -n`, plages).
4. **Vérification** : `k-check` sur les crates touchés (`-p <crate>`), puis `k-check`
   complet avant le commit, ou bien le sous-agent `test-runner`. Corrige jusqu'au vert.
   Relis ensuite la ligne de la tâche clause par clause : chacune a son code et son test.
   Sinon, retour à l'étape 2.
5. **Clôture** : coche la tâche dans `tasks.md`, avec une note courte (fichiers, tests)
   sur le format des tâches déjà cochées, sans rubrique d'écarts ; puis un commit atomique en
   Conventional Commits, avec le style des messages récents et `(task NNN-M)` dans le titre.
6. **Passation**, puis arrêt. 4 lignes au plus : ce qui est fait, le commit, la tâche
   suivante ; et, seulement s'il y en a un, un risque constaté hors du périmètre de la
   tâche. Termine par :
   « Tâche terminée — `/compact` (ou `/clear`) avant la suivante. »
