---
status: approved
updated: 2026-10-31
---

# 028 — Bancs ciblés : tâches

Outillage décidé par l'[ADR 0041](../../docs/adr/0041-bancs-cibles.md) (acceptée le
2026-10-31 ; historique dans l'arbre, exclu, au choix de l'utilisateur), en complément de l'ADR 0032. Chaque tâche est un commit qui laisse le build
vert. « verify » = preuve de la tâche ; « dépend de » = tâches à finir avant.

Commun à toutes les tâches : les budgets et leurs seuils ne changent pas. Une mesure
lancée seule ou dans le banc entier donne le même verdict. Aucun banc pleine échelle
par l'agent : la petite échelle des tests (`small`) et l'échelle réduite suffisent.

- [ ] 1. Mesures nommées et `--only` : chaque banc de `kartenn-bench` (`props`, `core`,
  `rbac`, `http`, `policy`, `flow`, `events`, `stats`, `telemetry`) se découpe en
  préparation et en mesures nommées `<banc>.<mesure>`, chacune avec ses préalables. Les
  mesures qui chronomètrent la préparation sont marquées « de préparation ».
  `kartenn-bench <banc> --list` les donne ; `--only <m1,m2>` exécute la préparation, les
  préalables et les mesures choisies. Les scripts `bench-*` passent les arguments
  (ADR 0041, décision 1).
  - verify : tests à l'échelle `small` : `--list` stable ; pour chaque banc, une mesure
    lancée seule et la même dans le banc entier donnent le même verdict ; un nom inconnu
    est refusé avec la liste.
  - où : `crates/kartenn-bench/src/main.rs` (l. 121-140, aiguillage des bancs) ;
    `*_budgets.rs`, `props.rs`, `rbac.rs`, `http.rs`, `telemetry.rs` (mesures actuelles,
    `progress`) ; `devenv.nix` (`bench`, l. 302).
  - dépend de : —
- [ ] 2. Base préparée réutilisée : empreinte (migrations, sources du générateur et de la
  préparation du banc, graine, échelle) ; modèle
  `kartenn_bench_tpl_<banc>_<empreinte>` enregistré après la préparation ; passage
  suivant repartant de `CREATE DATABASE … TEMPLATE … STRATEGY FILE_COPY` ; essai de
  `file_copy_method = clone` sur btrfs (PostgreSQL 18) ; trois modèles au plus par banc.
  Les mesures « de préparation » forcent une préparation (ADR 0041, décision 2).
  - verify : deuxième passage réduit sans préparation, avec des mesures égales à la
    marge de bruit près ; une migration ajoutée change l'empreinte et provoque une
    nouvelle préparation ; temps de copie et disque d'un modèle mesurés en réduit et en
    pleine échelle (tableau dans l'ADR).
  - où : `crates/kartenn-bench/src/main.rs`, `lib.rs` (connexion, création de la base) ;
    `devenv.nix` (`bench`, `pgBench` et ses réglages) ; ADR 0032 (instance, verrou).
  - dépend de : 1
- [ ] 3. Historique local : une ligne JSON par mesure et par passage dans
  `.bench/history.jsonl` à la racine de la copie principale (parent de
  `git rev-parse --git-common-dir`), exclu par `.git/info/exclude` (`/.bench/`, ajouté
  s'il manque) (date, commit, arbre
  modifié ou non, banc, mesure, échelle, graine, valeur, unité, seuil, verdict, charge
  au départ) ; script `bench-history <mesure> [--scale <échelle>]` (ADR 0041,
  décision 4).
  - verify : test du format écrit et relu ; deux worktrees écrivent dans le même
    fichier ; `git status` reste propre.
  - où : `crates/kartenn-bench/src/main.rs` (rapport final) ; `devenv.nix` (scripts).
  - dépend de : 1
- [ ] 4. Mesures touchées : table mesure → chemins dans `kartenn-bench` ; script
  `bench-affected [base]` (`git diff --name-only <base>...HEAD` et fichiers modifiés,
  base main par défaut), en échelle réduite. Un chemin non couvert d'un crate exercé
  déclenche le banc de ce crate (ADR 0041, décision 3).
  - verify : test : chaque mesure a ses chemins, chaque crate exercé par un banc est
    couvert ; essais sur trois diffs (un fichier de `kartenn-flow`, une migration, un
    fichier de `web/` seul) avec la sélection attendue.
  - où : `crates/kartenn-bench/src/` (nouvelle table) ; `devenv.nix` (scripts).
  - dépend de : 1, 2
- [ ] 5. Bissection : script `bench-bisect <mesure> [--good <rev>] [--bad <rev>]` dans un
  worktree dédié, supprimé à la fin. Sans `--good`, le bon commit est le dernier de
  l'historique où la mesure tenait à la même échelle. Le critère est la médiane de
  trois exécutions de la seule mesure, avec pour seuil le milieu entre la bonne et la
  mauvaise valeur. Un commit qui ne compile pas est sauté (125). La machine doit être
  libre (`k-heavy --status`), sous le verrou des bancs, avec `K_BENCH_STALL`
  (ADR 0041, décision 5).
  - verify : essai sur une régression introduite exprès dans une branche jetable (une
    pause dans une requête d'une mesure réduite), trouvée au bon commit ; essai sans
    `--good` : bornes tirées de l'historique.
  - où : `devenv.nix` (scripts, `k-heavy`) ; historique de la tâche 3.
  - dépend de : 2, 3
- [ ] 6. Règles et synchronisation : `.claude/agents/kartenn-tache.md` (`bench-affected`
  réduit permis à une tâche Rust exercée par un banc) ; `CLAUDE.md` (« Charge de la
  machine ») ; `docs/practices/rust-build.md` et ADR 0032 (renvoi) ; ADR 0041 →
  `accepted` ; `specs/README.md` (ADR 0041, décision 6).
  - verify : relecture `kartenn-revue` sans écart bloquant.
  - où : fichiers cités.
  - dépend de : 1 à 5
