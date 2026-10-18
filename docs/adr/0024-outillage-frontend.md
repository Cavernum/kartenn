---
status: accepted
date: 2026-10-18
---

# 0024 — Outillage du frontend : pnpm, Vite, ESLint, Vitest, Playwright

## Context

Le shell Web et le SDK de plugins (D17, ADR 0011) vivent sous `web/` (`shell/`, `sdk/`).
L'environnement de développement est décrit par devenv ; les sessions Claude Code doivent
pouvoir vérifier le frontend avec une sortie courte et voir l'interface rendue.

## Decision

- **pnpm workspace** sous `web/` (shell et SDK comme paquets du même espace de travail).
- **Vite** pour le serveur de développement et le build ; **vue-tsc** pour les types.
- **ESLint** (`eslint-plugin-vue`, configuration TypeScript de Vue) et **Prettier**.
- **Vitest** pour les tests unitaires et de composants, **Playwright** pour les parcours
  de bout en bout.
- Navigateurs fournis par Nix (`playwright-driver.browsers`), jamais téléchargés par npm ;
  la version npm de Playwright suit celle de nixpkgs.
- Claude Code pilote l'interface par le serveur MCP Playwright (navigateur headless isolé,
  origines limitées au serveur de développement local).

## Alternatives

- npm workspaces : rien à installer, mais résolution moins stricte et plus lente.
- oxlint ou Biome : plus rapides, couverture des fichiers `.vue` encore partielle.
- Navigateur personnel via extension : non reproductible, inutilisable en CI.

## Consequences

- Vérifications courtes `k-web-check` et `k-web-e2e`, pendants de `k-check`.
- Montée de version de Playwright liée à celle de nixpkgs.
