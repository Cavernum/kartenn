---
status: approved
updated: 2026-11-01
---

# 028 — Bancs ciblés : tâches

Outillage décidé par l'[ADR 0041](../../docs/adr/0041-bancs-cibles.md) (acceptée le
2026-10-31 ; historique dans l'arbre, exclu, au choix de l'utilisateur), en complément de l'ADR 0032. Chaque tâche est un commit qui laisse le build
vert. « verify » = preuve de la tâche ; « dépend de » = tâches à finir avant.

Commun à toutes les tâches : les budgets et leurs seuils ne changent pas. Une mesure
lancée seule ou dans le banc entier donne le même verdict. Aucun banc pleine échelle
par l'agent : la petite échelle des tests (`small`) et l'échelle réduite suffisent.

- [x] 1. Mesures nommées et `--only` : chaque banc de `kartenn-bench` (`props`, `core`,
  `rbac`, `http`, `policy`, `flow`, `events`, `telemetry` ; les statistiques sont des
  mesures de `core`, `core.stats`) se découpe en
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
  - fait : `kartenn_bench::measures` (`Measure`, `Bench`, `BENCHES`, `Selection` :
    préalables fermés transitivement, `runs`, `any` pour une mise en place partagée,
    ligne `mesures : … ; préalables : …` sous `--only`) ; un `BENCH` par module
    (`props.rs`, `core_budgets.rs` avec les mesures de `vlan_budgets.rs`,
    `machine_budgets.rs`, `stats_budgets.rs`, `rbac.rs`, `http.rs`, `policy_budgets.rs`,
    `flow_budgets.rs`, `events_budgets.rs`, `telemetry.rs`), chaque mesure sous
    `selection.runs(…)` ; `main.rs` (`<banc> --list [--only …]`, qui vérifie les noms
    et donne les mesures qui tourneraient ; `[small] [--only <m1,m2>]` dans tout ordre ;
    enfant `telemetry-run` avec les mêmes mesures ; la graine de `events`, qui ne tire
    rien, est désormais vérifiée comme entier non signé, comme ailleurs) ; `devenv.nix`
    (`bench` : arguments vérifiés avant tout verrou — `--only` sans valeur ou vide, deux
    `--only`, deux graines, argument inconnu refusés aussitôt ; `--list` hors verrou des
    bancs ; noms de `--only` vérifiés après la compilation, avant de recréer la base).
    Tests : `measures` (catalogues bien formés, listes figées, préalables, nom inconnu
    refusé avec la liste), `main` (arguments) ; `tests/it/only.rs` (une mesure seule
    contre le banc entier pour chaque banc, `core.stats` et `core.machines-suggestions`
    compris, à l'échelle `small`, un banc à la fois sous un verrou consultatif du
    cluster, lignes comparées durées, pourcentages et charge masqués ; ignorés par
    défaut : `k-check -p kartenn-bench --run-ignored all` ; `--list`, `--list --only`
    et nom inconnu par le binaire). Passage réduit
    `bench-properties 42 --only props.origins-3-domains`. Texte de la tâche corrigé : pas
    de banc `stats`, les statistiques sont la mesure `core.stats`.
  - choix : les tirages restent ceux du générateur du banc, dans l'ordre des mesures
    (le banc entier tire ce qu'il tirait ; seule, une mesure tire autre chose, de même
    loi) ; une mesure peut donner plusieurs lignes (boucles entrelacées) ; préalables
    d'état déclarés là où la mesure en dépend (écritures de `props` en chaîne ; dans
    `core`, ce qui lit les tables que les écritures grossissent part après elles : la
    fiche d'une machine après les écritures d'adresses, les machines d'un /24 après
    celles-ci et le conteneur large, les suggestions après toutes les écritures du cœur et
    des modules, `core.stats` après elles et l'import des machines ; rapport d'audit après
    le différentiel des règles ; aperçu et réévaluation de l'inférence après le contrôle,
    pages de non-conformités après les écritures de la politique, tempête après la
    mémoire, lecture passée après la lecture courante dont elle tire son seuil) ; mises
    en place partagées (VLAN, machines, règles d'inférence, lecteur par extrémités)
    lancées quand une de leurs mesures tourne ; seule mesure « de préparation » :
    `props.origins-every-root` (seul budget pris dans la préparation) ; la visibilité de
    `events` écrit d'abord les adresses qui manqueraient (aucune dans le banc entier) ;
    les clés des suggestions de `core` sont tirées par `md5(clé || graine)` et non plus
    `random()` : les mêmes d'un passage à l'autre, sans tirage du générateur du banc ;
    `bench-http` envoie `approval=self` sur ses écritures (sans lui, tout changeset passait
    en proposition et le banc échouait dès la première écriture, depuis la 024-11).
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
  - en cours : la ligne pleine échelle du tableau de l'ADR 0041 (préparation, copie,
    disque d'un modèle) et le disque total (3 modèles × 8 bancs), à mesurer par la session
    principale.
  - fait : `kartenn_bench::template` (`Reuse`, `prepared` : sans modèle de l'empreinte, ou
    sous une mesure « de préparation », la préparation écrit dans la base vide, puis la
    base est renommée en modèle `ALLOW_CONNECTIONS false`, commentée (banc, graine,
    échelle, date) et copiée ; sinon copie seule ; `CREATE DATABASE … TEMPLATE …
    STRATEGY FILE_COPY` après `SET file_copy_method = clone` ; connexions ouvertes sur la
    base renommée, la base du passage ou le modèle refusées avec leurs pid ; trois modèles
    par banc, les plus anciens par oid supprimés, seulement les noms
    `kartenn_bench_tpl_<banc>_<16 hex>` ; empreinte FNV-1a 64 bits du banc, de la graine,
    de l'échelle et de ses volumes, des migrations du cœur (sommes de sqlx) et des
    modules machines, politique, flux (SQL), des sources de la préparation par banc) ;
    chaque banc découpé en écriture de sa préparation (`write`) et relecture dans la
    copie de ce qu'en tirent les mesures (`core_budgets::prepare_with`, `Ids`, `author` ;
    `rbac` et `http` : liaisons et personnes dans la préparation ; `policy`, `flow` : modules,
    politique, flux et machines ; `events` : abonnés `r<i>`) ; `Selection::bench`,
    `prepares` ; `main.rs` (`--template`) ; `devenv.nix` (`bench` passe `--template` sur
    le port 5433 seulement). Tests : `template` (empreinte : migration ajoutée ou changée,
    graine, échelle, banc, sources ; noms ; trois gardés ; chaque banc liste ses sources),
    `measures` (`prepares`), `main` (`--template`) ; `tests/it/template.rs` (second passage
    copié aux mêmes lignes, mesure « de préparation » qui reprépare et remplace, modèle
    sans connexion permise, connexion ouverte refusée nommément) ; `tests/it/only.rs` :
    chaque banc entier prépare et garde son modèle, la mesure seule tourne sur sa copie.
    Passages réduits `bench-core 42 --only core.read-object` : préparation 10,6 s puis
    copie 0,35 s, mesures dans le bruit ; une migration ajoutée (sonde jetable) a donné une
    autre empreinte et une nouvelle préparation, retirée la copie du premier modèle ;
    tableau de l'ADR 0041 (ligne pleine échelle à mesurer par la session principale).
    Après relecture : clients seuls comptés (`backend_type = 'client backend'`), jusqu'à
    10 s d'attente avant l'erreur ; empreinte étendue aux crates du workspace traversés
    (`build.rs` : sources, `Cargo.toml`, `migrations/` de chaque dépendance du workspace
    hors développement, et `Cargo.lock` ; `template::crates`, `crate_hashes`, `Inputs`) ;
    `VACUUM (ANALYZE)` à la fin de chaque préparation, copie lue avant les mesures
    (`warm` : `pg_prewarm` si la base l'a, sinon lecture des tables) ; `--template` refusé
    hors d'une base `kartenn_bench…` ; lignes du rapport de la préparation gardées dans le
    commentaire du modèle et rendues par la copie ; `kartenn_testing::dropped_by_clean`
    (`BENCH_PREFIXES` : `kartenn_bench_it_`, `kartenn_bench_tpl_`), bases des tests des
    bancs nommées `kartenn_bench_it_…`. Tests : `template` (crates nommés par chaque
    préparation couverts, crate ou verrou changés, commentaire), `kartenn-testing`
    (`dropped_by_clean`), `tests/it/template.rs` (rapports `INFO` identiques entre passage
    préparé et copié, quatrième modèle élagué, client sur la base préparée refusé,
    `--template` hors `kartenn_bench…` refusé). Passages réduits sur copie, machine chargée
    (charge 4 à 6) : `bench-properties --only props.origins-3-domains` changeset 0,65 à
    0,86 s, valeur de toutes les racines 0,99 à 1,05 s, retrait de 3 domaines 3,14 à
    3,59 s, contre 0,31 à 0,36 s, 0,90 s et 2,91 à 3,11 s sur la base préparée sans
    modèle au même moment (design 003, code final, machine chargée : 0,53 s, 1,29 s,
    1,79 s) ; `bench-core --only core.read-object` 0,55 à 0,64 ms et 0,99 à 1,83 ms
    (passé), comme avant la relecture. `pg_prewarm` est absent des bases (extension non
    approuvée, `CREATE EXTENSION` demande un superutilisateur) : la lecture des tables ne
    réchauffe pas les index. Puis : `pg_prewarm` créé dans `template1` par le script `bench`
    (5433 seulement, superutilisateur par le socket), réchauffement des seules relations
    de l'utilisateur (`pg_has_role`), version de forme 2 dans l'empreinte. Mesures
    réduites entrelacées, copie réchauffée par `pg_prewarm` contre base préparée sans
    modèle, par le script : changeset 0,44 à 0,45 s contre 0,31 à 0,38 s, valeur de toutes
    les racines 0,89 à 0,96 s contre 0,82 à 0,90 s, retrait de 3 domaines 3,09 à 3,17 s
    contre 2,93 à 3,00 s. Reste du changeset : ni le cache ni le clonage (vraie copie
    0,40 à 0,42 s) ; le point de contrôle de `CREATE DATABASE … FILE_COPY` (images de page
    complètes à la première écriture) — un `CHECKPOINT` avant les mesures de la base
    préparée la porte à 0,38 à 0,45 s. Avec le `CHECKPOINT` des deux côtés (entrelacés,
    charge 3 à 4) : changeset 0,38 à 0,43 s sur copie contre 0,41 à 0,54 s sur base
    préparée, valeur de toutes les racines 0,84 à 0,91 s contre 0,82 à 1,01 s, retrait de
    3 domaines 3,01 à 3,19 s contre 2,96 à 3,05 s (4,61 s au premier passage, isolé).
  - choix : la copie aussi au premier passage (la base préparée est renommée en modèle,
    puis copiée) : premier passage et suivants mesurent de la même façon ; l'état tiré de
    la préparation (identifiants, abonnés, paires de machines…) est relu ou recalculé dans
    la copie, jamais transmis en mémoire ; les tirages de la préparation sont rejoués sans
    écrire (`props`, `policy`, `flow`) pour que les mesures tirent ce qu'elles tiraient ;
    `http` garde ses jetons et cookies hors du modèle (ils suivent les clés du serveur
    lancé à chaque passage), ses personnes et liaisons y sont ; sources à la granularité
    du fichier (une mesure modifiée dans le fichier d'une préparation invalide le
    modèle) ; empreinte exacte plutôt que réutilisation (session principale) : les
    sources des crates du workspace traversés et `Cargo.lock` y entrent, un modèle est
    refait dès qu'un d'eux change ; le module `template.rs` nomme les modules pour leurs
    migrations seulement (hors du test des crates nommés) ; réchauffement des relations
    les plus grandes d'abord ; `pg_prewarm` installé dans `template1` de l'instance des
    bancs par le script `bench` (session principale, option a), les modèles d'avant
    refaits par la version de forme de l'empreinte plutôt que par une extension ajoutée
    à la copie (que `kartenn` ne peut créer) ; mesures depuis un point de contrôle frais
    des deux côtés (session principale) : `template::checkpoint` après la préparation ou
    la copie réchauffée, `pg_checkpoint` donné à `kartenn` par le script `bench` sur 5433
    seulement, sans ce droit un avertissement et la suite (le `CHECKPOINT` propre à `core`
    retiré, sa ligne `INFO` désormais pour tous les bancs) ; ordre d'ancienneté par oid ; sans `--template` (`bench-web`, autre instance que
    celle des bancs), même chemin sans copie.
- [x] 3. Historique local : une ligne JSON par mesure et par passage dans
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
  - fait : `kartenn_bench::history` (`Figure` : mesure, budget, valeur, `Unit`, seuil,
    `Verdict` ; `Run` : date, commit, arbre modifié, banc, échelle, graine, charge,
    empreinte du modèle ; `Entry::line`/`parse`, champs en ordre fixe ; `location`
    (`git rev-parse --path-format=absolute --git-common-dir`, son parent), `exclude`
    (`/.bench/` ajouté à `info/exclude` s'il manque, sous verrou, le reste intact),
    `revision` (`HEAD`, `git status --porcelain`), `append` (une écriture en ajout sous
    `flock` exclusif), `read`, `series`) ; chaque chiffre d'un rapport porte sa mesure :
    `props::Budget::of(mesure, nom, seuil).measured(valeur, note)` (et `standing_for` pour
    les comptes et octets de `events` écrits en millisecondes), `Pair` de `rbac` et `http`
    avec leur mesure, chiffres hors `Budget` en `Figure` (index d'historique, dictionnaire,
    débit et erreurs de la charge HTTP, surcoûts modélisés de `telemetry` par sortie,
    `measure_of`) et chiffres seulement rapportés en `info` (instantané de la politique,
    import des machines, résolution des groupes, volume d'audit) ; les `run` rendent
    `(rapport, chiffres)`, le verdict du passage en découle ; `main.rs` (`--history`,
    `--history=<fichier>`, charge lue au lancement, commit et arbre lus avant la mesure,
    lignes écrites après le rapport ; `history <mesure> [--scale …]`, mesure vérifiée dans
    les catalogues) ; `template::Reuse::fingerprint` ; `devenv.nix` (`bench` passe
    `--history`, script `bench-history`) ; ADR 0041 (décision 4 précisée). Tests :
    `history` (ligne écrite et relue, unités et verdicts, série par échelle, deux worktrees
    × 4 fils qui écrivent des lignes de 10 ko dans le même fichier : aucune ligne entrelacée
    ni tronquée, `git status` propre des deux côtés, exclusion complétée une fois en gardant
    le fichier, lignes illisibles comptées), `main` (`--history`), `telemetry`
    (`measure_of`) ; `tests/it/only.rs` : chaque mesure du catalogue a ses lignes dans le
    banc entier, seule elle n'en a que pour les mesures lancées, sous la même mesure.
    Passage réduit `bench-core 42 --only core.read-object` : deux lignes dans
    `.bench/history.jsonl` de la copie principale, `/.bench/` ajouté en fin d'exclusion,
    `bench-history core.read-object` rend la série, `--scale full` aucune, `core.nope`
    refusé avec la liste.
  - choix : une ligne par chiffre (budget) d'une mesure plutôt qu'une par mesure, le nom du
    budget en champ `budget` (une mesure en donne parfois plusieurs) ; valeurs en unités de
    base (`s`, `B`) ; chiffre sans budget en `verdict: info` et `limit: null` ; l'écriture et
    l'exclusion faites par le programme (`--history`) plutôt que par le script, pour être
    testées ; un échec d'écriture de l'historique fait échouer le passage (après le
    rapport) ; `dirty` compte aussi les fichiers non suivis ; un passage sur une autre
    instance que celle des bancs (`K_BENCH_DATABASE_URL`) écrit aussi, `template` nul ;
    `bench-history` passe par `k-heavy` comme `--list` (il peut compiler).
- [x] 4. Mesures touchées : table mesure → chemins dans `kartenn-bench` ; script
  `bench-affected [base]` (`git diff --name-only <base>...HEAD` et fichiers modifiés,
  base main par défaut), en échelle réduite. Un chemin non couvert d'un crate exercé
  déclenche le banc de ce crate (ADR 0041, décision 3).
  - verify : test : chaque mesure a ses chemins, chaque crate exercé par un banc est
    couvert ; essais sur trois diffs (un fichier de `kartenn-flow`, une migration, un
    fichier de `web/` seul) avec la sélection attendue.
  - où : `crates/kartenn-bench/src/` (nouvelle table) ; `devenv.nix` (scripts).
  - dépend de : 1, 2
  - fait : `kartenn_bench::affected` (`PATHS` : chaque mesure, le fichier de
    `kartenn-bench` qui la chronomètre et les fichiers propres de ce qu'elle mesure, par
    groupes partagés (`ROUTER`, `VERDICT`, `EVENTS`, `TELEMETRY`…), un dossier finissant
    par `/` ; `exercised` : crates de la préparation (`template::crates`) et des chemins
    des mesures, fermés par `template::crate_hashes`, et `kartenn-bench` ; `touched` : un
    chemin listé → ses mesures ; une source de la préparation (`template::sources`) → le
    banc entier, et les mesures qu'il chronomètre ailleurs ; un autre chemin de code
    (`src/`, `migrations/`, `Cargo.toml`, `build.rs`) d'un crate exercé → les bancs
    entiers qui l'exercent ; `Cargo.lock`, `Cargo.toml` du workspace → tous ; `web/v2/`,
    `web/`, `clients/python/` → `bench-ui`, `bench-web`, `bench-cli` proposés, non lancés ;
    le reste, rien ; `affected` : bancs dans l'ordre des catalogues, un banc entier
    l'emporte sur ses mesures, `plan` (`<script> [<m1,m2>]`)) ; `template::named_crates`
    sorti des tests pour être partagé ; `main.rs` (`affected`, chemins sur l'entrée,
    plan sur la sortie, explication par chemin et scripts à proposer sur l'erreur) ;
    `devenv.nix` (`bench-affected [base] [--list]` : diff `<base>...HEAD` sans renommages,
    plus `git diff HEAD` et fichiers non suivis, base `origin/main` par défaut, dite dans
    l'aide ; `--list` n'en lance rien ; `K_BENCH_SCALE=full` refusé ; bancs l'un après
    l'autre, chacun sous son verrou, échecs comptés) ; ADR 0041 (décision 3 précisée).
    Tests : `affected` (chaque mesure a ses chemins et son fichier de `kartenn-bench`,
    chaque chemin existe et est du code hors migrations, crates nommés par le code d'un
    banc exercés et chemins de ses mesures atteints par lui, chaque crate exercé listé
    sauf `kartenn-workspace-hack`, fichier de `kartenn-flow` listé ou non, migrations du
    cœur et d'un module, banc entier qui l'emporte, fichiers de `kartenn-bench`, web et
    documentation, `Cargo.lock`, scripts de `devenv.nix`). Essais sur des diffs jetables
    (`bench-affected HEAD`) : `crates/kartenn-flow/src/read.rs` → `bench-flow --only
    flow.flows-page,flow.nonconformities-page,flow.endpoints-flows-page,
    flow.endpoints-nonconformities-page`, lancé en réduit (nouvelle préparation, l'empreinte
    couvrant le crate ; 8 PASS, préalables compris) ; migration ajoutée
    `crates/kartenn-store-pg/migrations/0074_….sql` (non suivie) → les huit bancs entiers ;
    `web/v2/ui/…` seul → aucune mesure, `bench-ui` proposé ; `web/shell/…` seul → aucune
    mesure, `bench-web` proposé.
  - choix : règle globale (un chemin listé par une mesure, quel que soit son banc, ne lance
    que les mesures qui le listent) plutôt que par banc, qui relancerait presque tout ;
    plomberie commune et migrations jamais listées : elles relancent les bancs entiers ;
    crates exercés fermés par les dépendances du workspace (prudent : `kartenn-server`
    tire les modules, un fichier non listé de `kartenn-flow` relance aussi `http` et
    `telemetry`) ; `affected.rs`, `history.rs`, `measures.rs` et les `tests/` d'un crate
    ne lancent rien, le reste de `kartenn-bench` (`main.rs`, `Cargo.toml`) tous les bancs ;
    `devenv.nix` ne lance rien ; `bench-ui`, `bench-web` et `bench-cli` proposés et non
    lancés (Playwright ou Python, sans échelle réduite, `bench-web` avec le verrou `--pg`) ;
    `core.history-index` n'a que son fichier (SQL seul, dans les migrations) ; `--list`
    ajouté pour voir la sélection ; la sélection passe par `k-heavy` (elle peut compiler).
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
