# kartenn — consignes de travail

- **Specs avant le code.** Toute nouvelle brique ou décision technique est d'abord
  discutée et écrite (`specs/NNN-slug/` pour une fonctionnalité, `docs/adr/` pour une
  décision d'architecture, `docs/spec/` pour la spec système). On n'implémente qu'à
  partir d'une spec validée par l'utilisateur. Dans une tâche, les choix peu coûteux à
  défaire et invisibles de l'extérieur se tranchent sans demander et se notent à la
  clôture ; seuls les choix structurants (contrat, schéma, sécurité, dépendance,
  irréversible) attendent un arbitrage (voir `/tache`).
- **Tâches ciblées.** Chaque tâche de `tasks.md` porte une ligne `où :` : fichiers et
  symboles à toucher ou à imiter, sections du design (titre ou numéro de ligne), test
  voisin à copier. Elle s'écrit avec la tâche et se corrige quand le code bouge.
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
- **Frontend** (`web/`, ADR 0024) : pnpm uniquement (pas de npm/npx), navigateurs Playwright
  fournis par Nix (`@playwright/test` épinglé sur la version de `playwright-driver`). Le
  serveur Vite est le processus devenv `web` : le lancer et lire ses logs par le MCP devenv
  (`start_process`, `get_process_logs`), jamais `pnpm dev` au premier plan. Voir le
  rendu avec le MCP `playwright` (headless, limité à `127.0.0.1:5173`) : `browser_snapshot`
  (arbre d'accessibilité, compact) plutôt que `browser_take_screenshot`, sauf question
  visuelle.
- **Charge de la machine.** Plusieurs sessions et worktrees partagent une machine. Les
  commandes lourdes passent par le sémaphore `k-heavy` (2 créneaux, `nice`, cœurs
  partagés) : `k-check`, `k-web-check`, `k-web-e2e`, `db-prepare`, les bancs et le hook
  clippy le prennent d'eux-mêmes ; toute autre commande lourde se lance en
  `k-heavy cargo …`. Pas plus de trois sous-agents d'implémentation en parallèle ;
  `sccache` partage les dépendances compilées entre worktrees.

## Économie de contexte

Le coût d'une session croît avec la taille du contexte × le nombre de requêtes.
- **Une tâche = un contexte.** Travailler tâche par tâche avec `/tache <NNN-M>` ; après le
  commit, s'arrêter sur un résumé de passation (l'état durable est dans la spec,
  `tasks.md` et git). Pas d'enchaînement de tâches dans une même session : pour un travail
  autonome long, `scripts/claude-taches.sh`.
- **Sorties courtes.** Vérifier avec `k-check [args nextest]` (verdict sur une ligne, détail
  seulement en échec) plutôt que `cargo test` brut ; côté web, `k-web-check [args vitest]`
  (prettier, eslint, vue-tsc, vitest) et `k-web-e2e [args playwright]`. Builds et commandes bavardes passent
  par `| tail`/`head` ou une redirection vers le scratchpad.
- **Lire ciblé.** Pour trouver un symbole, l'outil `LSP` (rust-analyzer : `workspaceSymbol`,
  `goToDefinition`, `findReferences`, `hover`) plutôt qu'une lecture. Pas de `cat` ni de
  `sed -n` d'un fichier de plus de 150 lignes sans `k-ctx <fichier>` d'abord (plan avec
  numéros de ligne ; Rust, SQL, Markdown, Vue, TypeScript) ou `grep -n`, puis des plages
  de 80 lignes au plus. `git diff --stat` avant un diff
  complet. Les décisions se lisent d'abord dans la synthèse en tête de `decisions.md`.
- **Regrouper** les commandes indépendantes dans un même appel ou des appels parallèles.
- **Déléguer le travail bavard** à un sous-agent léger qui ne rend que la conclusion :
  `test-runner` (haiku : verdict de tests/build), `bench-runner` (haiku : bancs, `psql`,
  `EXPLAIN`), `crate-scout` (sonnet : évaluer un crate, lire du code tiers),
  `ref-researcher` (sonnet : RFC, drafts, docs officielles → brouillon de note),
  `spec-keeper` (sonnet : dérive spec/code), `Explore` avec `model: haiku` (balayage).
  Conception, implémentation, revue et arbitrages restent dans l'agent principal. Ne pas
  déléguer la lecture d'un fichier qu'on va modifier.
