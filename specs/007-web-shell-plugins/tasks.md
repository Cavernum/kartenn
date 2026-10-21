---
status: in-progress
updated: 2026-10-20
---

# 007 — Shell Web et plugins : tâches

Chaque tâche est un commit, laisse le build vert (`devenv test`) et nomme ce qu'elle
implémente. Côté Rust, `k-check` ; côté web, `k-web-check` (prettier, eslint, vue-tsc,
Vitest) et `k-web-e2e` (Playwright, Chromium et Firefox) dès la tâche 10. Les ajouts à l'API
(tâches 6 à 8) et les quotas de la tâche 3 suivent W18.

## P0 — Sources et socle

- [x] 1. Note de référence `docs/reference/web-navigateur.md` : CSP Level 3 (`script-src`
  et empreinte d'une import map, `style-src` face au CSSOM et aux attributs `style`,
  `form-action` et redirections), SRI (`integrity` de `<script type=module>`, de
  `modulepreload`, de `fetch`), import maps (HTML), Custom Elements et Shadow DOM,
  héritage des propriétés personnalisées, cookies `Secure` et `__Host-` sur
  `http://127.0.0.1` (RFC 6265bis, Chromium, Firefox), IndexedDB et `BroadcastChannel`,
  WCAG 2.2 AA et motifs `treeview` et `grid` de l'APG ; prise en charge par les
  navigateurs de REQ-007-37 ; vue-i18n sans `eval` — REQ-007-2, 6, 33, 36, 37 — verify :
  chaque affirmation cite URL et section, exigences normatives distinguées des choix de
  kartenn ; « Conventions » des exigences complétées par les sections ; les risques du
  design (cookie sur le bouclage, vue-i18n, `modulepreload`) levés ou reformulés
  où : `docs/reference/README.md` (conventions, index), modèle
  `docs/reference/api-http.md` ; agent `ref-researcher` ; requirements « Conventions »,
  design « Service du shell », « Chargement des plugins par le shell », « Risques »
  — Note `docs/reference/web-navigateur.md`, indexée dans `docs/reference/README.md` :
  pages rendues des normes injoignables, textes lus dans les sources de leurs dépôts,
  renvois par ancres recoupées dans ces sources (HTML, DOM, CSP3, SRI, Storage,
  IndexedDB, 6265bis, Secure Contexts) ; 27 propositions [N]/[T]. Requirements
  « Conventions » : ancres par source. Design « Risques » : cookie sur le bouclage levé
  (Secure Contexts, code de Chromium et Firefox), vue-i18n levé sous condition (build
  avec le compilateur de messages), `modulepreload` reformulé (whatwg/html#10327) ;
  « Alternatives » : rejet de l'`integrity` d'import map motivé par Firefox
  choix : étapes 3 et 4 du chargement ordonnées (script d'entrée après le `load` de tous
  les `modulepreload`, parade du risque reformulé) ; propositions [T] de la note laissées
  aux tâches qui touchent leur sujet
- [x] 2. Espace pnpm `web/` : `package.json`, `pnpm-workspace.yaml`, paquets `shell`
  (Vue, vue-router, page d'accueil vide), `sdk` (bibliothèque à deux points d'entrée,
  `SDK_VERSION`), `plugins/example` (vide), `e2e` (`@playwright/test` épinglé sur
  `playwright-driver`) ; ESLint (dont `vue/no-static-inline-styles`), Prettier, Vitest,
  cible `es2022` ; avis de licence et leur test ; relais Vite de `/v1` et `/plugins` vers
  `https://127.0.0.1:8443` (reporté de la tâche 9) — REQ-007-36, 37, 38 — verify :
  `k-web-check` vert ; un fichier de `web/sdk` sans avis fait échouer le test ;
  `devenv up` démarre `web` et `browser_snapshot` montre la page ; connexion par
  l'émetteur de développement à travers Vite
  où : design « Arborescence de `web/` », « Dépendances du frontend », « Licences » ;
  `devenv.nix` (`hasWeb`, `languages.javascript`, `processes.web`, `scripts.k-web-check`,
  `k-web-e2e`) ; ADR 0024 ; avis MPL de `crates/syslog-sender/src/lib.rs`
  — `web/` : `package.json` (pnpm 12.9, Node ≥ 24), `pnpm-workspace.yaml`,
  `tsconfig.base.json` (`es2022`, strict), `eslint.config.js`, `prettier.config.js`,
  `vitest.config.ts` ; `sdk` (`src/index.ts` avec `SDK_VERSION`, `src/host.ts`, build en
  bibliothèque à deux entrées), `shell` (`createShellRouter`, `HomeView`), `plugins/example`
  (vide), `e2e` (`@playwright/test` 1.63.0 = `playwright-driver`, projets `chromium` et
  `firefox`). Tests : `sdk/test/licenses.test.ts` (avis exigé dans `sdk` et `plugins`,
  fichier nu détecté, avis altéré refusé), `shell/src/router.test.ts` ; page vue par
  `browser_snapshot` sur `127.0.0.1:5173`.
  choix : TypeScript 6.0 (typescript-eslint n'accepte pas encore 7) ; projets Vitest déclarés
  dans `vitest.config.ts` plutôt que `vitest.workspace.ts` (déprécié) ; `eslint-config-prettier`
  et `@types/node` en dépendances de développement ; test des avis dans le paquet `sdk`

## P1 — Service par kartenn-server

- [x] 3. Section `[web]` et service du shell : module `kartenn-rest::web` (résolution des
  chemins, types, cache et 304, en-têtes, empreinte de l'import map relue au changement
  d'`index.html`), `fallback` du routeur, quotas limités à `/v1`, câblage dans `app::run`
  — REQ-007-1, 2, 3 — verify : `rest/tests/web.rs` (repli sur `index.html`, 404 sous
  `/v1` et pour un fichier à extension absent, `..`, `%2e%2e`, fichier caché, lien
  symbolique sortant, 405 et `Allow`, `HEAD`, `immutable` sous `/assets`, `no-cache`,
  `ETag` et 304, CSP avec l'empreinte, `nosniff`) ; configuration : champ inconnu,
  `root` sans `index.html`, `index.html` sans import map refusés ; un shell de 60 fichiers
  chargé sans authentification ne reçoit pas 429
  où : design « Service du shell », « Configuration » ; `kartenn-rest/src/lib.rs`
  (`router`, `.fallback(not_found)`), `authn.rs` (`public`), `quota.rs` (`layer`) ;
  `kartenn-server/src/config.rs` (`Config`, `Http`, tests en fin de fichier),
  `app.rs` (`kartenn_rest::router`, vers la ligne 718) ; tests à la manière de
  `rest/tests/socle.rs` ; `dev/kartenn.toml` (ajouter `[web] plugins_dir`, reporté de
  la tâche 9 ; test `the_development_configuration_is_valid` de
  `kartenn-server/tests/dev_idp.rs`)
  — `kartenn-rest/src/web.rs` (`Web`, `Shell`, `serve`), `router_with_web` dans
  `lib.rs`, `quota::layer` limité à `/v1` ; `[web]` dans `kartenn-server/src/config.rs`
  (`Web`, refus au chargement par `Shell::open`) et `app.rs` (`Api::start`) ; tests
  `rest/tests/web.rs` (9), `config::tests::a_faulty_web_section_…`, routes de
  `rest/tests/quota.rs` passées sous `/v1`
  choix : `router` garde sa signature et délègue à `router_with_web` (appelants de test
  inchangés) ; 404 du shell en `text/plain`, 404 problème sous `/v1` et `/health` ;
  `index.html` relu au changement de taille ou de date à chaque requête, l'ancien gardé
  si le nouveau n'a pas d'import map ; dépendances `arc-swap`, `base64`,
  `percent-encoding` (déjà dans l'arbre) ajoutées à `kartenn-rest`
- [x] 4. Registre des plugins : lecture de `plugins_dir`, validation du manifeste,
  empreintes `sha384` (rehachées au seul changement de taille ou de date), version
  publiée, filtre par module, service sous `/plugins/`, relecture toutes les 10 s et sur
  `SIGHUP`, journaux `web.plugin.*` — REQ-007-4, 5, 6, 8, 11 — verify : unitaires du
  manifeste, un par motif de refus ; intégration sur un répertoire temporaire : 1.0.0 et
  1.1.0 → 1.1.0 publiée et 1.0.0 servie, dépôt visible en ≤ 10 s et dès `SIGHUP`,
  retrait, plugin invalide écarté et les autres servis, empreinte égale à
  `openssl dgst -sha384 -binary | base64`, module absent → non publié, limites de taille
  et de nombre
  où : design « Plugins côté serveur » ; `kartenn-rest/src/web.rs` (tâche 3) ;
  `kartenn-server/src/app.rs` (`hangup`, lignes 648 à 682) et `http.rs` (certificat relu
  sur `SIGHUP`) à imiter ; `arc-swap`, `sha2`, `base64` de l'espace de travail
  — `kartenn-rest/src/web/plugins.rs` (`Plugins` : `open`, `reload`, `watch`,
  `published`, `RELOAD_EVERY`, `MAX_FILES`, `MAX_BYTES`), service sous `/plugins/` dans
  `web.rs` (`ETag` = empreinte SRI, 304) ; `capabilities::module_names` ; serveur :
  `serve_web` et relecture sur `SIGHUP` (signal installé avec TLS ou plugins) dans
  `app.rs`, `plugins_dir` illisible refusé au chargement de la configuration ; tests
  `rest/tests/plugins.rs` (9), unitaires du module (formes de `sdk`, noms, clés) et
  `server/tests/http.rs` (`a_plugin_deposited_is_served_at_the_next_sighup`)
  choix : dépendance `semver` (déjà dans l'arbre) pour l'ordre des versions ; clés des
  catalogues plates (`"tab.notes"`) ou imbriquées ; `label` et `nav` facultatifs,
  vérifiés s'ils sont présents ; plugin d'un module absent journalisé comme
  `web.plugin.rejected` ; empreinte de « `abc` » tirée de FIPS 180-2 (pas d'`openssl`
  dans devenv) ; le délai de 10 s éprouvé par `watch` à 100 ms et la constante
- [x] 5. `ui_plugins` enrichi dans les capacités et contrat de référence mis à jour —
  REQ-007-7 — verify : comparateur du contrat vert (membres ajoutés seulement) ;
  `GET /v1/capabilities` sur un répertoire de test publie base, entrée, empreintes,
  plage, extensions et catalogues
  où : `kartenn-rest/src/capabilities.rs` (`UiPlugin`, `capabilities`), `contract.rs`
  (`breaches`), `specs/006-api-rest/openapi-v1.json`, binaire `openapi` ;
  `rest/tests/contract.rs` (`the_contract_keeps_its_reference`)
  — `UiPlugin` (`sdk`, `base`, `entry`, `integrity`, `catalogs`, `extensions`) rempli
  depuis `Plugins::published` (le registre passe aux routes en `Extension<Arc<Web>>`) ;
  schémas `UiExtension` et `UiExtensionPoint` ; `openapi-v1.json` réécrit (membres
  ajoutés seulement) ; test `contract.rs`
  (`the_capabilities_publish_the_plugins_of_the_directory`)
  choix : noms de schéma préfixés `Ui` (`Extension` seul aurait été ambigu dans le
  contrat)

## P2 — Ajouts à l'API

- [x] 6. Racines de l'arbre d'un domaine, `GET /v1/domains/{id}/roots` (W18) —
  REQ-007-39 — verify : mêmes éléments, ordre et curseurs que les enfants d'un préfixe ;
  lecture datée ; nœuds illisibles absents ; page de 100 sous 150 ms au p95 sur le jeu de
  `bench-core` (agent `bench-runner`)
  où : design « Ajouts à l'API » ; `kartenn-store-pg/src/tree.rs` (`children_of`,
  `ChildKey`, `ReadScope`), `kartenn-rest/src/tree.rs` (`children`, `listed`) ;
  `rest/tests/objects.rs` (`children_and_history_page_by_cursor`,
  `the_tree_shows_only_what_the_caller_may_read`) à imiter ; contrat de référence
  — Store : `tree.rs` (`roots_of` : préfixes racines parcourus d'une sonde d'index à
  l'autre dans l'ordre de `cidr`, arrêt à une racine lisible de plus que la page ; plages
  et adresses lues dans les intervalles entre deux racines, bornes comparables pour
  l'index `keyset` ; `domain_exists`, `node_of` partagé avec `children_of`). Rest :
  `tree.rs` (`roots`, domaine illisible ou absent à la date → 404). Banc `bench-core` :
  « roots of a domain, page of 100, p95 » (150 ms). Test
  `the_roots_of_a_domain_list_as_the_children_of_a_prefix` (pages, curseur figé, parcours
  un par un, dates, droits partiels, racines illisibles qui bornent les intervalles).
  Contrat de référence. Mesure sur le jeu de `bench-core` (10⁶ objets, `psql`) : 1 à 6 ms
  la première page de 100 ; une première version par anti-jointure prenait 25 s
  choix : parcours des racines plutôt qu'anti-jointure sur tout le domaine ; lecture
  datée dans le passé correcte mais sans budget (l'instantané d'historique n'a pas
  d'ordre d'index) ; mesure faite à la main (le banc complet, ~50 min, n'a été relancé
  qu'avant la correction)
- [x] 7. Imports de l'appelant, `GET /v1/imports` (W18) — REQ-007-40 — verify : du
  plus récent au plus ancien, paginé ; ceux d'un autre absents
  où : design « Ajouts à l'API » ; `kartenn-store-pg/src/imports.rs` (`header`),
  `kartenn-rest/src/imports.rs` (contrôle de l'auteur, vers la ligne 114) ;
  `rest/tests/imports.rs` (`an_import_is_its_authors`) ; contrat de référence
  — Store : `imports.rs` (`of_author`, clé `(created_at, id)` décroissante). Rest :
  `imports.rs` (`list` : liste à l'état courant, ni filtre ni date, curseur
  `next_current` ; chaque élément est la représentation de `GET /v1/imports/{id}`).
  Test `the_imports_of_the_caller_list_the_most_recent_first`. Contrat de référence
  choix : élément de liste identique à la fiche d'un import (état, lots, rapport), lu
  import par import ; pas d'index sur `import.author_id` (table de quelques imports par
  auteur) ; lecture non auditée, comme `GET /v1/imports/{id}`
- [x] 8. Émetteurs de connexion, `GET /v1/auth/issuers` publique (W18) —
  REQ-007-41 — verify : 200 sans session, `[{id}]` dans l'ordre de la configuration,
  ni URL ni client ; route dans `PUBLIC`
  où : design « Ajouts à l'API » ; `kartenn-rest/src/auth.rs` (`routes`), `openapi.rs`
  (`PUBLIC`), `authn.rs` (`public`, trait `Authenticator`) ; `kartenn-server/src/config.rs`
  (`Issuer::id`) ; `rest/tests/sessions.rs` ; contrat de référence
  — Auth : `tokens.rs` (`TokenVerifier::issuer_ids`). Rest : `authn.rs` (méthode
  `Authenticator::issuers`, vide par défaut ; `PUBLIC`), `auth.rs` (`issuers`,
  `IssuerDto`), `openapi.rs` (`PUBLIC`). Test `the_issuers_of_the_login_are_public`
  (deux émetteurs, sans session ni TLS ; fixture `Fixture::of_issuers`). Contrat de
  référence
  choix : identifiants lus dans le vérificateur de jetons (même liste que la connexion)
  plutôt que dans la configuration du serveur ; route servie aussi sans TLS (aucun cookie
  posé)

## P3 — Émetteur de développement et bout en bout

- [x] 9. Crate `kartenn-dev-idp` (`publish = false`) : `certs` (autorité et certificats
  de développement), `serve` (découverte, JWKS, `authorize` avec page de choix, PKCE,
  `token`, `end-session`, utilisateurs, groupes et erreurs en TOML, bouclage seul) ;
  processus devenv `dev-idp` et `kartenn` (`dev/kartenn.toml`), relais Vite de `/v1` et
  `/plugins` — REQ-007-35, 36 — verify : intégration : flux complet de kartenn-server
  contre l'émetteur sur des ports locaux, groupes du TOML dans `/v1/me`, utilisateur en
  erreur → `authn.login.failed`, écoute hors bouclage refusée ; `devenv up` : connexion à
  travers Vite puis lecture d'une liste (`browser_snapshot`)
  où : design « Bout en bout et développement » ; `crates/kartenn-rest/tests/sessions.rs`
  (`Idp`, `discovery`, `jwks`, `token`, lignes 54 à 160) ; `kartenn-auth/src/http.rs`
  (`allowed`, racines natives) ; `kartenn-server/src/config.rs` (`Issuer`) ;
  `devenv.nix` (`processes.openobserve` et `processes.web` à imiter, commentaire `/api`) ;
  `kartenn-workspace-hack`
  — `crates/kartenn-dev-idp` (`certs`, `config`, `server`, binaire) ; `dev/idp.toml`,
  `dev/kartenn.toml`, `dev/state/` ignoré ; processus devenv `dev-idp` et `kartenn`.
  Tests : `crates/kartenn-server/tests/dev_idp.rs` (instance lancée par son binaire,
  `SSL_CERT_FILE` sur l'autorité de développement, page de choix et formulaire joués,
  groupes du TOML dans `/v1/me`, `refuse` → `authn.login.failed` ; configurations de
  `dev/` valides), tests unitaires du crate (écoute hors bouclage refusée, codes d'erreur,
  matériel écrit une fois). Le relais Vite de `/v1` et `/plugins` et la vérification
  `devenv up` à travers Vite passent à la tâche 2 (`web/` absent) ; `[web] plugins_dir`
  de `dev/kartenn.toml` à la tâche 3 (section encore refusée)
  choix : `certs` écrit aussi le secret du client et le trousseau de kartenn-server ; un
  seul certificat du bouclage partagé par l'émetteur et kartenn-server ; `cert_file` et
  `key_file` dans la configuration de l'émetteur ; jeton d'accès opaque ; jeton de
  rafraîchissement renouvelé à chaque usage ; test de bout en bout dans kartenn-server
  (binaire lancé en sous-processus, seul moyen de lui passer `SSL_CERT_FILE` sans `unsafe`)
- [x] 10. Harnais de bout en bout `web/e2e` : projets `chromium` et `firefox`, `webServer`
  (émetteur, kartenn-server sur `web/e2e/kartenn.toml`), `global-setup.ts` (base
  `kartenn_e2e`, `migrate`, compte de secours, liaisons des groupes, jeu de données par
  changesets), fixtures de connexion, `expectAccessible`, échec sur violation CSP ;
  parcours de fumée — REQ-007-33, 34 — verify : `k-web-e2e` vert sur les deux
  navigateurs (connexion, lecture, déconnexion) ; une violation axe injectée et un script
  en ligne injecté font échouer le parcours
  où : design « Bout en bout et développement », « Thème, composants, arbre »
  (accessibilité) ; `devenv.nix` (`scripts.k-web-e2e`) ; `kartenn-server/src/main.rs`
  (`migrate`, `breakglass create`)
  — `web/e2e/` : `playwright.config.ts` (deux `webServer` sur `serve.sh idp` puis
  `serve.sh kartenn`, ports 9444 et 9443), `serve.sh`, `global-setup.ts`, `users.toml`,
  `kartenn.toml`, `fixtures/` (`env`, `data`, `totp`, `index` : `login`,
  `expectAccessible`, garde CSP, exceptions et erreurs de console), `plugins/` (vide),
  `journeys/smoke.spec.ts` ; `@axe-core/playwright` ; build du shell vers `web/dist/shell`
  et import map vide dans `index.html` ; `k-web-e2e` lancé depuis le paquet `e2e`.
  Tests : `k-web-e2e` vert, 6 parcours (connexion, lecture de `/v1/me` et `/v1/domains`
  depuis la page, déconnexion jusqu'à l'émetteur puis 401 ; une image sans `alt` et un
  script en ligne injectés, attendus en échec, échouent bien par axe-core et par la garde
  CSP)
  choix : préparation (certificats, base `kartenn_e2e`, `migrate`, compte de secours) dans
  `serve.sh`, car Playwright lance les `webServer` avant `global-setup.ts` ; la liaison
  `administrator` du compte de secours écrite en SQL par `serve.sh`, aucune opération de
  l'API ne liant le premier administrateur, tout le reste (rattachement de l'émetteur,
  liaisons des groupes, jeu de données) par l'API ; erreurs de console relevées sur les
  pages de kartenn seulement (la page de l'émetteur n'est pas sous test)

## P4 — SDK

- [x] 11. Client de l'API : `scripts/gen.mjs` (`schema.d.ts`, `dated.ts`) et test de
  fraîcheur ; `api.get`, `query`, `pages`, `post`, `patch`, `delete`, `ApiProblem`,
  branchement du 401, paramètres datés du contexte — REQ-007-15, 24 — verify : Vitest
  avec `fetch` simulé : méthode `QUERY` et son corps, `If-Match` exigé par le type
  (test de type), clé d'idempotence générée ou reprise, 412 et 422 typés avec
  `Request-Id`, pages suivies jusqu'à la dernière, `valid_at` ajouté sur un chemin daté
  seulement ; contrat modifié sans régénération → test en échec
  où : design « SDK » (Client) ; `specs/006-api-rest/openapi-v1.json` ; 006 design
  « Curseurs », « Écritures », « Erreurs » ; `web/sdk/src/api/`
  — `web/sdk/scripts/gen.mjs` (+ `gen.d.mts`) ; `src/api/schema.d.ts`, `dated.ts`
  (générés), `client.ts` (`createClient`, types d'options tirés de `paths`), `problem.ts`
  (`ApiProblem.from`) ; `src/hooks.ts` (temps et 401 branchés par le shell),
  `host.ts` (`onUnauthorized`), `index.ts` (`api`). Tests : `sdk/test/client.test.ts`
  (QUERY, If-Match et ETag, `@ts-expect-error` sur `ifMatch` absent, clé d'idempotence,
  412 et 422, 401, pages, `valid_at` daté seulement, fraîcheur et contrat modifié).
  choix : routes `QUERY` (extension `x-query` du contrat, ignorée par openapi-typescript)
  ajoutées à `schema.d.ts` en interface `queries` ; fichiers générés formatés par Prettier ;
  `PATCH` en `application/merge-patch+json` ; `ifMatch` exigé là où le contrat déclare
  `If-Match` obligatoire ; `pages` suit `next` tel quel (le curseur porte filtres et dates) ;
  `put` ajouté pour les morceaux d'import ; dépendances de développement `openapi-typescript`
  (TypeScript 6 admis par `peerDependencyRules`) et `happy-dom` (tâche 12)
- [x] 12. Contexte, `definePlugin`, semver, i18n de plugin, `styles()`, brouillon (surface),
  jetons, `@kartenn/sdk/host`, bac à sable `@kartenn/sdk/testing` — REQ-007-12, 13, 14,
  16 — verify : Vitest : notification du contexte, élément non déclaré ou déjà défini
  refusé, matrice semver (`^`, `~`, exact, prépublications), `t` préfixé, bac à sable qui
  monte un élément avec contexte et `fetch` simulé ; deux importateurs voient le même
  contexte
  où : design « SDK », « Chargement des plugins par le shell » (étape 4) ;
  `web/sdk/src/` (tâche 2)
  — `web/sdk/src/` : `context.ts` (`context`, `setContext`), `extensions.ts`
  (`definePlugin`, `expectPlugin`, `abandonPlugin`, `PluginHandle` avec `t`, `format`,
  `styles`), `semver.ts` (`satisfies`, `compare`), `i18n.ts` (traducteur du shell ou
  catalogues), `draft.ts` (`draft`, `draftHost`), `tokens.ts`, `host.ts` (branchements),
  `testing/index.ts` (`mount`, `fakeFetch` typé par les routes du contrat,
  `announcePlugin`) ; troisième entrée `@kartenn/sdk/testing` (package et build). Tests :
  `sdk/test/semver.test.ts` (matrice `^`, `~`, exact, prépublications, ordre §11),
  `sdk/test/plugins.test.ts` (happy-dom : contexte notifié et partagé, permissions,
  élément non déclaré, déjà défini, plugin non annoncé ou abandonné, `t` préfixé et
  traducteur du shell, bac à sable avec `valid_at` du contexte, brouillon, jetons).
  choix : prépublications à la manière des plages npm (une prépublication ne satisfait
  qu'une plage qui nomme une prépublication du même `x.y.z`) ; `definePlugin` définit tous
  les éléments ou aucun ; le traducteur par défaut (catalogues ajoutés, `{param}`) sert au
  bac à sable, le shell branche vue-i18n par `setTranslator` ; le brouillon du SDK est une
  liste en mémoire que le shell synchronise (`draftHost`) ; `User.has` est fourni par le
  shell ; `fetch` du client branché par `host.setFetch` ; jetons `--k-*` nommés ici,
  définis par `tokens.css` à la tâche 13

## P5 — Shell

- [x] 13. Cadre du shell : mise en page, navigation, `vue-router`, jetons de design
  clair et sombre, Reka UI, vue-i18n FR et EN, choix de la langue, traduction des
  problèmes, droits par `/v1/me`, page `/about` — REQ-007-20, 29, 30 — verify : Vitest :
  parité des clés, ordre du choix de langue, problème sans traduction → `detail`,
  contrastes des jetons dans les deux modes, couverture des motifs de permission ;
  E2E : un lecteur ne voit pas l'entrée « Administration »
  où : design « i18n », « Thème, composants, arbre », « Écrans et routes du client »
  (droits) ; 006 design « Erreurs » (URN des problèmes) ; `kartenn-rest/src/auth.rs`
  (`GET /v1/me`)
  — `web/shell/src/` : `app/AppShell.vue` (lien d'évitement, en-tête, navigation,
  `main`), `app/UserMenu.vue` (menu Reka UI : langue, thème, déconnexion par formulaire
  `POST`), `navigation.ts` (entrées filtrées par droits), `rights/` (`covers`, `Rights`,
  `rightsOf`, `loadSession` qui pousse l'utilisateur au contexte du SDK), `i18n/`
  (`fr.json`, `en.json`, `chooseLocale`, `problemText`, `createShellI18n` branché sur
  `setTranslator`), `theme.ts`, `styles/{tokens,base}.css`, vues `AdminView`,
  `AboutView`, `NotFoundView` ; dépendances `vue-i18n` 11, `reka-ui`, `@kartenn/sdk`.
  Tests : Vitest `i18n.test.ts` (parité du shell et des plugins, messages compilés, ordre
  de la langue, problème par `type` puis `detail` puis `title`), `tokens.test.ts`
  (contrastes des deux modes, mode sombre du système identique au choisi, jetons du SDK
  définis), `permissions.test.ts` (`*`, entrée exacte, `prop.*.<action>` sur clés à
  points, permissions de la navigation présentes dans `docs/spec/permissions.md`, rôles
  des liaisons portées lus) ; E2E `journeys/shell.spec.ts` (un lecteur ne voit pas
  « Administration », un administrateur l'atteint, langue et thème choisis et gardés au
  rechargement, page « À propos »), axe-core à chaque étape
  choix : vue-i18n sans `@intlify/unplugin-vue-i18n`, drapeaux définis dans
  `vite.config.ts` (compilation JIT gardée, sans `eval`, éprouvée par la garde CSP des
  parcours) ; navigation limitée aux écrans existants, chaque tâche d'écran ajoute son
  entrée ; « Administration » visible avec `core.rbac.read`, `core.rbac.admin` ou
  `core.audit.read` ; entrées des rôles à portée lues par `GET /v1/roles/{key}`, cachées
  si ce droit manque ; menu de l'utilisateur non modal (un menu modal masque la page par
  `aria-hidden` en y laissant des liens focalisables) et nom accessible qui commence par
  le texte visible (WCAG 2.5.3)
- [x] 14. Session : page `/login` (émetteurs, secours), URL de retour, reconnexion sur
  401, déconnexion par formulaire — REQ-007-28 — verify : E2E : session expirée sur une
  fiche → reconnexion → même fiche et même contexte temporel ; déconnexion → retour à
  l'émetteur, préférences sensibles effacées
  où : design « Session » ; 006 design « Authentification et sessions » ;
  `kartenn-rest/src/auth.rs` (`callback` vers `/`, `logout`)
  — `web/shell/src/session/` : `index.ts` (`internalPath` : chemin interne seulement,
  ni `//`, ni `/\`, ni `/login`, ni `/v1/` ; `rememberReturn`/`takeReturn` en
  `sessionStorage`, lue une fois ; `loginAt` garde l'émetteur en `localStorage` ;
  `clearSensitive` : toute clé `kartenn.*` sauf langue, thème et émetteur, clés de
  l'onglet, bases IndexedDB `kartenn*` ; `shellFetch` et `onUnauthorized` du SDK ouvrent
  le dialogue ; `installSession` : la première navigation attend `/v1/me`, sans session
  → `/login?return=…` hors `login` et `about`, puis retour à l'URL gardée),
  `SessionDialog.vue` (`alertdialog` « Session expirée », « Se reconnecter » vers
  l'émetteur gardé, sinon `/login`) ; `views/LoginView.vue` (émetteurs de
  `GET /v1/auth/issuers`, formulaire de secours, problème traduit) ; route `login` ;
  `main.ts` installe la session avant la première navigation ; déconnexion du menu
  précédée de `clearSensitive` ; catalogues `login.*`, `session.*`. Tests : Vitest
  `session/session.test.ts` (chemins refusés, retour lu une fois, émetteur gardé,
  effacement sélectif, garde sans session puis retour après connexion) ; E2E
  `journeys/session.spec.ts` (cookie retiré sur une fiche datée → `/login` → émetteur →
  même fiche, même `valid_at` ; 401 en cours d'usage → dialogue → émetteur gardé → même
  fiche ; déconnexion → `end-session` de l'émetteur, langue et émetteur gardés, recherches
  récentes et URL de retour effacées), axe-core
  choix : sans session, toute page sauf `/login` et `/about` renvoie à la connexion (l'API
  n'a pas de lecture anonyme) ; le formulaire de secours n'a pas de parcours E2E (un code
  TOTP n'est accepté qu'une fois par pas de 30 s, déjà pris par `global-setup.ts`)
- [x] 15. Chargement des plugins par le shell et hôtes des quatre points d'extension,
  filtres par type et permission — REQ-007-9, 10, 13 — verify : E2E avec les plugins de
  test de `web/e2e/plugins/` : compatible actif ; `sdk: "^9"`, empreinte fausse,
  exception au chargement, `definePlugin` absent → écartés avec leur motif sur `/about`,
  shell utilisable ; `fetch` tiers bloqué ; onglet absent sur un autre type ; action
  absente sans la permission
  où : design « Chargement des plugins par le shell » ; `web/shell/src/plugins/` ; SDK
  `definePlugin` et `host` (tâche 12)
  — `web/shell/vite.config.ts` (`sharedSdk` : `@kartenn/sdk` et `/host` externes, SDK
  construit une fois sous `dist/shell/sdk/`, import map de `index.html` réécrite ; sous
  Vite, la carte nomme les modules servis au shell) ; `web/shell/src/plugins/` :
  `loader.ts` (`readPlugins`, `loadPlugins` : plage du SDK, catalogues par `fetch` avec
  empreinte fusionnés sous le nom du plugin, `modulepreload` avec empreinte, entrée,
  exception venue de `base`, délai de 5 s), `registry.ts` (`usePlugins`,
  `extensionsOf` par point, type et permission), `ExtensionElement.vue` (hôte : crée
  l'élément, `object` et `row`, encart d'erreur, `kartenn-close`), `PluginView.vue`
  (`/x/<plugin>/<id>/*`) ; `views/ObjectView.vue` (fiche `/:collection/:id` : onglets
  Reka UI, panneaux latéraux, menu d'actions ouvert en dialogue), « Extensions » dans la
  navigation, plugins et motifs sur `/about` ; plugins de test `web/e2e/plugins/`
  (`notes`, `incompatible`, `throws`, `silent`, `forged`). Tests : Vitest
  `registry.test.ts`, `router.test.ts` ; E2E `journeys/plugins.spec.ts` (motifs sur
  `/about` et shell utilisable, empreinte forgée dans les capacités, vue de plugin et
  `connect-src` bloqué, onglet et panneau selon le type, action absente sans
  `core.address.write`, action en dialogue fermée par son élément), axe-core
  choix : fiche minimale (attributs scalaires de la ressource) en attendant la tâche 17,
  qui placera aussi l'hôte de colonne (`ExtensionElement` avec `row`) dans les listes ;
  catalogues chargés pour les deux langues du shell (changer de langue sans recharger) ;
  `router.onError` en avertissement (une vue paresseuse coupée par le départ de la page
  n'est plus une erreur non rattrapée) ; lectures du shell tolérantes aux requêtes
  coupées par une navigation ; parcours : erreurs du plugin `throws` et modules coupés
  par le départ de la page (empreinte du contenu vide sous Firefox) tolérés, tolérances
  propres à un parcours par `test.use`
- [x] 16. Contexte temporel : `useTime`, garde de navigation, bandeau, écritures
  désactivées, contexte poussé au SDK — REQ-007-22, 23, 24 — verify : Vitest : `useTime`
  et la garde ; E2E : lien avec `valid_at` et `as_of` ouvert dans un second contexte de
  navigateur → mêmes valeurs ; bandeau → présent ; sous `as_of`, boutons d'écriture
  désactivés
  où : design « Contexte temporel » ; 006 design « Lectures datées et cache »
  — `web/shell/src/time/` : `index.ts` (`timeOf` : instants RFC 3339 UTC, une date
  malformée vaut absente ; `useTime`, `useSetTime`, `datedSearch`, `carryTime` : garde
  qui recopie les dates sur une navigation qui n'en nomme pas, sauf retour explicite par
  `toPresent` ; `installTime` : garde et `setContext` du SDK après chaque navigation ;
  `writesOf`/`useWrites` : rien sous `as_of`, brouillon seul sous `valid_at`),
  `TimeBanner.vue` (`role="status"`, dates en `<time datetime>`, fuseau, « Revenir au
  présent ») au-dessus de chaque page ; `installTime` dans `createShellRouter` ; fiche
  lue aux dates du contexte, actions des plugins désactivées sous `as_of`. Tests :
  Vitest `time/time.test.ts` (lecture et dates refusées, paramètres de lecture, garde,
  lien qui nomme ses dates, retour au présent, contexte poussé au SDK, écritures) ; E2E
  `journeys/time.spec.ts` (bandeau, dates gardées à la navigation, même lien dans un
  second contexte de navigateur avec une autre session, retour au présent ; actions
  désactivées sous `as_of`, permises sous `valid_at`), axe-core
  choix : les actions des plugins comptent comme écritures (seul bouton d'écriture
  existant avant la modification rapide de la tâche 19) ; dates affichées par `Intl` dans
  la langue du shell
- [x] 17. Listes et fiches du noyau : collections filtrées et virtualisées, recherche
  globale, fiche (attributs, propriétés résolues et explication, historique, anomalies et
  acquittement), occupation et prochain libre, déclarations et écarts de propriétés —
  REQ-007-17, 31 — verify : E2E sur le jeu de données : domaine → préfixe → adresse,
  propriété héritée expliquée, historique ; curseur refusé → première page ; liste de
  1 000 lignes avec quelques dizaines de lignes dans le DOM
  où : design « Écrans et routes du client » ; 006 design « Ressources et routes » ;
  `web/shell/src/views/`
  — `web/shell/src/data/` : `collections.ts` (collections, filtres et colonnes du
  contrat, noms et champs d'un objet, `pageOf` d'une URL de l'API vers une page),
  `api.ts` (`read`/`write` par `shellFetch`, problème gardé ; `Pager` : pages de 100 par
  `links.next`, curseur jamais dans l'URL du shell, `invalid-cursor` → première page),
  `DataTable.vue` (`<table>` natif virtualisé par `@tanstack/vue-virtual` à hauteur fixe,
  `aria-rowcount` à -1 tant que la liste n'est pas lue, `aria-rowindex`, une ligne
  focalisable à la fois, flèches, Origine, Fin, page, Entrée ouvre la fiche, page
  suivante demandée 20 lignes avant la fin), `recent.ts` (recherches récentes, effacées
  à la déconnexion) ; `views/ListView.vue` (`/:collection`, filtres dans l'URL),
  `ObjectView.vue` (attributs et liens des résumés ; onglets Contenu par `roots` ou
  `children`, Propriétés, Historique, Anomalies, puis ceux des plugins),
  `record/` (`PropertiesPanel` : valeurs résolues, explication à la demande, nœuds
  nommés par la lecture de leur fiche ; `HistoryPanel` ; `AnomaliesPanel` : acquittement
  avec motif, coupé hors du présent ; `ContentPanel` ; `OccupancyPanel` : occupation et
  prochain libre), `AnomaliesView.vue`, `PropertiesView.vue` (déclarations, écarts),
  `SearchView.vue` et `app/SearchBox.vue` (adresse ou préfixe, une seule réponse ouvre
  sa fiche) ; routes et navigation ; catalogues `field.*`, `list.*`, `record.*`,
  `anomalies.*`, `properties.*`, `search.*`. Jeu de bout en bout : réseau `10.2.0.0/22` et
  ses 1 000 adresses (`fixtures/data.ts`, `global-setup.ts`). Tests : Vitest
  `data/data.test.ts` (pages suivies, curseur refusé → reprise, autre problème gardé,
  paramètres, noms, résumés, `pageOf`, valeurs) ; E2E `journeys/records.spec.ts`
  (domaine → Contenu → préfixe → préfixe → adresse, `core.owner` expliqué jusqu'au
  domaine, historique ; 1 001 adresses lues au défilement, moins de 100 lignes dans le
  DOM, `aria-rowcount`, Origine, flèche et Entrée ; curseur refusé simulé →
  « la liste repart du début »), axe-core
  choix : dépendance `@tanstack/vue-virtual` (W12) ; anomalies d'une fiche lues par son
  domaine (ou son entité) puis filtrées par objet, l'API n'ayant pas de filtre par
  objet ; l'acquittement suit les écritures rapides (coupé sous `valid_at` comme sous
  `as_of`) ; « Domaines » d'abord dans la navigation, l'arbre restant l'accueil
  (tâche 18)
- [x] 18. Arbre des préfixes : racines du domaine, enfants à la demande, virtualisation,
  clavier `treeview` — REQ-007-17, 31, 33 — verify : E2E : ouverture, page suivante
  chargée en défilant, flèches, Origine, Fin, Entrée ; conteneur à 10⁴ enfants du jeu de
  bout en bout : quelques centaines de nœuds au plus dans le DOM (10⁵ au banc, tâche 24)
  où : design « Thème, composants, arbre » (arbre) ; route de la tâche 6 ;
  `kartenn-rest/src/tree.rs` (`children`)
  — `web/shell/src/tree/model.ts` (modèle aplati des lignes visibles, branches lues page
  par page, ligne « suite » tant qu'il reste des pages, `aria-setsize` à -1 jusqu'à la
  dernière, pages d'une remise à zéro écartées ; `moveFocus` : touches du motif
  `treeview`), `tree/PrefixTree.vue` (`role="tree"` focalisable, `aria-activedescendant`,
  lignes `treeitem` à `aria-level`, `aria-posinset`, `aria-expanded`, virtualisées par
  `@tanstack/vue-virtual` à 32 px ; un `Pager` de `data/api.ts` par parent ; la ligne
  « suite » qui entre dans la fenêtre lit la page suivante ; Entrée ouvre la fiche ou
  relit une page en échec), `views/HomeView.vue` (domaine choisi dans l'URL, `?domain=`,
  le premier par défaut) ; catalogues `home.*`, `tree.*` ; `select` aux couleurs des
  jetons (contraste en mode sombre). Jeu de bout en bout : conteneur `10.3.0.0/16` et ses
  10⁴ réseaux `/30`. Tests : Vitest `tree/model.test.ts` (pages, ouverture, fermeture,
  remise à zéro, échec puis reprise, touches) ; E2E `journeys/tree.spec.ts` (Chromium et
  Firefox : droite, droite, bas, Fin et défilement jusqu'au-delà de la 200ᵉ ligne, au plus
  300 lignes dans le DOM, Origine, gauche, Entrée), axe-core
  choix : lignes « suite » focalisables (`treeitem` « Charger la page suivante ») plutôt
  qu'invisibles au clavier ; un clic sur une ligne ouvre ou ferme, son lien mène à la
  fiche ; la page d'un conteneur à 10⁴ enfants coûte environ 2 s au serveur (chaque page
  recalcule tous les enfants avant le curseur), d'où des délais de 60 s dans le parcours
- [x] 19. Modification rapide : création, modification, clôture avec motif et `If-Match`,
  réponses 202, 403, 412 et 422 — REQ-007-25, 20 — verify : E2E : deux onglets sur le
  même préfixe → 412, saisie retrouvée après relecture ; politique d'approbation → 202 et
  lien ; violation placée sur son champ ; refus 403 sans perte de saisie
  où : design « Écriture » (tableau des réponses) ; 006 design « Écritures »
  — `web/shell/src/edit/fields.ts` (champs de création et de modification par
  collection, d'après `New*` et `*Patch` du contrat ; formulaire rempli depuis l'objet,
  Merge Patch des seuls membres changés, domaine en référence), `edit/submit.ts`
  (`POST`, `PATCH` en `application/merge-patch+json` ou `DELETE` par `shellFetch`, motif,
  `If-Match`, clé d'idempotence d'une création ; 200 et 201 → appliqué, 202 → en attente,
  412 → périmé, 422 → violations par champ selon leur pointeur (`/slug`), les autres en
  tête, règles `*.forbidden` lues comme un refus ; autre problème → refus),
  `edit/QuickEdit.vue` (formulaire, motif obligatoire, `aria-invalid` et
  `aria-describedby` sur le champ fautif, alerte en tête, avis et lien vers le
  changeset ; dialogue du 412 : valeur actuelle et saisie côte à côte, « Reprendre sur
  la nouvelle version » relit l'objet et garde les champs saisis, les autres suivant la
  nouvelle version), `views/ObjectView.vue` (Modifier, Clore, `ETag` de la lecture
  gardé ; `read` de `data/api.ts` rend l'`ETag`), `views/NewView.vue` et route
  `/:collection/new` (`?domain=` présélectionne ; 201 → fiche créée), lien « Créer » des
  listes ; formulaires cachés hors du présent et sans permission d'écrire ou de proposer
  le type ; catalogues `edit.*`. Jeu de bout en bout : utilisatrice `carol` (groupe
  `redacteurs` : lecteur et lecture des droits partout, éditeur de la seule entité
  `redaction`). Tests : Vitest `edit/edit.test.ts` (champs, patch, issues de chaque
  réponse, en-têtes) ; E2E `journeys/edit.spec.ts` (création d'un préfixe puis deux
  onglets → 412, dialogue, reprise, enregistrement, clôture ; domaine sous politique
  d'approbation → 202 et lien ; slug invalide → violation sur « Nom court » ; `carol` sur
  une autre entité → refus, saisie et motif gardés), axe-core
  choix : le serveur refuse une écriture hors du périmètre par un 422 portant
  `core.object.forbidden` (REQ-005-22), le 403 étant réservé aux permissions globales ;
  le shell montre les deux comme un refus (« Vous n'avez pas le droit de faire cela. »),
  saisie gardée ; le lien du changeset vise `/changesets/<id>` (écran de la tâche 21) ;
  valeurs des listes fermées (nature, usage) affichées telles que le contrat les nomme
- [x] 20. Brouillon : magasin IndexedDB, onglets, simulation continue, soumission
  idempotente, identité, ajout par les plugins — REQ-007-26, 27 — verify : Vitest
  (`fake-indexeddb`) : clés par identité et onglet, onglet dupliqué, brouillons d'une
  autre identité effacés sans lecture, clé reprise tant que le contenu ne change pas ;
  E2E : trois opérations dont une invalide, simulation, correction, un seul changeset ;
  coupure réseau à la soumission puis nouvel essai → un seul changeset ; session expirée
  → brouillon intact ; déconnexion → brouillon effacé
  où : design « Écriture » (brouillon), « Erreurs et cas limites » ; 004 REQ-004-9 ;
  `web/shell/src/draft/`
  — `web/shell/src/draft/store.ts` (IndexedDB `kartenn-drafts`, clé
  `<identité>:<onglet>`, index `identity`, purge des autres identités par curseur de clés,
  purge à 30 jours, connexion fermée après chaque transaction), `tab.ts` (identifiant en
  `sessionStorage`, onglet dupliqué détecté par `BroadcastChannel`), `index.ts`
  (`DraftController` : simulation 500 ms après le dernier changement, précédente annulée ;
  clé d'idempotence écrite avant l'envoi et reprise tant que `contentHash` ne change pas ;
  201/202 efface, 422 garde, 409 réessaie, réseau → « Réessayer » ; `installDraft` suit
  l'identité de la session et reçoit le `draft.add` des plugins), `DraftDrawer.vue`
  (tiroir non modal), bouton « Brouillon (n) » dans `AppShell.vue`, catalogues `draft.*` ;
  dépendance de développement `fake-indexeddb`. Tests : `draft/draft.test.ts` (les quatre
  du verify, plus purge à 30 jours), `e2e/journeys/draft.spec.ts` (les trois parcours) ;
  `example.spec.ts` admet les simulations parmi les requêtes du brouillon.
  choix : une opération s'ajoute ou se corrige au JSON du contrat dans le tiroir (pas de
  formulaire par type d'opération ; la modification rapide reste le parcours guidé) ; le
  tiroir est une colonne non modale ; un brouillon est écrit en copie JSON (le proxy
  réactif ne passe pas le clonage structuré d'IndexedDB) ; la déconnexion efface déjà les
  bases `kartenn*` (tâche 14), sans code de plus.
- [x] 21. Changesets : liste filtrée, fiche et différentiel, validation, rejet, retrait,
  annulation, propositions — REQ-007-18 — verify : E2E : proposition de Bob validée par
  Alice puis annulée, chaque étape visible
  où : design « Écrans et routes du client » ; 006 design « Ressources et routes »
  (changesets) ; `web/e2e/users.toml`
  — `web/shell/src/changesets/` : `changesets.ts` (états du journal, actions offertes
  selon l'état, l'auteur et le contexte temporel : validation et rejet par un autre,
  retrait par l'auteur, annulation d'un appliqué), `ChangesetsView.vue` (`/changesets`,
  filtres état, auteur, période dans l'URL, « Mes changesets »), `ProposalsView.vue`
  (`/proposals`, droits qui manquent à l'auteur), `ChangesetView.vue` (`/changesets/:id`
  : état, opérations, violations, différentiel compté, actions envoyées avec l'`ETag` lu
  en `If-Match`, lien vers le changeset d'annulation) ; `data/api.ts` (`send` : toute
  méthode, en-têtes, `ETag` de la réponse) ; route et entrée « Changesets » sous
  `core.changeset.read` ; catalogues `changeset.*`. Tests : `changesets.test.ts`,
  `e2e/journeys/changesets.spec.ts` ; `global-setup.ts` définit le rôle `contributor`
  (proposer adresses et préfixes, lire les changesets) lié au groupe de Bob.
  choix : l'auteur est montré par son identifiant (« vous » pour l'appelant), sans lecture
  des identités, réservée à `core.rbac.read` ; le différentiel montré est le décompte du
  changeset, le détail paginé (`/diff`) reste à la demande des plugins ; pas de point
  d'accroche du brouillon (007-20) : un changeset soumis n'en dépend pas.
- [x] 22. Administration : rôles, liaisons, rattachements d'émetteurs, comptes de service
  et secrets, identités et pseudonymisation, audit (recherche, export), suivi des imports
  — REQ-007-19 — verify : E2E : compte de service et secret affiché une seule fois ;
  recherche d'audit qui retrouve sa création ; export NDJSON enregistré ; import suivi,
  repris, annulé
  où : design « Écrans et routes du client » ; 006 design « Ressources et routes »
  (droits, audit, imports) ; route de la tâche 7
  — `web/shell/src/admin/` : `admin.ts` (sections et permissions qui les ouvrent, issue
  d'une écriture par changeset : appliquée, en attente, refusée, secret montré une fois),
  `write.ts` (`useAdminWrite`), `PagedTable.vue`, `WriteOutcome.vue`, `SecretDialog.vue`
  (secret dans un dialogue, oublié à sa fermeture), `RolesView`, `BindingsView`,
  `IssuersView`, `ServiceAccountsView` et `ServiceAccountView` (secrets sans valeur,
  nouveau secret, révocation), `IdentitiesView` et `IdentityView` (désactivation,
  révocation des sessions en Merge Patch, pseudonymisation confirmée), `AuditView`
  (filtres, export `application/x-ndjson` enregistré comme fichier), `ImportsView` et
  `ImportView` (relu chaque seconde tant qu'il est prêt ou en cours, rapport, reprise,
  annulation) ; `views/AdminView.vue` (sections selon les droits) ; routes `/admin/*`,
  `/imports[/:id]`, entrée « Imports » ; catalogues `admin.*`, `imports.*`. Tests :
  `admin.test.ts`, `e2e/journeys/admin.spec.ts` (compte de service et secret, audit et
  export, import en pause repris puis annulé, sections accessibles, lecteur refusé).
  choix : les écritures portent leur motif dans le formulaire et désactivent les boutons
  hors du présent ; clore un rôle, une liaison, un rattachement ou un secret réutilise le
  motif saisi dans le formulaire de la page ; l'entrée « Imports » est montrée à tous
  (chacun ne voit que les siens) ; dates saisies en RFC 3339 dans des champs texte.

## P6 — Plugin d'exemple, budgets, synchronisation

- [x] 23. Plugin d'exemple sans framework : vue, onglet, panneau et colonne, action qui
  ajoute une opération au brouillon, catalogues FR et EN, feuille par `styles()`, tests
  Vitest par le bac à sable ; build vers `web/dist/plugins/` — REQ-007-6, 11, 21 —
  verify : Vitest du plugin ; E2E sur Chromium et Firefox : chaque point visible là où
  il est déclaré, élément en mode sombre, opération ajoutée au brouillon
  où : design « Plugins côté serveur » (manifeste), « SDK » ; `web/plugins/example/`
  — `web/plugins/example/` : `src/index.ts` (cinq éléments en shadow DOM, client et
  brouillon du SDK, redessin au changement de contexte), `public/` (`manifest.json`,
  `styles.css` sur les seuls jetons, `i18n/fr.json`, `i18n/en.json`), `vite.config.ts`
  (build vers `web/dist/plugins/example/1.0.0/`, `@kartenn/sdk` externe), projet Vitest
  `example` ; colonnes des plugins dans les listes (`DataTable.vue`, `ListView.vue`,
  point `column` jusque-là sans hôte) ; `e2e/serve.sh` réunit plugins de test et plugin
  d'exemple construit sous `web/e2e/.state/plugins` (`plugins_dir` des parcours). Tests :
  `plugins/example/test/example.test.ts` (10, bac à sable), `e2e/journeys/example.spec.ts`
  (points, mode sombre, brouillon).
  choix : axe-core lancé sans préchargement CSSOM (il recopie les feuilles des shadow
  roots en `<style>`, refusés par la CSP ; seule `css-orientation-lock` en dépend) ;
  libellés distincts de ceux du plugin de test `notes` pour garder les sélecteurs
  univoques ; le compteur du brouillon est montré par la vue du plugin en attendant la
  tâche 20 ; `[web] plugins_dir` non ajouté à `dev/kartenn.toml` : la configuration
  exige un répertoire existant, relatif au répertoire courant, ce que le test
  `the_development_configuration_is_valid` (lancé depuis le crate) ne trouve pas.
- [x] 24. Banc `bench-web` : taille du shell, `kartenn-bench seed` et domaine à 10⁵
  enfants, projet Playwright `budgets` (premier écran, page de 100 lignes, arbre, plugin
  lent) — REQ-007-32 — verify : chaque mesure sous son budget, sortie non nulle sinon ;
  référence consignée dans la note de clôture
  où : design « Budgets » ; `devenv.nix` (`scripts.bench-http` à imiter) ;
  `kartenn-bench/src/main.rs` (sous-commandes), `lib.rs` (`Dataset`) ; agent
  `bench-runner`
  — `devenv.nix` (`scripts.bench-web [graine] [small]`, sous `k-heavy --pg`) ;
  `web/shell/scripts/size.mjs` (manifeste de Vite, `build.manifest`) ;
  `kartenn-bench seed` (`core_budgets::seed`, `wide_container` partagé avec le banc des
  enfants) ; `web/e2e/serve.sh budgets` (kartenn-server en release sur `kartenn_bench`,
  plugin d'exemple et plugin `slow`) ; `web/e2e/budgets.config.ts`, `budgets/global-setup.ts`,
  `budgets/budgets.spec.ts` ; marques `k:ready` (`PrefixTree.vue`, racines dessinées) et
  `k:rows` (`DataTable.vue`, lignes dessinées). Référence du 2026-10-21, graine 42, jeu
  complet, CPU ×4 : shell initial 123 858 o gzip -9 (≤ 300 000) ; premier écran 451 ms
  (≤ 2 s) ; page de 100 lignes 22 ms p95 sur 20 (≤ 100 ms) ; 10 011 lignes du conteneur
  de 10⁵ atteintes en 10 s, aucune tâche longue (> 50 ms) ; plugin `slow` écarté à 5 s.
  choix : premier écran compté depuis `fetchStart` du document du shell (après les
  redirections du rappel) ; la page de 100 lignes est mesurée sur `/addresses` ; le
  groupe d'`alice` lié à `reader` par l'API sous la session de secours, comme les
  parcours.
- [x] 25. Accessibilité de bout en bout : parcours entier au clavier (arbre, table,
  dialogue, menu d'actions), axe dans chaque parcours vérifié par un test, revue des
  écrans des tâches 13 à 23 — REQ-007-33 — verify : `k-web-e2e` vert ; un fichier de
  `journeys/` sans `expectAccessible` fait échouer le test de couverture
  où : design « Thème, composants, arbre » ; `web/e2e/fixtures/`
  — `web/e2e/journeys/accessibility.spec.ts` : test de couverture (`uncovered`, chaque
  fichier de `journeys/` appelle `await expectAccessible(` ; vérifié sur un parcours sans
  axe), axe sur les 26 écrans du shell et la page de connexion, parcours au clavier seul
  (lien d'évitement, arbre, fiche, menu « Actions », dialogue d'un plugin fermé par
  Échap, liste des adresses, ligne ouverte par Entrée) ; `k-web-e2e` vert (80 parcours).
  choix : la revue des écrans est faite par axe sur chacun (aucune violation trouvée,
  rien à corriger) ; le test de couverture lit les sources des parcours plutôt qu'il
  n'instrumente l'exécution.
- [x] 26. Synchronisation de la spec système : `docs/spec/interfaces.md` (SDK de plugins
  UI, `[web]` et l'avertissement de confiance, ajouts à l'API), `architecture.md`,
  `invariants.md` (INV-23 et ses tests), ADR 0025 acceptée, `specs/README.md` — verify :
  `spec-keeper` sur `specs/007-web-shell-plugins`
  où : `docs/spec/interfaces.md` (« SDK de plugins UI », « Capacités »),
  `docs/spec/architecture.md`, `docs/spec/invariants.md`, `docs/adr/0025-*`,
  `specs/README.md` ; agent `spec-keeper`
  — `interfaces.md` (section « Shell Web et plugins UI », capacités et ajouts à l'API,
  « Configuration du shell » avec l'avertissement de confiance), `architecture.md`
  (composant, arborescence `web/`, `kartenn-dev-idp`, déploiement sans serveur statique
  dédié, contrat écrit par `kartenn-server openapi`), `invariants.md` (INV-23 et ses
  tests), ADR 0025 acceptée, `specs/README.md` (tableau et point d'étape) ; dérive
  corrigée : relais Vite de `/v1` et `/plugins` (REQ-007-36) absent de
  `web/shell/vite.config.ts`.
  choix : audit de dérive fait par lecture ciblée du code face au design (pas d'agent
  `spec-keeper` disponible dans ce contexte) ; budgets du banc web laissés à la tâche 24 ;
  écarts restants listés au point d'étape de `specs/README.md` (422 hors périmètre,
  premier administrateur, `plugins_dir` de développement).
