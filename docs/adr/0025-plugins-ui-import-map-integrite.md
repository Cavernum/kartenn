---
status: proposed
date: 2026-10-19
---

# 0025 — Shell servi par kartenn-server, plugins sous une import map rendue avec l'intégrité de chaque module

## Context

Les plugins UI sont des modules ES chargés à chaud qui enregistrent des Custom Elements
(D6, D17, ADR 0011), protégés par CSP et SRI. La session du navigateur est un cookie
`__Host-` en `SameSite=Strict`, la CSRF se contrôle par `Sec-Fetch-Site` (ADR 0020,
006/H10) : le shell doit être sur la même origine que l'API. Or SRI ne s'applique pas à
`import()` : un module importé n'est contrôlé que par la clé `integrity` d'une import
map, qui est un script en ligne, unique en pratique (plusieurs import maps manquent à
Firefox), figée avant le premier import ; un module absent de la table s'exécute sans
contrôle ([note](../reference/plateforme-web.md)). Décisions 007/W2, W3, W4, W7, W20,
W21.

## Decision

- kartenn-server sert le build du web (`[web] root`) et les plugins déposés par
  l'exploitant (`[web] plugins_dir`), depuis la mémoire, sur la même origine que `/v1`.
- Il rend `index.html` à chaque ensemble de plugins : une import map avec
  `@kartenn/sdk` et une entrée `integrity` (SHA-384) pour chaque module du shell et des
  plugins retenus, un bloc de données des plugins du document, et une CSP qui n'autorise
  que `'self'` et l'empreinte SHA-256 de l'import map (sans `unsafe-eval`,
  `'unsafe-inline'` ni `'strict-dynamic'`).
- Les plugins ne sont listés et servis qu'à une session authentifiée.
- Le registre est relu toutes les 30 s et sur `SIGHUP` ; un shell ouvert garde les
  plugins de son document.
- Un plugin est du code de confiance de l'exploitant : même document, même session,
  aucune isolation propre ; le serveur reste seul juge des droits (INV-23).

## Alternatives

- Import map figée au build : ne couvre pas les plugins déposés à chaud.
- `modulepreload` avec `integrity` : ne couvre pas les fragments importés par un plugin.
- Plugins en iframe sandboxée : contredit D17 (pont d'API, mise en page, thème).
- Serveur statique séparé : CSP et cache configurés hors de kartenn, même origine à
  garantir par le répartiteur.

## Consequences

- `index.html` n'est jamais `immutable` : `no-cache` et ETag par ensemble de plugins.
- Navigateurs requis : import maps avec `integrity` (Chromium 127, Firefox 138,
  Safari 18) ; plus ancien, le contrôle serait muet.
- Le CSS ne peut être en ligne (`style-src 'self'`) : le shell et les plugins livrent
  des fichiers CSS.
- Installer un plugin revient à lui confier la session de chaque utilisateur ; la
  documentation de l'exploitant le dit.
- D35 amendée : plus d'image « web statique ».
