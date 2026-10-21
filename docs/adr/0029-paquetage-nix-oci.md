---
status: proposed
date: 2026-10-21
---

# 0029 — Paquetage par un flake Nix : une image OCI, un module NixOS

## Context

La v0.1 doit s'installer (D35). Le dépôt n'a ni `flake.nix` ni CI ; devenv épingle
nixpkgs par `devenv.yaml`. D35 prévoyait trois images (serveur, migration, web
statique), mais le serveur sert lui-même le shell et les plugins (W2, ADR 0025) et le
job de migration a le même binaire que le serveur. Spec :
[015](../../specs/015-paquetage-deploiement/decisions.md) (E1 à E8).

## Decision

1. Un `flake.nix` à la racine, sur le nixpkgs de devenv, sans autre entrée ; sorties :
   paquets, image, module NixOS, vérifications.
2. Binaire par `rustPlatform.buildRustPackage` (sqlx hors ligne), interface par
   `pnpm.fetchDeps`, plugins de première partie livrés.
3. Une seule image OCI, `dockerTools.buildLayeredImage`, non root, sans shell :
   instance, migration et compte de secours par la commande. D35 est amendée.
4. Module `services.kartenn` : migration en unité oneshot séparée, instances nommées
   sous `DynamicUser`, secrets par `LoadCredential=`, durcissement systemd.

## Alternatives

- crane ou naersk : entrée de plus, gain de cache sans CI.
- Trois images : la troisième contredit W2, les deux premières ont le même contenu.
- Migration au démarrage des instances : N instances concurrentes et rôle sans droits
  de propriétaire.

## Consequences

- Une version de nixpkgs à tenir alignée entre `devenv.lock` et `flake.lock` (test).
- L'empreinte des dépendances pnpm est à recalculer à chaque changement du verrou.
- L'URL de la base passe par fichier (`[database] url_file`) : ajout additif à la
  configuration.
