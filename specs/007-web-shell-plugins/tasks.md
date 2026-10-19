---
status: approved
updated: 2026-10-19
---

# 007 — Shell Web et plugins : tâches

Chaque tâche est un commit, laisse le build vert (`devenv test`, `k-check`,
`k-web-check`) et nomme ce qu'elle implémente. Les tests Rust du web passent par
`tower::ServiceExt::oneshot` sur le routeur complet, comme ceux de la 006 ; les
parcours E2E par `k-web-e2e` contre le build servi par kartenn-server (design « Tests de
bout en bout »). Une phase ne commence que lorsque la précédente est verte.

## P1 — Socle et prototype de risque

- [ ] 1. Socle `web/` : espace de travail pnpm (`sdk`, `shell`, `plugins/exemple`),
  TypeScript, Vite, `vue-tsc`, ESLint (Vue, TypeScript, `@intlify/eslint-plugin-vue-i18n`,
  règle qui interdit d'importer `web/shell` depuis `web/plugins`), Prettier, Vitest
  (projets `sdk`, `shell`, `plugins`), Playwright épinglé sur `playwright-driver`,
  en-têtes SPDX et `web/scripts/licenses.mjs` ; versions de chaque dépendance fixées et
  consignées — REQ-007-62 (lint), 65 — verify : `k-web-check` vert sur le squelette ; un
  import de `web/shell` depuis un plugin fait échouer le lint ; un fichier sans en-tête
  et une dépendance sous licence hors liste font échouer `licenses.mjs`
  où : ADR 0024 ; `devenv.nix` (`hasWeb`, `scripts.k-web-check`, `scripts.k-web-e2e`,
  `processes.web`, `languages.javascript`) ; design « Crates, paquets et dépendances »,
  « Arborescence de `web/` », « Licences »
- [ ] 2. Prototype de risque `docs/spikes/007-import-map-integrite.md` : page servie par
  un petit routeur axum de test avec une import map (`imports`, `integrity`), un bloc de
  données JSON et la CSP du design ; un SDK factice partagé par un « shell » et un
  « plugin » ; un module altéré ; un composant Vue avec Reka UI sous `style-src 'self'`
  ; vue-i18n sans `unsafe-eval` ; cookie `Secure` sur `http://127.0.0.1` à travers le
  relais de Vite — verify : la note donne, par navigateur (Chromium, Firefox, WebKit
  s'il est fourni par Nix), instance unique du SDK, module altéré refusé, bloc de
  données non bloqué, violations de CSP relevées, cookie accepté ; chaque risque du
  design est confirmé ou levé, et un écart ouvre un arbitrage avant P2
  où : design « Risques », « Rendu d'`index.html` », « Développement » ; note
  [`plateforme-web.md`](../../docs/reference/plateforme-web.md) (pièges) ;
  `docs/spikes/README.md` (format, tableau à compléter)

## P2 — Serveur

- [ ] 3. Configuration `[web]` (`root`, `plugins_dir`) et `label` de `[[auth.issuers]]`,
  champs inconnus refusés, `[web]` sans `[http]` refusé, variables `KARTENN_WEB__*` —
  REQ-007-66, 67 (`label`) — verify : tests de configuration (champ inconnu, `root`
  absent, `[web]` sans `[http]`, `label` de 65 caractères, défaut `id`)
  où : `kartenn-server/src/config.rs` (`Config`, `Http`, `Issuer`, `check_auth`, tests
  du module) ; design « Configuration »
- [ ] 4. `kartenn_rest::web` : `Build::load` (build en mémoire, bornes, liens
  symboliques, marqueur, `kartenn-build.json`, SHA-384 de chaque module), routes
  `/assets`, repli de navigation, 404, `nosniff`, `Referrer-Policy`, cache et 304, routeur
  web hors des couches 6 à 8, câblage dans `Api::start` — REQ-007-1, 2, 4 — verify :
  intégration `tower` : `GET /` et une navigation profonde → gabarit, `GET
  /assets/absent.js` → 404, `/v1/absent` → problème, `..`/`%2f`/lien sortant → 404,
  `immutable` sur `/assets`, 304 ; sans `[web]`, `GET /` → 404 ; 300 requêtes anonymes
  de fichiers sans 429
  où : `kartenn-rest/src/lib.rs` (`router`, `not_found`), `authn.rs` (`public`),
  `kartenn-server/src/app.rs` (`Api::start`) ; design « Service du web par
  kartenn-server » ; test voisin `kartenn-rest/tests/socle.rs`
- [ ] 5. Rendu d'`index.html` : import map (`@kartenn/sdk`, `integrity` de chaque module
  du build), bloc de données des plugins, échappement du JSON, CSP exacte avec
  l'empreinte SHA-256, ETag, cache par ensemble ; `web::session` (session facultative,
  réémission du cookie, suppression d'un cookie expiré) — REQ-007-3, 11, 15 (serveur) —
  verify : la CSP du design à l'octet près ; l'empreinte de la CSP égale le SHA-256 du
  contenu de l'import map ; un manifeste contenant `</script>` ne ferme pas le bloc ;
  ETag stable, changé avec l'ensemble ; anonyme et authentifié diffèrent
  où : design « Rendu d'`index.html` », « Service du web par kartenn-server » (CSP) ;
  `authn.rs` (`layer`, `Authenticated`), `cookies.rs` ; test voisin
  `kartenn-rest/tests/sessions.rs` (fixture de session)
- [ ] 6. Manifeste et plages : `web::manifest` (serde `deny_unknown_fields`, contrôles du
  design), `web::range` (sous-ensemble SemVer), fixtures partagées
  `web/sdk/test/manifests/{valid,invalid}/` et `web/sdk/test/ranges.json` lues par les
  tests Rust — REQ-007-8, 12 (validation) — verify : chaque fixture invalide refusée
  avec son champ ; `foo` et `foo-bar` déclarant `foo-bar-vue` ; noms `k` et `kartenn`
  refusés ; vecteurs de plages (`^0.y`, `~`, `>= <`, `||` refusé)
  où : design « Registre des plugins » (manifeste, plages) ; note de référence
  (SemVer et plages)
- [ ] 7. Registre et service des plugins : `web::Registry::scan` (deux sources, empreintes
  de liste, bornes 8/32/128 Mio, conflits, `requires_module`, plage hors SDK servie →
  `warn`), `ArcSwap`, logs, routes `/plugins/<nom>/<version>/…` (session requise,
  `no-cache`, ETag), intégrité des plugins dans l'import map rendue, `ui_plugins` des
  capacités — REQ-007-7, 8 (conflits), 9, 11 — verify : intégration : plugin servi à une
  session, 404 sans session et `index.html` anonyme sans lui ; `.sh` → 404 ; conflit
  → deux plugins écartés, première partie gardée ; `ui_plugins` décrit par le contrat ;
  fichier modifié sur le disque : octets servis = empreinte publiée jusqu'au scan
  où : design « Registre des plugins », « Ajouts à l'API » (`ui_plugins`) ;
  `capabilities.rs` (`UiPlugin`, `Capabilities`, `capabilities`) ; test voisin
  `kartenn-rest/tests/contract.rs` (assertion `ui_plugins`)
- [ ] 8. Rechargement à chaud : scan toutes les 30 s et sur `SIGHUP` (signal installé
  aussi avec `plugins_dir`), une passe à la fois, invalidation du rendu, `plugins_dir`
  disparu → ensemble vide et `error` — REQ-007-10 — verify : instance en marche : plugin
  ajouté visible ≤ 30 s, remplacé, retiré (404) ; `SIGHUP` immédiat ; un `SIGHUP` relit
  aussi le certificat
  où : `kartenn-server/src/app.rs` (`hangup`, `Api::reload`, boucle principale) ; design
  « Registre des plugins » (minuterie) ; test voisin du rechargement TLS
  (`kartenn-server`, tâche 006-2)
- [ ] 9. Ajouts de l'API pour la session et les droits : `Rights::permissions` et
  `permissions` dans `GET /v1/me` ; `GET /v1/auth/issuers` public (`authn::PUBLIC`,
  compartiment anonyme) ; `POST /v1/auth/logout` avec `Clear-Site-Data: "storage"` et sa
  variante JSON ; contrat de référence régénéré — REQ-007-45 (serveur), 67 — verify :
  `permissions` d'une personne liée sur un domaine seulement ; motifs rendus tels quels ;
  `issuers` sans session, dans l'ordre de la configuration ; logout JSON → 200 et
  `end_session_url`, sans `Accept` → 303, les deux avec `Clear-Site-Data` et cookie
  retiré ; comparateur : ajouts seuls
  où : `kartenn-service/src/authn.rs` (`Rights`), `kartenn-store-pg/src/identities.rs`
  (`rights`), `rbac_read.rs` (`global_entries`), `kartenn-rest/src/auth.rs` (`me`, `Me`,
  `logout`, `routes`), `authn.rs` (`PUBLIC`) ; design « Ajouts à l'API » ; tests voisins
  `kartenn-rest/tests/sessions.rs` (`me_over`), `contract.rs`
- [ ] 10. Crate `kartenn-dev-idp` : découverte, JWKS ES256, `authorize` (page de choix,
  `login_hint`), `token` (code unique, PKCE S256, `nonce`), `end_session`, utilisateurs
  et groupes en TOML, écoute sur la boucle locale seulement, sous-commande `cert` (rcgen)
  — REQ-007-61 — verify : connexion OIDC complète de kartenn contre lui (groupes reçus) ;
  `listen` hors boucle locale refusé ; test de l'arbre des dépendances : aucune crate
  livrée n'en dépend
  où : `kartenn-rest/tests/sessions.rs` (`Idp`, `discovery`, `jwks`, `token`,
  `authorize`) ; design « Tests de bout en bout et émetteur de développement » ;
  `Cargo.toml` (`members`), workspace-hack à régénérer
- [ ] 11. Environnement de développement : `dev/dev.toml`, `dev/dev-idp.toml`,
  processus devenv `kartenn` et `dev-idp`, relais de Vite (`/v1`, `/plugins`,
  `Forwarded: proto=https`), import map de développement — W26 — verify : `devenv up`
  puis, par le MCP `playwright` sur `127.0.0.1:5173`, connexion par l'émetteur de
  développement et `GET /v1/me` réussi depuis la page
  où : `devenv.nix` (`processes.web`, `processes.openobserve` comme modèle),
  `web/shell/vite.config.ts` ; design « Développement » ; prototype de la tâche 2
  (cookie sur `http://127.0.0.1`)

## P3 — SDK

- [ ] 12. Client HTTP du SDK : types générés (`openapi-typescript`) et leur contrôle
  dans `k-web-check`, `request` (même origine, `QUERY`, ETag et `If-Match`,
  `Idempotency-Key` réutilisée, 3 essais), itérateur de pages, `ApiError`, crochet 401
  — REQ-007-20, 21 — verify : Vitest avec `fetch` simulé : 412 → conflit ; coupure puis
  nouvel essai → même clé ; `PATCH` sans ETag refusé par le type ; pages suivies jusqu'à
  la fin ; problème inconnu → `ApiError` avec `requestId` ; types modifiés à la main →
  `k-web-check` échoue
  où : design « Client HTTP » ; contrat `specs/006-api-rest/openapi-v1.json` ; design
  de la 006 « Erreurs » (URN), « Écritures » (membres réservés)
- [ ] 13. Surface du SDK : `context` observable, `attachHost` (`@internal`, unique),
  `can` (motifs de la 005), `i18n`, `draft`, `notify`, `navigation`, `tokens`,
  `SDK_VERSION` = `1.0.0` ; rapport api-extractor `web/sdk/etc/sdk.api.md` et script de
  comparaison — REQ-007-15, 16, 17, 26 — verify : Vitest : un abonné reçoit chaque
  changement ; second `attachHost` refusé ; `can('prop.env.write')` avec `prop.*.write` ;
  une ligne retirée du rapport sans version majeure fait échouer `k-web-check`
  où : design « SDK » ; `docs/spec/permissions.md` (motifs) ; W24
- [ ] 14. Schéma JSON du manifeste, plages en TypeScript (vecteurs communs) et bac à
  sable `@kartenn/sdk/testing` — REQ-007-8 (schéma), 12, 22 — verify : les fixtures de
  la tâche 6 passent le schéma comme le Rust ; vecteurs de plages ; un élément de test
  monté dans le bac à sable lit un objet par un `fetch` simulé typé
  où : design « Registre des plugins » (manifeste), « SDK » (bac à sable) ; fixtures
  `web/sdk/test/` de la tâche 6

## P4 — Shell : cadre

- [ ] 15. Build et cadre du shell : `@kartenn/sdk` `external` et SDK compilé à part,
  marqueur et message « navigateur non pris en charge », `kartenn-build.json`,
  `web/scripts/size.mjs` (≤ 300 Ko gzip), vue-router et mise en page, invitation à
  recharger après l'échec d'un fragment — REQ-007-5, 6, 56, 63 — verify : build servi
  par kartenn-server (tâche 4) ; `size.mjs` échoue au-delà du budget ; fragment
  introuvable → invitation à recharger
  où : design « Build et répertoire servi », « Budgets » (poids) ; `Build::load` de la
  tâche 4 (format attendu)
- [ ] 16. Jetons, thèmes, primitives et i18n : `tokens.css` (clair, sombre, préférence),
  test des contrastes, composants Reka UI de base, région de statut, vue-i18n
  (catalogues FR/EN, parité des clés, `no-raw-text`, choix de la langue, `lang`),
  formats de dates et de nombres, fuseau affiché, saisie en RFC 3339 — REQ-007-47, 48,
  50, 52, 53 (contrastes), 55 — verify : Vitest : parité des clés, repli sur `en`,
  contrastes, conversion `10:00` d'hiver → `+01:00` ; un élément sans Vue en Shadow DOM
  hérite de `--k-color-text`
  où : design « Internationalisation », « Composants, jetons et accessibilité »
- [ ] 17. Session dans le shell : page de connexion (émetteurs, secours), 401 et retour à
  l'URL (`kartenn.return`), déconnexion à plusieurs onglets (IndexedDB, Cache Storage,
  `localStorage`, `BroadcastChannel`), réécriture de la langue et du thème —
  REQ-007-44, 45, 46 — verify : Vitest sur la séquence (URL de retour hostile ignorée,
  ordre effacement → logout → navigation) ; le parcours E2E est à la tâche 31
  où : design « Session » ; routes `/v1/auth/*` (`kartenn-rest/src/auth.rs`) ; W22, W28
- [ ] 18. Contexte temporel et URL : paramètres de route, `useTime`, contexte du SDK,
  bandeau, lecture seule (`readOnly`, refus du SDK sans requête) — REQ-007-23, 24, 25,
  26 — verify : Vitest : URL → contexte → appels du SDK avec `valid_at`/`as_of` ;
  `as_of` passé → formulaires désactivés, `draft.add` refusé ; bandeau absent au présent
  où : design « Contexte temporel et URL » ; REQ-006-5 (format des dates)
- [ ] 19. Hôte des plugins : lecture du bloc et de l'import map, plage, présence de
  l'intégrité, enveloppe de `customElements.define`, délai de 5 s, états, emplacements
  avec cadre d'erreur et attribution par URL, catalogues sous espace de noms, filtrage
  par type et permission, vue `/plugins` — REQ-007-11, 12, 13, 14, 18, 19, 49 — verify :
  Vitest : plugin incompatible, expiré, nom non déclaré, élément déjà défini, erreur au
  rendu attribuée, clé hors espace de noms ignorée, extension masquée sans permission
  où : design « Chargeur de plugins du shell » ; SDK de la tâche 13 (`attachHost`, `can`)

## P5 — Shell : écritures et vues

- [ ] 20. Modification rapide et issues des écritures (200/201, 202, 412 et version
  courante, 422 aux champs par pointeur JSON, 403), traduction des problèmes par `type`
  et des violations par règle, identifiant de requête copiable — REQ-007-27, 28, 51 —
  verify : Vitest des composants de formulaire contre des réponses simulées typées ; la
  saisie survit à chaque issue
  où : design « Écritures et brouillon » (modification rapide), « Internationalisation »
  (erreurs) ; design de la 006 « Erreurs » (`violations`)
- [ ] 21. Brouillon : magasin IndexedDB, identité, purge à 7 jours, verrou Web Locks,
  plafond des capacités, simulation (400 ms, abandon, révision), soumission idempotente,
  opérations des plugins et leur origine, lecture seule au passé — REQ-007-29 à 34 —
  verify : Vitest (IndexedDB simulé) : autre identité purgée ; deux onglets → lecture
  seule ; trois modifications → une simulation affichée ; réponse perdue → même clé ;
  opération d'un plugin marquée
  où : design « Écritures et brouillon » (brouillon) ; SDK `draft` (tâche 13), client
  `submit` (tâche 12)
- [ ] 22. Listes et fiches du noyau : entités, sites, domaines, préfixes, plages,
  adresses ; filtres de REQ-006-9 dans l'URL ; listes virtualisées en motif Grid
  (`aria-rowcount`, `aria-rowindex`, ligne focalisée rendue) ; propriétés résolues sur
  la fiche ; colonnes et panneaux de plugins — REQ-007-35, 54 (tables) — verify : Vitest
  des listes (pages chargées au défilement, navigation au clavier) ; axe sur la fiche
  dans le parcours de la tâche 31
  où : design « Vues », « Contexte temporel et URL » (paramètres) ; design de la 006
  « Ressources et routes »
- [ ] 23. Arbre des préfixes : niveaux à la demande, virtualisation, occupation, prochain
  libre vers le brouillon ou la modification rapide, motif Tree View au clavier —
  REQ-007-36, 54 (arbre), 59 (structure) — verify : Vitest : flèches droite et gauche,
  `aria-level`, `aria-setsize`, `aria-posinset` ; niveau de 10⁴ enfants rendu par
  fenêtre
  où : design « Vues » (arbre) ; APG Tree View (note de référence)
- [ ] 24. Propriétés, historique et anomalies : déclarations, listes de valeurs,
  affectations, valeurs résolues et explication, suggestion, écarts ; historique et
  « ouvrir à cette version » ; anomalies filtrées et acquittement avec motif —
  REQ-007-37, 38, 39 — verify : Vitest des vues contre des réponses typées ; ouvrir une
  version pose `valid_at` et `as_of` dans l'URL
  où : design « Vues » ; 003/REQ-003-14, 23, 24 ; 004/REQ-004-18 ; 002/REQ-002-36
- [ ] 25. Changesets et revue : liste filtrée, lecture et différentiel, file des
  propositions et approbations, valider, rejeter (motif), retirer, annuler (conflits),
  `If-Match` — REQ-007-40 — verify : Vitest : l'auteur ne voit pas « valider » ; 409
  d'annulation → liste des conflits
  où : design « Vues » ; REQ-006-16
- [ ] 26. Administration des droits : rôles, liaisons, rattachements, comptes de service
  et secrets (montrés une fois, gardés en mémoire du composant), état des identités,
  par changesets — REQ-007-41 — verify : Vitest : le secret n'apparaît dans aucun
  stockage après fermeture du dialogue
  où : design « Vues », « Session » (rien de secret) ; 005/REQ-005-11, 25, 26
- [ ] 27. Audit et imports : recherche, pages, export NDJSON téléchargé,
  pseudonymisation confirmée ; liste et état des imports, relecture au rythme de
  `Retry-After`, reprendre et annuler — REQ-007-42, 43 — verify : Vitest : export en
  `Blob` ; relecture au délai de `Retry-After`
  où : design « Vues » ; REQ-006-19 ; 012/REQ-012-15, 17

## P6 — Plugins, bout en bout, budgets, synchronisation

- [ ] 28. Plugin d'exemple sans framework : quatre points d'extension, catalogues FR/EN,
  suivi du contexte, opération ajoutée au brouillon, CSS en fichier, tests dans le bac à
  sable, build vers `web/dist-plugins/exemple/` ; guide de l'auteur de plugin
  (`web/plugins/exemple/README.md`) — REQ-007-18, 22, 64 — verify : tests Vitest du
  plugin ; manifeste accepté par le registre (tâche 7)
  où : design « Registre des plugins » (manifeste d'exemple), « SDK » ; W5, W7
- [ ] 29. Mise en place des parcours : `dev/e2e.toml`, `globalSetup` (base, émetteur,
  serveur TLS, compte de secours, liaisons, jeu fonctionnel par l'API, dossier
  `target/e2e-plugins`), plugins d'essai écrits à la main, fixtures (axe avec les tags
  du design et `target-size`, collecteur de `securitypolicyviolation`, refus de toute
  requête vers une autre origine), projets `chromium`, `firefox`, `webkit` — REQ-007-53
  (axe), 60 — verify : un parcours vide passe sur les trois projets ; une violation axe
  injectée échoue ; une requête vers une autre origine échoue
  où : design « Tests de bout en bout et émetteur de développement » ; `devenv.nix`
  (`scripts.k-web-e2e`) ; émetteur de la tâche 10
- [ ] 30. Parcours de sécurité et des plugins : CSP (aucune violation ; `fetch`
  ailleurs, `eval`, script en ligne bloqués), intégrité (module altéré), incompatible,
  en échec, lent (premier écran non retardé), conflit de noms, droits d'un plugin,
  plugin servi à chaud au rechargement, plugins absents sans session, message sans
  import maps — REQ-007-3, 5, 7, 10, 11, 13, 14 — verify : `k-web-e2e` vert sur
  `chromium`, `firefox` ; intégrité et CSP sur `webkit` s'il est fourni
  où : plugins d'essai de la tâche 29 ; design « Erreurs et cas limites »
- [ ] 31. Parcours fonctionnels : connexion OIDC et secours, 401 et retour, lecture
  datée et bandeau, modification rapide (412, 422, 202), brouillon (simulation,
  soumission, rechargement, deux onglets), revue par un autre utilisateur,
  administration, audit, imports, plugin d'exemple aux quatre points, clavier seul et
  géométrie du focus (2.4.11), changement de langue, déconnexion à deux onglets et
  stockage vide — REQ-007-23 à 46, 48, 54, 60 — verify : `k-web-e2e` vert sur
  `chromium` et `firefox`, axe à chaque étape
  où : design « Tests de bout en bout et émetteur de développement » (parcours),
  « Composants, jetons et accessibilité » (critères sans règle)
- [ ] 32. Budgets du navigateur : sous-commande `kartenn-bench web <url> <graine>` (jeu
  de `bench-core`, personne liée par l'émetteur de développement), projet Playwright
  `budgets` (CPU ×4, marques, `longtask`), script devenv `bench-web` — REQ-007-56 à 59 —
  verify : premier écran ≤ 2 s avec un plugin en échec et un lent ; page de 100 ≤ 100 ms ;
  défilement d'un niveau de 10⁵ sans tâche longue ; mesures rapportées dans la note de
  la tâche
  où : design « Budgets » ; `kartenn-bench/src/http.rs`, `rbac.rs` (personne des
  budgets), `main.rs` ; `devenv.nix` (`scripts.bench-http` comme modèle) ; agent
  `bench-runner`
- [ ] 33. Documentation : guide de l'exploitant (`[web]`, dépôt d'un plugin par copie
  puis renommage, confiance accordée à un plugin, bornes), grille de revue
  d'accessibilité `docs/practices/accessibilite.md` (2.4.11, 2.5.7, 3.2.6, 3.3.7,
  3.3.8) — REQ-007-14 (documentation), 53 — verify : relecture ; liens valides
  où : W4 (conséquence), W27 ; design « Composants, jetons et accessibilité », « Erreurs
  et cas limites » (copie à moitié)
- [ ] 34. Synchronisation de la spec système : `docs/spec/interfaces.md` (SDK de plugins
  UI, configuration `[web]`, ajouts de l'API), `invariants.md` (tests d'INV-23 ; nouvel
  invariant « tout module exécuté par le shell a l'empreinte publiée »),
  `architecture.md` (livrables : plus d'image « web statique »), ADR 0025 accepté, ADR
  0011 (Q-ouv-1 close), `specs/README.md` — verify : `spec-keeper` sur
  `specs/007-web-shell-plugins`
  où : `docs/spec/interfaces.md` (« SDK de plugins UI »), `docs/spec/invariants.md`,
  `docs/spec/architecture.md` (« Livrables »), `docs/adr/0011-*`, `docs/adr/0025-*` ;
  tâche 006-26 comme modèle ; agent `spec-keeper`
