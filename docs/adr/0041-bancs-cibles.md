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

3. **Mesures touchées par une modification.** Une table du crate `kartenn-bench` associe
   chaque mesure aux chemins du dépôt qu'elle exerce.
   - `bench-affected [base]` lit `git diff --name-only <base>...HEAD`, avec main pour
     base par défaut, plus les fichiers modifiés. Il lance en échelle réduite les mesures
     concernées.
   - Un chemin de code qu'aucune mesure ne couvre déclenche tout le banc de son crate.
   - Un test vérifie que chaque mesure a ses chemins et que chaque crate exercé par un
     banc y figure.

4. **Historique local des mesures.** Chaque passage ajoute une ligne JSON par mesure à
   `.bench/history.jsonl`, à la racine de la copie principale. Le chemin se déduit de
   `git rev-parse --git-common-dir` : le parent de ce dossier. Tous les worktrees
   écrivent donc dans le même fichier. Le fichier est exclu par `.git/info/exclude`,
   commun aux worktrees (entrée `/.bench/` ajoutée par le script si elle manque), et
   n'est jamais versionné.
   - Une ligne porte : date, commit, arbre modifié ou non, banc, mesure, échelle,
     graine, valeur, unité, seuil, verdict et charge de la machine au départ.
   - `bench-history <mesure>` affiche la série d'une mesure.

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
