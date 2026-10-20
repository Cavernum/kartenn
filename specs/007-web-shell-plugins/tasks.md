---
status: draft
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

- [ ] 3. Section `[web]` et service du shell : module `kartenn-rest::web` (résolution des
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
- [ ] 4. Registre des plugins : lecture de `plugins_dir`, validation du manifeste,
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
- [ ] 5. `ui_plugins` enrichi dans les capacités et contrat de référence mis à jour —
  REQ-007-7 — verify : comparateur du contrat vert (membres ajoutés seulement) ;
  `GET /v1/capabilities` sur un répertoire de test publie base, entrée, empreintes,
  plage, extensions et catalogues
  où : `kartenn-rest/src/capabilities.rs` (`UiPlugin`, `capabilities`), `contract.rs`
  (`breaches`), `specs/006-api-rest/openapi-v1.json`, binaire `openapi` ;
  `rest/tests/contract.rs` (`the_contract_keeps_its_reference`)

## P2 — Ajouts à l'API

- [ ] 6. Racines de l'arbre d'un domaine, `GET /v1/domains/{id}/roots` (W18) —
  REQ-007-39 — verify : mêmes éléments, ordre et curseurs que les enfants d'un préfixe ;
  lecture datée ; nœuds illisibles absents ; page de 100 sous 150 ms au p95 sur le jeu de
  `bench-core` (agent `bench-runner`)
  où : design « Ajouts à l'API » ; `kartenn-store-pg/src/tree.rs` (`children_of`,
  `ChildKey`, `ReadScope`), `kartenn-rest/src/tree.rs` (`children`, `listed`) ;
  `rest/tests/objects.rs` (`children_and_history_page_by_cursor`,
  `the_tree_shows_only_what_the_caller_may_read`) à imiter ; contrat de référence
- [ ] 7. Imports de l'appelant, `GET /v1/imports` (W18) — REQ-007-40 — verify : du
  plus récent au plus ancien, paginé ; ceux d'un autre absents
  où : design « Ajouts à l'API » ; `kartenn-store-pg/src/imports.rs` (`header`),
  `kartenn-rest/src/imports.rs` (contrôle de l'auteur, vers la ligne 114) ;
  `rest/tests/imports.rs` (`an_import_is_its_authors`) ; contrat de référence
- [ ] 8. Émetteurs de connexion, `GET /v1/auth/issuers` publique (W18) —
  REQ-007-41 — verify : 200 sans session, `[{id}]` dans l'ordre de la configuration,
  ni URL ni client ; route dans `PUBLIC`
  où : design « Ajouts à l'API » ; `kartenn-rest/src/auth.rs` (`routes`), `openapi.rs`
  (`PUBLIC`), `authn.rs` (`public`, trait `Authenticator`) ; `kartenn-server/src/config.rs`
  (`Issuer::id`) ; `rest/tests/sessions.rs` ; contrat de référence

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
- [ ] 10. Harnais de bout en bout `web/e2e` : projets `chromium` et `firefox`, `webServer`
  (émetteur, kartenn-server sur `web/e2e/kartenn.toml`), `global-setup.ts` (base
  `kartenn_e2e`, `migrate`, compte de secours, liaisons des groupes, jeu de données par
  changesets), fixtures de connexion, `expectAccessible`, échec sur violation CSP ;
  parcours de fumée — REQ-007-33, 34 — verify : `k-web-e2e` vert sur les deux
  navigateurs (connexion, lecture, déconnexion) ; une violation axe injectée et un script
  en ligne injecté font échouer le parcours
  où : design « Bout en bout et développement », « Thème, composants, arbre »
  (accessibilité) ; `devenv.nix` (`scripts.k-web-e2e`) ; `kartenn-server/src/main.rs`
  (`migrate`, `breakglass create`)

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
- [ ] 12. Contexte, `definePlugin`, semver, i18n de plugin, `styles()`, brouillon (surface),
  jetons, `@kartenn/sdk/host`, bac à sable `@kartenn/sdk/testing` — REQ-007-12, 13, 14,
  16 — verify : Vitest : notification du contexte, élément non déclaré ou déjà défini
  refusé, matrice semver (`^`, `~`, exact, prépublications), `t` préfixé, bac à sable qui
  monte un élément avec contexte et `fetch` simulé ; deux importateurs voient le même
  contexte
  où : design « SDK », « Chargement des plugins par le shell » (étape 4) ;
  `web/sdk/src/` (tâche 2)

## P5 — Shell

- [ ] 13. Cadre du shell : mise en page, navigation, `vue-router`, jetons de design
  clair et sombre, Reka UI, vue-i18n FR et EN, choix de la langue, traduction des
  problèmes, droits par `/v1/me`, page `/about` — REQ-007-20, 29, 30 — verify : Vitest :
  parité des clés, ordre du choix de langue, problème sans traduction → `detail`,
  contrastes des jetons dans les deux modes, couverture des motifs de permission ;
  E2E : un lecteur ne voit pas l'entrée « Administration »
  où : design « i18n », « Thème, composants, arbre », « Écrans et routes du client »
  (droits) ; 006 design « Erreurs » (URN des problèmes) ; `kartenn-rest/src/auth.rs`
  (`GET /v1/me`)
- [ ] 14. Session : page `/login` (émetteurs, secours), URL de retour, reconnexion sur
  401, déconnexion par formulaire — REQ-007-28 — verify : E2E : session expirée sur une
  fiche → reconnexion → même fiche et même contexte temporel ; déconnexion → retour à
  l'émetteur, préférences sensibles effacées
  où : design « Session » ; 006 design « Authentification et sessions » ;
  `kartenn-rest/src/auth.rs` (`callback` vers `/`, `logout`)
- [ ] 15. Chargement des plugins par le shell et hôtes des quatre points d'extension,
  filtres par type et permission — REQ-007-9, 10, 13 — verify : E2E avec les plugins de
  test de `web/e2e/plugins/` : compatible actif ; `sdk: "^9"`, empreinte fausse,
  exception au chargement, `definePlugin` absent → écartés avec leur motif sur `/about`,
  shell utilisable ; `fetch` tiers bloqué ; onglet absent sur un autre type ; action
  absente sans la permission
  où : design « Chargement des plugins par le shell » ; `web/shell/src/plugins/` ; SDK
  `definePlugin` et `host` (tâche 12)
- [ ] 16. Contexte temporel : `useTime`, garde de navigation, bandeau, écritures
  désactivées, contexte poussé au SDK — REQ-007-22, 23, 24 — verify : Vitest : `useTime`
  et la garde ; E2E : lien avec `valid_at` et `as_of` ouvert dans un second contexte de
  navigateur → mêmes valeurs ; bandeau → présent ; sous `as_of`, boutons d'écriture
  désactivés
  où : design « Contexte temporel » ; 006 design « Lectures datées et cache »
- [ ] 17. Listes et fiches du noyau : collections filtrées et virtualisées, recherche
  globale, fiche (attributs, propriétés résolues et explication, historique, anomalies et
  acquittement), occupation et prochain libre, déclarations et écarts de propriétés —
  REQ-007-17, 31 — verify : E2E sur le jeu de données : domaine → préfixe → adresse,
  propriété héritée expliquée, historique ; curseur refusé → première page ; liste de
  1 000 lignes avec quelques dizaines de lignes dans le DOM
  où : design « Écrans et routes du client » ; 006 design « Ressources et routes » ;
  `web/shell/src/views/`
- [ ] 18. Arbre des préfixes : racines du domaine, enfants à la demande, virtualisation,
  clavier `treeview` — REQ-007-17, 31, 33 — verify : E2E : ouverture, page suivante
  chargée en défilant, flèches, Origine, Fin, Entrée ; conteneur à 10⁴ enfants du jeu de
  bout en bout : quelques centaines de nœuds au plus dans le DOM (10⁵ au banc, tâche 24)
  où : design « Thème, composants, arbre » (arbre) ; route de la tâche 6 ;
  `kartenn-rest/src/tree.rs` (`children`)
- [ ] 19. Modification rapide : création, modification, clôture avec motif et `If-Match`,
  réponses 202, 403, 412 et 422 — REQ-007-25, 20 — verify : E2E : deux onglets sur le
  même préfixe → 412, saisie retrouvée après relecture ; politique d'approbation → 202 et
  lien ; violation placée sur son champ ; refus 403 sans perte de saisie
  où : design « Écriture » (tableau des réponses) ; 006 design « Écritures »
- [ ] 20. Brouillon : magasin IndexedDB, onglets, simulation continue, soumission
  idempotente, identité, ajout par les plugins — REQ-007-26, 27 — verify : Vitest
  (`fake-indexeddb`) : clés par identité et onglet, onglet dupliqué, brouillons d'une
  autre identité effacés sans lecture, clé reprise tant que le contenu ne change pas ;
  E2E : trois opérations dont une invalide, simulation, correction, un seul changeset ;
  coupure réseau à la soumission puis nouvel essai → un seul changeset ; session expirée
  → brouillon intact ; déconnexion → brouillon effacé
  où : design « Écriture » (brouillon), « Erreurs et cas limites » ; 004 REQ-004-9 ;
  `web/shell/src/draft/`
- [ ] 21. Changesets : liste filtrée, fiche et différentiel, validation, rejet, retrait,
  annulation, propositions — REQ-007-18 — verify : E2E : proposition de Bob validée par
  Alice puis annulée, chaque étape visible
  où : design « Écrans et routes du client » ; 006 design « Ressources et routes »
  (changesets) ; `web/e2e/users.toml`
- [ ] 22. Administration : rôles, liaisons, rattachements d'émetteurs, comptes de service
  et secrets, identités et pseudonymisation, audit (recherche, export), suivi des imports
  — REQ-007-19 — verify : E2E : compte de service et secret affiché une seule fois ;
  recherche d'audit qui retrouve sa création ; export NDJSON enregistré ; import suivi,
  repris, annulé
  où : design « Écrans et routes du client » ; 006 design « Ressources et routes »
  (droits, audit, imports) ; route de la tâche 7

## P6 — Plugin d'exemple, budgets, synchronisation

- [ ] 23. Plugin d'exemple sans framework : vue, onglet, panneau et colonne, action qui
  ajoute une opération au brouillon, catalogues FR et EN, feuille par `styles()`, tests
  Vitest par le bac à sable ; build vers `web/dist/plugins/` — REQ-007-6, 11, 21 —
  verify : Vitest du plugin ; E2E sur Chromium et Firefox : chaque point visible là où
  il est déclaré, élément en mode sombre, opération ajoutée au brouillon
  où : design « Plugins côté serveur » (manifeste), « SDK » ; `web/plugins/example/`
- [ ] 24. Banc `bench-web` : taille du shell, `kartenn-bench seed` et domaine à 10⁵
  enfants, projet Playwright `budgets` (premier écran, page de 100 lignes, arbre, plugin
  lent) — REQ-007-32 — verify : chaque mesure sous son budget, sortie non nulle sinon ;
  référence consignée dans la note de clôture
  où : design « Budgets » ; `devenv.nix` (`scripts.bench-http` à imiter) ;
  `kartenn-bench/src/main.rs` (sous-commandes), `lib.rs` (`Dataset`) ; agent
  `bench-runner`
- [ ] 25. Accessibilité de bout en bout : parcours entier au clavier (arbre, table,
  dialogue, menu d'actions), axe dans chaque parcours vérifié par un test, revue des
  écrans des tâches 13 à 23 — REQ-007-33 — verify : `k-web-e2e` vert ; un fichier de
  `journeys/` sans `expectAccessible` fait échouer le test de couverture
  où : design « Thème, composants, arbre » ; `web/e2e/fixtures/`
- [ ] 26. Synchronisation de la spec système : `docs/spec/interfaces.md` (SDK de plugins
  UI, `[web]` et l'avertissement de confiance, ajouts à l'API), `architecture.md`,
  `invariants.md` (INV-23 et ses tests), ADR 0025 acceptée, `specs/README.md` — verify :
  `spec-keeper` sur `specs/007-web-shell-plugins`
  où : `docs/spec/interfaces.md` (« SDK de plugins UI », « Capacités »),
  `docs/spec/architecture.md`, `docs/spec/invariants.md`, `docs/adr/0025-*`,
  `specs/README.md` ; agent `spec-keeper`
