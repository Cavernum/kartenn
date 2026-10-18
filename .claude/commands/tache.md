---
description: Implémente une seule tâche d'une spec (ex. /tache 003-17), la vérifie, la commite et s'arrête
argument-hint: <NNN-M>
---

Tâche à réaliser : **$ARGUMENTS** (spec `NNN`, tâche `M` de `specs/NNN-*/tasks.md`).
Une seule tâche : ne commence pas la suivante.

1. **Contexte minimal.** `grep -n` la ligne de la tâche dans `specs/NNN-*/tasks.md`, puis
   les REQ qu'elle cite dans `requirements.md` et `design.md` (`k-ctx` pour situer, puis
   lecture des plages utiles). Ne relis pas les specs entières ni l'historique git au-delà
   de `git log --oneline -10`.
2. **Arbitrage.** Si la tâche contredit une décision (D1…D37), une spec validée, ou demande
   un choix que la spec ne tranche pas : arrête-toi, expose le problème et les options en
   10 lignes au plus, sans coder ni cocher.
3. **Implémentation** selon la tâche et les compétences du projet (coding-rust), avec les
   tests qu'elle nomme dans son `verify`. Lis ciblé (`k-ctx`, `grep -n`, plages).
4. **Vérification** : `k-check` sur les crates touchés (`-p <crate>`), puis `k-check`
   complet avant le commit, ou bien le sous-agent `test-runner`. Corrige jusqu'au vert.
5. **Clôture** : coche la tâche dans `tasks.md`, avec une note courte (fichiers, tests)
   comme les tâches déjà cochées ; puis un commit atomique en Conventional Commits, avec
   le style des messages récents et `(task NNN-M)` dans le titre.
6. **Passation**, puis arrêt. 5 lignes au plus : ce qui est fait, le commit, les écarts
   ou dettes notés, la tâche suivante. Termine par :
   « Tâche terminée — `/compact` (ou `/clear`) avant la suivante. »
