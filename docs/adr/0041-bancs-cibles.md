---
status: accepted
date: 2026-10-31
---

# 0041 — Bancs ciblés : base préparée, mesures choisies, historique et bissection

## Context

Les bancs (`bench-*`, ADR 0032) recréent leur base à chaque passage et rejouent toute la
préparation avant de mesurer. En pleine échelle, la préparation domine : pour
`bench-core`, 34 min avant les machines, dont 33 min de jeu de données, pour quelques
minutes de mesures. Un banc mesure tout son module ou rien, et rien ne garde les
chiffres d'un passage à l'autre : la seule référence est le tableau daté d'un design.

Conséquences constatées les 2026-10-30 et 31 :

- un banc pleine échelle lancé à côté de quatre agents a gardé un créneau de `k-heavy`
  dix heures durant ;
- trois budgets de la 003 dépassent en pleine échelle (retrait des racines de trois
  domaines : 1 197,8 s, contre 13,8 s le 2026-10-15). Faute d'historique, on ignore à
  quel commit la mesure a dérivé. La bissection à la main relance la préparation à
  chaque étape.

## Decision

1. **Préparation et mesures séparées.** Chaque banc se découpe en une préparation
   déterministe (graine, échelle) et en mesures nommées (`core.machines-import`,
   `props.origins-3-domains`…). `kartenn-bench <banc> --list` donne les mesures,
   `--only <m1,m2>` n'exécute qu'elles.
   - Une mesure part de l'état laissé par la préparation, ou de celui laissé par les
     mesures qu'elle déclare comme préalables. Le programme les exécute avant elle.
   - Une mesure qui chronomètre la préparation elle-même (chargement, import) est
     marquée « de préparation ». Elle ne tourne qu'avec la préparation.

2. **Base préparée réutilisée.** La base qui sort de la préparation devient une base
   modèle de l'instance des bancs : `kartenn_bench_tpl_<banc>_<empreinte>`.
   - L'empreinte couvre les migrations, les sources du générateur et de la préparation
     du banc, la graine et l'échelle.
   - Un passage dont l'empreinte existe repart d'une copie,
     `CREATE DATABASE kartenn_bench TEMPLATE … STRATEGY FILE_COPY`. PostgreSQL 18 peut
     cloner les fichiers sur btrfs (`file_copy_method = clone`, à vérifier à la tâche
     2). Sinon, il prépare puis enregistre le modèle.
   - Trois modèles au plus sont gardés par banc ; le plus ancien est supprimé.
   - Précisé à la tâche 2 (session principale) : l'exactitude prime sur la réutilisation,
     la bissection (décision 5) en dépend. L'empreinte couvre donc aussi les sources
     (`Cargo.toml`, `src/`, `migrations/`) des crates du workspace par lesquels passe la
     préparation du banc, avec leurs dépendances du workspace (`kartenn-core`,
     `kartenn-service`, `kartenn-store-pg` pour tous ; `kartenn-auth`, `kartenn-rest`,
     `kartenn-server` pour `http` ; les modules machines, politique et flux pour
     `policy` et `flow`), et `Cargo.lock`. Un test vérifie que chaque crate que nomment
     les sources d'une préparation y figure. Coût : un modèle est refait dès qu'un de ces
     crates change, même sans effet sur la base préparée ; une bissection sur un
     intervalle qui touche le cœur reprépare donc à presque chaque étape.
   - La préparation finit par `VACUUM (ANALYZE)` : carte de visibilité et statistiques
     sont dans le modèle. La copie est lue avant les mesures (`pg_prewarm` si la base a
     l'extension, sinon lecture séquentielle de ses tables), pour qu'elles partent à
     chaud comme sur la base préparée. Sur l'instance des bancs, le script `bench` crée
     `pg_prewarm` dans `template1` (superutilisateur par le socket, idempotent, sans
     redémarrage) : chaque base préparée, donc chaque modèle, l'a, et la copie relit
     tables et index. Sans index réchauffés, le changeset ≤ 10⁴ de `props` doublait sur
     copie en réduit (0,65 à 0,86 s contre 0,31 à 0,36 s) ; avec, il reste 0,44 à 0,45 s
     contre 0,31 à 0,38 s, les deux autres mesures sont égales à 5 % près. Le reste ne
     tient ni au cache (copie réchauffée) ni au clonage btrfs (une vraie copie,
     `file_copy_method = copy`, donne 0,40 à 0,42 s) : `CREATE DATABASE … FILE_COPY`
     fait un point de contrôle juste avant la copie, et la première mesure qui écrit
     paie les images de page complètes (`full_page_writes`) de chaque page qu'elle
     touche. Un `CHECKPOINT` ajouté avant les mesures de la base préparée sans modèle la
     porte à 0,38 à 0,45 s. La mesure dépend donc du dernier point de contrôle, avec ou
     sans modèle. La version de forme de l'empreinte (2) fait refaire les modèles
     d'avant.
   - Les mesures partent donc d'un point de contrôle frais, des deux côtés : le
     programme lance `CHECKPOINT` après la préparation (base préparée sans modèle) ou
     après la copie et son réchauffement, avant la première mesure. C'est le pire cas
     des images de page complètes, le même pour une copie et pour une base préparée.
     Le script `bench` donne `pg_checkpoint` à `kartenn` sur l'instance des bancs
     (superutilisateur par le socket, idempotent) ; ailleurs, sans ce droit, le programme
     le dit sur sa sortie d'erreur et mesure depuis le dernier point de contrôle du
     serveur. La ligne `INFO checkpoint before the measures` du rapport dit lequel. Les lignes du rapport écrites par la préparation
     sont gardées dans le commentaire du modèle et rendues par chaque copie : deux
     passages de même empreinte rendent les mêmes lignes.
   - Vérifié à la tâche 2 : `file_copy_method` est un réglage de contexte `user`. Le
     programme le pose par session (`SET file_copy_method = clone`) avant la copie, sans
     toucher à la configuration de l'instance ni la redémarrer. Les données de
     l'instance des bancs sont sur btrfs : la copie clone les fichiers
     (`copy_file_range`), et un modèle ne partage que ses blocs avec ses copies.
   - Un modèle n'accepte aucune connexion (`ALLOW_CONNECTIONS false`). Avant de
     renommer ou de copier, le programme attend jusqu'à 10 s qu'aucun client
     (`client backend`) ne reste connecté à la base ; les processus du serveur
     (autovacuum…) sont laissés au serveur. L'erreur nomme les clients restés. Le
     programme refuse `--template` hors d'une base `kartenn_bench…`. Ce qu'une mesure tire de la
     préparation (identifiants…) est relu dans la copie, au premier passage comme aux
     suivants : les deux mesurent sur une copie, de la même façon.
   - Le nettoyage ne supprime que les bases nommées comme les modèles du banc
     (`kartenn_bench_tpl_<banc>_` suivi de 16 chiffres hexadécimaux). Les scripts
     `bench-*` ne gardent de modèles que sur l'instance des bancs (port 5433).

3. **Mesures touchées par une modification.** Une table du crate `kartenn-bench` associe
   chaque mesure aux chemins du dépôt qu'elle exerce.
   - `bench-affected [base]` lit `git diff --name-only <base>...HEAD`, avec main pour
     base par défaut, plus les fichiers modifiés. Il lance en échelle réduite les mesures
     concernées.
   - Un chemin de code qu'aucune mesure ne couvre déclenche tout le banc de son crate.
   - Un test vérifie que chaque mesure a ses chemins et que chaque crate exercé par un
     banc y figure.
   - Précisé à la tâche 4. Dans le doute, on sélectionne plus, jamais moins. La base par
     défaut est `origin/main`, la `main` locale d'un worktree étant souvent en retard ;
     `--list` donne la sélection sans rien lancer. Pour chaque chemin modifié, chaque banc
     tourne :
     - **entier**, quand le chemin est dans son empreinte (décision 2 : sources de sa
       préparation, crates que traverse sa préparation, toute migration, `Cargo.lock`) ;
     - sinon, **les mesures dont les chemins le couvrent**, et celles qui partent de
       l'état qu'elles laissent (entier si ce sont toutes) ;
     - sinon, pas du tout.

     La table (`kartenn_bench::affected`) donne pour chaque mesure les modules de
     `kartenn-bench` que traverse son code, depuis le fichier qui la chronomètre, et les
     crates qu'ils nomment, entiers (`crates/<crate>/`), avec leurs dépendances du
     workspace, hors de l'empreinte du banc. Elle ne nomme aucun fichier isolé d'un autre
     crate : la plomberie (pipeline, écriture d'un changeset, contrôle d'accès, kit REST,
     serveur) est atteinte par toutes les mesures qui passent par son crate. Un test la
     redérive des sources : imports `crate::<module>`, crates `kartenn_<crate>::`. Un
     module partagé de `kartenn-bench` relance donc tous les bancs qui l'importent.

     Les autres chemins :
     - la racine, les commandes, le manifeste et le script de build de `kartenn-bench`,
       ainsi que le manifeste du workspace, relancent tous les bancs ;
     - la configuration des bancs (`devenv.nix`, `devenv.lock`) ne lance rien, mais
       conseille un passage complet ;
     - un diff de `web/v2/`, `web/` ou `clients/python/` propose `bench-ui`, `bench-web`
       ou `bench-cli` (Playwright, Python, sans échelle réduite), sans les lancer ;
     - le reste ne lance rien.

     L'empreinte couvre aussi le `build.rs` et le `data/` des crates (fichiers lus par
     `include_str!`).

4. **Historique local des mesures.** Chaque passage ajoute une ligne JSON par mesure à
   `.bench/history.jsonl`, à la racine de la copie principale. Le chemin se déduit de
   `git rev-parse --git-common-dir` : le parent de ce dossier. Tous les worktrees
   écrivent donc dans le même fichier. Le fichier est exclu par `.git/info/exclude`,
   commun aux worktrees (entrée `/.bench/` ajoutée par le script si elle manque), et
   n'est jamais versionné.
   - Une ligne porte : date, commit, arbre modifié ou non, banc, mesure, échelle,
     graine, valeur, unité, seuil, verdict et charge de la machine au départ.
   - `bench-history <mesure>` affiche la série d'une mesure.
   - Précisé à la tâche 3 : une ligne par chiffre d'une mesure, avec le nom de son budget
     (une mesure peut en donner plusieurs : p95 et p99, lecture courante et passée…).
     Champs, dans cet ordre : `date` (UTC, début du passage), `commit`, `dirty`, `bench`,
     `measure`, `budget`, `scale`, `seed`, `value`, `unit` (`s`, `%`, `B`, `/s`, `count`),
     `limit` (`null` sans budget), `verdict` (`pass`, `fail`, ou `info` pour un chiffre
     rapporté sans budget), `load` (charge sur une minute au départ), `template`
     (empreinte du modèle de la décision 2, `null` sans `--template`). Ni chemin ni nom
     d'utilisateur.
   - C'est le programme qui écrit : `--history`, que passent les scripts `bench-*`, ou
     `--history=<fichier>` pour un autre fichier (tests, bissection). Il ajoute `/.bench/`
     à l'exclusion s'il manque et lit le commit et l'état de l'arbre du répertoire courant
     au départ. Les lignes d'un passage partent en une seule écriture, en ajout, sous
     verrou exclusif du fichier (`flock`) : deux worktrees n'entrelacent ni ne tronquent
     jamais une ligne.

5. **Bissection.** `bench-bisect <mesure> [--good <rev>] [--bad <rev>]` lance
   `git bisect run` dans un worktree dédié, puis le supprime.
   - **Bornes** : sans `--good`, le bon commit est le dernier de l'historique où la
     mesure tenait, à la même échelle. L'historique resserre ainsi l'intervalle avant
     toute compilation.
   - **Critère** : la médiane de trois exécutions de la seule mesure, sur une copie du
     modèle de l'étape. Le seuil est le milieu entre la valeur du bon commit et celle du
     mauvais, pas le budget, pour rester loin du bruit.
   - **Commit qui ne compile pas** : sauté (code 125).
   - **Conditions** : machine libre (`k-heavy --status`), sous le verrou des bancs et
     avec un créneau, avec le chien de garde `K_BENCH_STALL`.

6. **Rythme.**
   - **Tâche qui touche du Rust exercé par un banc** : son agent lance `bench-affected`
     en échelle réduite. C'est la seule exception à « aucun banc par l'agent ».
   - **Fin d'une série** : la session principale lance la suite complète en pleine
     échelle, sur une machine sans autre agent.
   - **Dépassement ou dérive** : `bench-bisect`. Une dérive, c'est une mesure qui
     s'écarte de plus de 20 % de sa médiane sur les cinq derniers passages à la même
     échelle.

## Consequences

- Un passage ciblé coûte la copie du modèle plus les mesures choisies, au lieu de toute
  la préparation. Une bissection en échelle réduite devient de l'ordre d'une heure.
- L'instance des bancs garde jusqu'à trois modèles par banc, donc plus de disque (à
  mesurer à la tâche 2). Un changement de migration invalide les modèles : le premier
  passage suivant reprépare.
- Une mesure dont l'état dépend d'une autre doit le déclarer. Un préalable oublié donne
  une mesure fausse, sans erreur. Un test compare une mesure lancée seule (`--only`) et
  la même mesure dans le banc entier, à la petite échelle des tests.
- Une régression visible seulement en pleine échelle demande une bissection en pleine
  échelle. Elle reste possible grâce aux modèles, mais coûte une préparation par
  migration rencontrée.
- L'historique est propre à la machine. Il ne vaut pas mesure de budget pour une
  validation : seul compte le passage pleine échelle consigné dans le design de la spec.
- Complète l'ADR 0032 sans le remplacer : même instance, même verrou, mêmes scripts.

### Coût d'un modèle (tâche 2)

`bench-core 42 --only core.read-object` sur l'instance des bancs (btrfs, PostgreSQL 18.6),
mesuré le 2026-10-31. La copie du premier passage suit la préparation ; celle des
passages suivants la remplace. La taille est `pg_database_size` ; le disque propre est la
part exclusive donnée par `btrfs filesystem du`, le reste étant partagé avec la copie.

| Échelle  | Préparation | Copie, premier passage | Copie, passages suivants | Taille  | Disque propre du modèle |
| -------- | ----------- | ---------------------- | ------------------------ | ------- | ----------------------- |
| réduite  | 10,6 s      | 2,1 s                  | 0,35 à 0,84 s            | 201 Mio | 0 o (201 Mio partagés)  |
| complète | à mesurer   | à mesurer              | à mesurer                | à mesurer | à mesurer             |
| disque total : 3 modèles × 8 bancs, pleine échelle | — | — | — | à mesurer | à mesurer |

La ligne pleine échelle et le disque total reviennent à la session principale (aucun
banc pleine échelle par un agent). Les chiffres réduits ci-dessus précèdent le
`VACUUM (ANALYZE)` final et le réchauffement de la copie.
