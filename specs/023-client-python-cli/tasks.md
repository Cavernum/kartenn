---
status: draft
updated: 2026-10-23
---

# 023 — Bibliothèque cliente Python et CLI : tâches

Ce plan est un **brouillon**. Aucune tâche ne commence avant l'arbitrage de A1 à A8
([`decisions.md`](decisions.md)) et la validation de la [maquette](maquette.md). Chaque
tâche est un commit qui laisse le build vert (`k-py-check` à partir de la tâche 2, plus
`k-check` pour la tâche 3). Chaque tâche porte `dépend de :`. Les chemins
`clients/python/…` sont ceux du design (§ Disposition).

## P0 — Arbitrages et prototype

- [ ] 1. Prototype de démarrage et de dérivation (jetable, dans `docs/spikes/`), trois
  mesures :
  - temps de `--version` et de `--help` avec Click, httpx et rich importés
    paresseusement ;
  - temps de dérivation du catalogue depuis `specs/006-api-rest/openapi-v1.json` ;
  - taille du catalogue dérivé.
  Résultat consigné dans `docs/spikes/00N-cli-demarrage.md` — T12, T20, T24 — verify :
  chiffres comparés aux budgets de REQ-023-39
  - où : forme des prototypes `docs/spikes/001-gist-exclusions.md` ; contrat
    `specs/006-api-rest/openapi-v1.json` (279 opérations, `x-query`)
  - dépend de : arbitrages A1, A2

## P1 — Outillage et cœur de la bibliothèque

- [ ] 2. Espace de travail `clients/python/` :
  - uv, deux membres `kartenn` et `kartenn-cli`, `uv_build` épinglé ;
  - licence MPL-2.0 ;
  - Python et uv dans `devenv.nix`, script `k-py-check` (ruff, pyright, pytest ; verdict
    sur une ligne, comme `k-web-check`) ;
  - étape ajoutée à `scripts/ci.sh`.
  — T6, T7, T8 — verify : `k-py-check` vert sur un test vide ; `uv build` produit deux
  wheels `py3-none-any`
  - où : `devenv.nix` (`scripts.k-web-check`, l. ~456, à imiter) ; `web/sdk/package.json`
    (licence MPL-2.0) ; `scripts/ci.sh`
  - dépend de : 1, A3, A7
- [ ] 3. Amendements additifs du serveur (A5) :
  - ETag de `GET /v1/openapi.json` ;
  - extension `x-kartenn` dans `kartenn_rest::kit` (clé naturelle et colonnes des
    collections du noyau, de `vlan`, `machines`, `policy`, `flow`) ;
  - `GET /v1/auth/native-clients` et le champ `native_client_id` de
    `[[auth.issuers]]` ;
  - contrat de référence régénéré.
  — REQ-023-2, 7, 27 — verify : tests REST (304 sur `If-None-Match`, route publique sans
  secret, émetteur sans `native_client_id` absent) ; comparateur de contrat vert
  (ajouts seulement)
  - où : `crates/kartenn-rest` (route `openapi`, `kit`), `crates/kartenn-auth`
    (configuration des émetteurs, 005/design § configuration) ; amendement à écrire dans
    `specs/006-api-rest/` et `specs/005-authn-rbac/` ; tests voisins de
    `crates/kartenn-rest/tests/it/`
  - dépend de : A5
- [ ] 4. Catalogue : dérivation, modèle gelé, cache XDG par révision, mode 0600 et
  écriture atomique — REQ-023-1, 2, 3 — verify : instantané du catalogue de référence ;
  contrat sans le tag `policy` → aucune ressource `policy` ; module fictif `demo` dérivé
  sans code ; même révision → aucun téléchargement
  - où : `clients/python/kartenn/src/kartenn/catalog/` ; design § Catalogue (tableau de
    dérivation)
  - dépend de : 2
- [ ] 5. Cœur sans entrées-sorties et transport :
  - requêtes (dates, filtres, `QUERY` au-delà de 8 Kio) ;
  - problèmes → exceptions ;
  - pages et curseurs ;
  - port httpx synchrone et asynchrone, reprise sur 429 et 503, idempotence.
  — REQ-023-4, 5, 9, 10, 11, 14 — verify : tests `httpx.MockTransport` : `Retry-After`
  en secondes et en date, `POST` sans clé jamais rejoué, 3 pages suivies, membre
  d'extension inconnu conservé
  - où : `clients/python/kartenn/src/kartenn/_core/`, `transport.py`, `retry.py` ;
    006/design § Curseurs, § Filtres, § Erreurs
  - dépend de : 4
- [ ] 6. Ressources dynamiques et `Record` (ETag, liens), `update` en Merge Patch sous
  `If-Match`, `close` avec motif, `kn.call(operationId)`, `kn.at(...)` — REQ-023-12,
  16 — verify : 412 → `PreconditionFailed` avec l'ETag courant ; `close` sans motif →
  refus local
  - où : `clients/python/kartenn/src/kartenn/resources.py` ; 006/design § Écritures
    (arbitrage 006-16)
  - dépend de : 5
- [ ] 7. Changesets et imports :
  - `ChangesetBuilder`, simulation, soumission, actions, différentiel par sections ;
  - `ImportSession` : morceaux, état local, reprise par empreinte, rappel d'avancement.
  — REQ-023-13, 15 — verify : 202 → `Pending` ; reprise au morceau 41 ; 10⁶ éléments
  synthétiques en mémoire ≤ 100 Mio (transport simulé)
  - où : `clients/python/kartenn/src/kartenn/changesets.py`, `imports.py` ;
    006/design § Imports et tâches de fond ; 017/decisions Q-017-1 (sections de module)
  - dépend de : 6
- [ ] 8. Authentification :
  - Basic ;
  - Bearer fourni ;
  - OIDC natif (bouclage avec PKCE `S256`, appareil, rafraîchissement) ;
  - secours avec `Origin` ;
  - stockage au trousseau, sans repli implicite.
  Extension de `kartenn-dev-idp` (client public, `device_authorization_endpoint`,
  rotation des jetons de rafraîchissement). Note `docs/reference/` (RFC 8252, 8628,
  9700, 7636). — REQ-023-6, 7, 8, 40 — verify :
  - `state` falsifié refusé ;
  - `slow_down` allonge l'intervalle de 5 s ;
  - aucun secret dans les journaux (capture des logs) ;
  - Basic vers `http://` non bouclage refusé.
  - où : `clients/python/kartenn/src/kartenn/auth/` ; `crates/kartenn-dev-idp/src/server.rs`
    (`grant_types_supported`, l. ~183 ; échange, l. ~394) ; 005/design § Jetons Bearer
  - dépend de : 5, A4
- [ ] 9. Stubs : `kartenn stubs generate` (`TypedDict`, `Literal`, `Protocol`) —
  REQ-023-17 — verify : pyright signale un membre inconnu dans un script d'exemple ;
  aucun import des stubs à l'exécution
  - où : `clients/python/kartenn/src/kartenn/stubs.py`
  - dépend de : 6

## P2 — CLI

- [ ] 10. Racine paresseuse :
  - profils TOML (XDG), précédence option > environnement > profil ;
  - `config`, `login`, `logout`, `me`, `capabilities`, `health` ;
  - langue (catalogues `fr.toml`, `en.toml`) ;
  - garde d'import (`--version` n'importe ni httpx ni rich).
  — REQ-023-35, 36, 37, 39 — verify : test d'import ; `NO_COLOR` ; `--no-input` sans
  terminal ne lance ni navigateur ni question
  - où : `clients/python/kartenn-cli/src/kartenn_cli/main.py`, `config.py`, `i18n/` ;
    catalogues du web à imiter (`web/sdk/src/i18n.ts`)
  - dépend de : 8
- [ ] 11. Arbre dérivé : groupes de modules, nommage (retrait du préfixe), verbes, sous-
  ressources, module absent → code 2 et message ; aide tirée du contrat — REQ-023-18,
  21 — verify : instantané de `--help` de chaque groupe sur le contrat de référence ;
  contrat sans `policy` → message de la maquette § 12
  - où : `clients/python/kartenn-cli/src/kartenn_cli/tree.py` ; design § Nommage ;
    maquette § 2, 3, 12
  - dépend de : 10
- [ ] 12. Lectures : `--at` et `--as-of` (RFC 3339, date seule, relative), filtres
  typés et `--prop`, rendus `table`, `json`, `ndjson`, `yaml`, `csv`, `--fields`,
  `--query`, clés et libellés, références par clé naturelle — REQ-023-22, 23, 25, 26,
  27 — verify : instantanés des rendus de la maquette § 4 ; `ndjson --all` en mémoire
  constante ; filtre inconnu refusé sans requête
  - où : `clients/python/kartenn-cli/src/kartenn_cli/render/` ; maquette § 4
  - dépend de : 11
- [ ] 13. Complétion bash, zsh et fish : catalogue, énumérations, clés et valeurs de
  propriétés, slugs ; cache de 5 min sans valeur masquée ; délai d'une seconde au plus —
  REQ-023-24 — verify : scripts générés et testés dans les trois shells (pexpect) ;
  serveur muet → propositions locales en moins d'une seconde
  - où : `clients/python/kartenn-cli/src/kartenn_cli/complete.py` ; maquette § 5
  - dépend de : 12
- [ ] 14. Écritures :
  - `create`, `update`, `close`, `edit` (éditeur, fichier 0600, 412 → réouverture) ;
  - `--dry-run`, `--into-changeset` ;
  - `changeset new|add|simulate|apply|show|diff|list|approve|reject|withdraw|undo|proposals` ;
  - rendu des problèmes et codes de sortie.
  — REQ-023-28 à 31, 33, 34 — verify : un test par code de sortie du design ; rendu des
  sections de module (maquette § 8) ; fichier temporaire supprimé même sur erreur
  - où : `clients/python/kartenn-cli/src/kartenn_cli/edit.py`, `render/problem.py` ;
    design § Codes de sortie, § Rendu d'un problème
  - dépend de : 12
- [ ] 15. Import : NDJSON, CSV et JSON par ressource (`--resource`, `--id-column`),
  avancement sur terminal et par lignes, `--resume`, `--follow`, violations rapportées
  à la ligne source — REQ-023-32 — verify : maquette § 10 rejouée sur transport simulé ;
  `import.unknown-reference` affiché avec `fichier:ligne`
  - où : `clients/python/kartenn-cli/src/kartenn_cli/importing.py` ; schéma
    `ImportElement` du contrat
  - dépend de : 14
- [ ] 16. Commandes nommées : `policy check` (`--exit-status`), `policy matrix
  show|set`, `policy report`, `flow request|review|waive` ; table `named.py` liée aux
  `operationId`, masquée si une opération manque — REQ-023-20 — verify : maquette § 6 et
  7 sur transport simulé ; contrat sans `flow` → commandes absentes
  - où : `clients/python/kartenn-cli/src/kartenn_cli/named.py` ; design § Commandes
    nommées ; 010 et 011 (routes), 017/design § Requête de conformité
  - dépend de : 14
- [ ] 17. `kartenn api` : méthodes dont `QUERY`, `-f`, `--data @fichier|-`, `-H`,
  `--paginate` — REQ-023-19 — verify : maquette § 11
  - où : `clients/python/kartenn-cli/src/kartenn_cli/main.py`
  - dépend de : 12
- [ ] 18. Couverture : `coverage.py` (exclusions motivées) et test qui classe chaque
  opération du contrat de référence — REQ-023-43 — verify : une opération ajoutée au
  contrat de test, non classée → échec
  - où : design § Couverture de l'API
  - dépend de : 16, 17

## P3 — Bout en bout, budgets, livraison

- [ ] 19. Parcours de bout en bout contre `kartenn-server`, sous `k-heavy --pg` :
  - base `kartenn_cli`, compte de secours, données de la maquette ;
  - sessions de la maquette rejouées (UUID et dates normalisés) ;
  - connexion OIDC par bouclage et par appareil sur `kartenn-dev-idp`.
  Script `k-py-e2e`. — REQ-023-7, 8, 18, 32 — verify : `k-py-e2e` vert ; le serveur sans
  `policy` (build sans la feature) donne la maquette § 12
  - où : `web/e2e/serve.sh` et `web/e2e/global-setup.ts` (à imiter) ; `devenv.nix`
    (`scripts.k-web-e2e`, l. ~492)
  - dépend de : 3, 18
- [ ] 20. Banc `bench-cli` : budgets de REQ-023-39 (hyperfine ou boucle chronométrée,
  cache chaud et froid) ; résultats consignés dans le design — REQ-023-39 — verify :
  tous les budgets tenus, ou un écart documenté et arbitré
  - où : forme des bancs `crates/kartenn-bench` (consignation), `k-heavy`
  - dépend de : 19
- [ ] 21. Livraison : paquet Nix du flake (bibliothèque et CLI), complétions installées,
  page de manuel générée depuis l'aide ; `docs/spec/` synchronisée (interfaces : clients)
  — REQ-023-42 — verify : `nix build .#kartenn-cli` puis `kartenn --version` ;
  `devenv test` vert
  - où : paquets de la 015 (`specs/015-paquetage-deploiement/`, ADR 0029)
  - dépend de : 20

## Différé

- `kartenn watch` (REQ-023-38) : après la spec 022 (SSE), dans une tâche de la 022 ou
  d'un amendement de la 023.
- Publication sur PyPI et image de conteneur (Q-023-2, Q-023-14) : à la première version
  stable.
