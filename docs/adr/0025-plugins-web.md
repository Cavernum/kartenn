---
status: accepted
date: 2026-10-19
---

# 0025 — Plugins Web : répertoire servi, code de confiance, SDK partagé

## Context

Le shell Web accueille des plugins ajoutés ou mis à jour sans reconstruction (D6, D17,
ADR 0011). La session du navigateur est un cookie `__Host-` sur la même origine que l'API
(ADR 0020) ; tout code chargé dans le document agit donc avec la session de
l'utilisateur. Décisions 007/W3, W4, W7.

## Decision

- Les plugins sont déposés par l'exploitant dans un répertoire configuré
  (`[web] plugins_dir`) ; kartenn-server les sert sous `/plugins/<nom>/<version>/`, sur la
  même origine, en calcule les empreintes SRI et les publie dans `ui_plugins` de
  `GET /v1/capabilities` ; la relecture du répertoire les met à jour à chaud.
- Un plugin est du code de confiance : même document, Custom Elements, CSP stricte
  (`script-src 'self'`, `connect-src 'self'`, aucun cadre), sans restriction propre au
  plugin ; il ne peut jamais plus que l'utilisateur.
- Le SDK `@kartenn/sdk` est une instance unique, désignée par l'import map du shell et
  marquée `external` au build des plugins ; le manifeste déclare la plage de SDK attendue,
  vérifiée au chargement.
- Chaque module, feuille et catalogue est chargé avec son empreinte (`modulepreload`,
  `<script type=module>`, `fetch`).

## Alternatives

- Iframe sandboxée et permissions déclarées : pont d'API, mise en page et thème à
  reconstruire ; contredit D17.
- URL externes en configuration : CSP ouverte à des tiers.
- Téléversement par l'API : injection de code par un droit applicatif.
- SDK embarqué par chaque plugin : copies divergentes, contexte partagé par événements.

## Consequences

- Installer un plugin revient à lui confier la session de chaque utilisateur ; la
  documentation de l'exploitant le dit.
- Avec N instances, le répertoire des plugins doit être le même partout.
- Le SDK est un contrat public en semver (MPL-2.0, Q-ouv-1) ; une rupture est une
  version majeure.
