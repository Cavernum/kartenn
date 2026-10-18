# kartenn — consignes de travail

- **Specs avant le code.** Toute nouvelle brique ou décision technique est d'abord
  discutée et écrite (`specs/NNN-slug/` pour une fonctionnalité, `docs/adr/` pour une
  décision d'architecture, `docs/spec/` pour la spec système). On n'implémente qu'à
  partir d'une spec validée par l'utilisateur.
- **Décisions de référence** : `specs/001-cadrage/decisions.md` (D1…D37). Toute
  proposition qui contredit une décision le signale et demande un arbitrage.
- **Sources primaires** : les notes de `docs/reference/` s'appuient sur les RFC
  (rfc-editor.org), les drafts IETF (datatracker) et les documentations officielles.
  Chaque note cite URL et section, et distingue les exigences normatives
  (MUST/SHOULD, RFC 2119/8174) des choix propres à kartenn.
- **Confidentialité** : aucun nom d'organisation réelle, ni d'exemple tiré du SI de
  l'utilisateur, dans les specs ni dans la documentation.
- **Langue** : documentation en français pour l'instant (question ouverte Q-ouv-6).
- **Commits** : Conventional Commits, atomiques.
- **Environnement** : devenv (`devenv shell`, `devenv test`). PostgreSQL 18 via `devenv up`.

## Économie de contexte

Le coût d'une session croît avec la taille du contexte × le nombre de requêtes.
- **Une tâche = un contexte.** Travailler tâche par tâche avec `/tache <NNN-M>` ; après le
  commit, s'arrêter sur un résumé de passation (l'état durable est dans la spec,
  `tasks.md` et git). Pas d'enchaînement de tâches dans une même session : pour un travail
  autonome long, `scripts/claude-taches.sh`.
- **Sorties courtes.** Vérifier avec `k-check [args nextest]` (verdict sur une ligne, détail
  seulement en échec) plutôt que `cargo test` brut. Builds et commandes bavardes passent
  par `| tail`/`head` ou une redirection vers le scratchpad.
- **Lire ciblé.** Pas de `cat` d'un fichier de plus de 150 lignes : `k-ctx <fichier>` (plan
  avec numéros de ligne) ou `grep -n`, puis la plage utile. `git diff --stat` avant un diff
  complet. Les décisions se lisent d'abord dans la synthèse en tête de `decisions.md`.
- **Regrouper** les commandes indépendantes dans un même appel ou des appels parallèles.
- **Déléguer le travail bavard** à un sous-agent léger qui ne rend que la conclusion :
  `test-runner` (haiku : verdict de tests/build), `bench-runner` (haiku : bancs, `psql`,
  `EXPLAIN`), `crate-scout` (sonnet : évaluer un crate, lire du code tiers),
  `ref-researcher` (sonnet : RFC, drafts, docs officielles → brouillon de note),
  `spec-keeper` (sonnet : dérive spec/code), `Explore` avec `model: haiku` (balayage).
  Conception, implémentation, revue et arbitrages restent dans l'agent principal. Ne pas
  déléguer la lecture d'un fichier qu'on va modifier.
