---
status: accepted
date: 2026-10-22
---

# 0032 — Une instance PostgreSQL dédiée aux bancs, sous un verrou à part

## Context

Plusieurs sessions et worktrees partagent une machine (24 cœurs). Les commandes lourdes
passent par le sémaphore `k-heavy` : deux créneaux pour les compilations et les tests,
et un verrou exclusif `--pg` sur l'instance PostgreSQL de développement (port 5432).
Ce verrou protège deux usages : les bases jetables de `#[kartenn_testing::pg_test]` et
de `#[sqlx::test]` (ADR 0028), que deux `k-check` concurrents se supprimeraient
mutuellement, et la base `kartenn_bench`, que chaque banc recrée.

Constat du 2026-10-22 : la machine est presque inactive (charge 2,7 sur 24 cœurs), mais
tout attend sur `--pg`. Un banc de référence le tient pendant une heure ou davantage,
et tous les `k-check` des autres worktrees attendent derrière lui. Les agents
d'implémentation relancent des bancs complets pour itérer, ce qui prolonge l'attente.
Enfin, l'attente d'un `flock` ne disait ni qui tient le verrou ni depuis quand.

## Decision

1. **Une instance dédiée aux bancs.** Le script devenv `k-pg-bench` garantit une
   instance PostgreSQL 18, du même paquet que `services.postgres` :
   - données partagées par tous les worktrees, sous
     `${XDG_STATE_HOME:-$HOME/.local/state}/kartenn/pg-bench`, et non sous
     `$DEVENV_STATE`, propre à chaque worktree ;
   - port 5433, écoute sur `127.0.0.1`, socket unix dans ce même répertoire ;
   - initialisée une fois avec les rôles, les bases et le script initial de
     `services.postgres` (rôle `kartenn` CREATEDB, rôle `kartenn_app`), repris de la
     configuration devenv, sans copie ;
   - démarrée à la demande, si `pg_isready` ne répond pas, comme unité systemd
     utilisateur transitoire `kartenn-pg-bench`, pour survivre au terminal ; à défaut de
     systemd, par `pg_ctl start` ;
   - jamais arrêtée par le script ; idempotent, sûr en concurrence (`flock` sur un
     fichier du répertoire autour de l'initialisation et du démarrage).
2. **Les bancs visent cette instance.** Chaque commande `bench-*` qui recrée
   `kartenn_bench` appelle `k-pg-bench`, puis vise `K_BENCH_DATABASE_URL`
   (`postgres://kartenn@127.0.0.1:5433/kartenn_bench` par défaut, surchargeable).
3. **Un verrou à part.** Les bancs prennent `k-heavy --bench`, et non plus `--pg` :
   un `k-check` et un banc ne s'attendent plus. `--pg` reste aux tests (`k-check`),
   à `db-prepare` et aux parcours de bout en bout (`k-web-e2e`). `bench-web` prend les
   deux : il recrée `kartenn_bench`, mais occupe aussi les ports fixes et l'état de
   `web/e2e` que partagent les parcours. Ordre d'acquisition : `--bench`, puis `--pg`,
   puis les créneaux.
4. **Des attentes visibles et bornées.** À l'acquisition d'un verrou ou d'un créneau,
   `k-heavy` écrit qui le tient (commande, pid, worktree, heure). Un appelant qui attend
   l'affiche sur une ligne, la répète toutes les 5 minutes et abandonne après
   `K_HEAVY_WAIT` minutes (45 par défaut pour `--pg` et `--bench`, sans limite pour les
   créneaux), en code 75.
5. **Deux échelles de banc.** `K_BENCH_SCALE=reduced` (défaut) divise les volumes par
   environ 10 et prévient que le verdict ne vaut pas mesure de budget ;
   `K_BENCH_SCALE=full` est l'échelle de référence. Un agent d'implémentation n'utilise
   que l'échelle réduite ; la mesure complète qui coche une tâche de budget est lancée
   une fois, par l'orchestrateur, en fin de série.

## Alternatives

- **Verrous séparés sur la même instance** (5432) : les bancs et les tests ne se
  bloqueraient plus, mais les mesures seraient bruitées par les tests qui créent et
  détruisent des bases sur le même serveur (WAL, checkpoints, cache partagé). Une
  instance à part isole au moins les tampons, les checkpoints et les autovacuums ; la
  machine, elle, reste partagée.
- **Statu quo** : correct, mais le débit de développement est borné par la durée des
  bancs, sans que la machine soit chargée.

## Consequences

- Une deuxième instance tourne en permanence après le premier banc ; elle se gère comme
  la première (`systemctl --user stop kartenn-pg-bench` pour l'arrêter, ses données
  sous `~/.local/state/kartenn/pg-bench`).
- Un banc et des tests tournent désormais en même temps : la machine partagée reste une
  source de bruit, que les créneaux de `k-heavy` bornent. Une mesure de budget se lit
  avec la charge du moment (`uptime`).
- Un nouveau banc reprend le gabarit des scripts `bench-*` de `devenv.nix` ; une
  requête ou un `EXPLAIN` sur la base de banc vise `K_BENCH_DATABASE_URL`.
- Un banc réduit ne coche pas une tâche de budget.
