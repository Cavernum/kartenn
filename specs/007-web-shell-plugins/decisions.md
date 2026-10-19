# 007 — Shell Web et plugins : journal des décisions

Entretien (grill-me) du 2026-10-19. Cadre : D6 (plugins UI à chaud), D7 (REST, seule
frontière publique), D17 (Vue 3 + TypeScript, Custom Elements, SDK versionné), D37
(AGPL-3.0) ; ADR 0011, 0020 (session BFF), 0022 (contrat HTTP), 0024 (outillage) ;
Q-ouv-1 close (SDK et clients générés en MPL-2.0), Q-ouv-6 tranchée pour l'interface
(i18n, catalogues FR et EN). Existant : `GET /v1/capabilities` porte déjà `ui_plugins`
(nom, version), vide (`crates/kartenn-rest/src/capabilities.rs`) ; processus devenv
`web` prêt, `web/` absent.

## Synthèse
- W1 périmètre : noyau, changesets, administration (RBAC, audit, imports), plugin d'exemple.
- W2 service : kartenn-server sert le build (`[web] root`), même origine, CSP posée par lui.
- W3 plugins : répertoire `[web] plugins_dir`, servis sous `/plugins/`, SRI, à chaud.
- W4 confiance : plugin = code de confiance de l'exploitant, même document, CSP stricte.
- W5 extensions : vues, onglets de fiche, panneaux et colonnes, actions ; filtrés.
- W6 modules métier : plugins de première partie sous `web/plugins/<module>/`.
- W7 SDK : import map `@kartenn/sdk`, instance unique, semver vérifié au chargement.
- W8 client : types openapi-typescript, couche `fetch` propre (QUERY, If-Match…).
- W9 temps : `valid_at`/`as_of` dans l'URL, bandeau hors présent, `as_of` passé en lecture.
- W10 écriture : modification rapide par ressource et brouillon local simulé en continu.
- W11 i18n : vue-i18n, catalogues FR/EN, plugins à espace de noms, erreurs par `type`.
- W12 composants : Reka UI, jetons CSS traversant le Shadow DOM, TanStack Virtual.
- W13 budgets : 300 Ko gzip, 2 s, 100 ms par page, arbre virtualisé ; bloquants en CI.
- W14 accessibilité : WCAG 2.2 AA, axe-core bloquant.
- W15 E2E : vrai serveur et PostgreSQL ; Vitest pour composants et SDK.
- W16 émetteur de dev : crate `kartenn-dev-idp`, jamais livré.
- W17 choix de conduite : API additive, `/v1/me` pour l'interface, session, état,
  navigateurs, licences par répertoire.
- W18 API : `permissions` dans `/v1/me`, `GET /v1/auth/issuers` public, `label` des émetteurs.
- W19 styles : `style-src 'self'` strict, CSS extrait au build.
- W20 `index.html` rendu par kartenn : import map avec `integrity`, plugins du document, CSP.
- W21 plugins réservés aux sessions authentifiées.
- W22 déconnexion : effacement par le shell et `Clear-Site-Data: "storage"`.
- W23 budgets sur Chromium ; WebKit pour l'intégrité et la CSP s'il est fourni.
- W24 surface du SDK : api-extractor, rapport comparé.
- W25 livraison : un répertoire de build ; paquets OCI et NixOS hors 007 ; D35 amendée.
- W26 développement : Vite relaie à kartenn-server ; E2E sur le build servi en TLS.
- W27 choix de rédaction des exigences, validés par délégation.

## W1 — Périmètre des vues
- Décision : **noyau, changesets et administration**. Données du noyau (domaines, arbre
  des préfixes, adresses, plages, propriétés, historique et contexte daté, anomalies) ;
  cycle complet des changesets (brouillon, simulation, revue, approbation, annulation) ;
  administration du RBAC, recherche dans l'audit, suivi des imports ; un plugin
  d'exemple qui éprouve le SDK.
- Pourquoi : la v0.1 (D31) doit être exploitable sans outil annexe ; VLAN (008) et
  Machines (009) viendront par les points d'extension.
- Rejeté : noyau et changesets seuls (exploitation par curl en attendant le CLI) ;
  lecture seule (ne prouve que le SDK).
- Décidé par : recommandation acceptée

## W2 — Service du shell
- Décision : **kartenn-server sert le build du shell** depuis un répertoire configuré
  (`[web] root`), sur la même origine que l'API ; les paquets OCI et Nix embarquent ce
  répertoire. Le serveur pose la CSP et les en-têtes de cache. Un serveur statique
  derrière le même répartiteur reste possible.
- Pourquoi : cookie `__Host-` en `SameSite=Strict` et CSRF par `Sec-Fetch-Site` (ADR
  0020, 006/H10) imposent la même origine ; un seul point maîtrise la CSP ; pas de
  serveur de plus.
- Rejeté : assets compilés dans le binaire (recompiler le serveur pour l'interface) ;
  serveur statique séparé par défaut (CSP et cache configurés hors de kartenn).
- Décidé par : recommandation acceptée

## W3 — Distribution et installation des plugins
- Décision : **répertoire local déclaré en configuration** (`[web] plugins_dir`).
  L'exploitant y dépose chaque plugin (manifeste et modules ES) ; le serveur les sert
  sous `/plugins/<nom>/<version>/`, calcule leurs empreintes SRI et les publie dans
  `ui_plugins` de `/v1/capabilities`. Ajout ou mise à jour par relecture du répertoire,
  sans reconstruire ni redémarrer. Avec N instances, le répertoire est le même partout
  (même paquet ou volume partagé).
- Pourquoi : à chaud sans rebuild (D6) ; tout sur la même origine, donc CSP
  `script-src 'self'` ; installer un plugin reste un geste d'exploitant.
- Rejeté : URL externes en configuration (CSP ouverte à des tiers, dépendance à un
  hébergeur) ; téléversement par l'API (injection de code par un droit applicatif,
  hors de la 006).
- Décidé par : recommandation acceptée

## W4 — Modèle de confiance des plugins
- Décision : **un plugin est du code de confiance**, choisi par l'exploitant au même
  titre que le shell : même document, Custom Elements, CSP stricte (`script-src
  'self'` sans `unsafe-eval`, `connect-src 'self'`, pas de cadre tiers), SRI. Il agit
  avec tous les droits de l'utilisateur, jamais plus (INV-23), sans restriction propre.
  Une extension non fiable passe par l'API comme un connecteur, pas par le shell.
- Pourquoi : modèle de D17 et de l'ADR 0011 ; pas de coût d'isolation (pont,
  sérialisation, thème) ; installer un plugin est déjà réservé à l'exploitant (W3).
- Rejeté : iframe sandboxée avec permissions déclarées (contredit D17, pont d'API,
  mise en page difficile) ; deux modes selon le manifeste (deux hôtes à maintenir).
- Conséquence : la documentation de l'exploitant dit qu'installer un plugin revient à
  lui confier la session de chaque utilisateur.
- Décidé par : recommandation acceptée

## W5 — Points d'extension du SDK
- Décision : **quatre points**, déclarés dans le manifeste du plugin et filtrés par type
  d'objet et par permission : (1) routes et vues pleine page avec entrée de
  navigation ; (2) onglet sur la fiche d'un type d'objet ; (3) panneau ou colonne sur
  la fiche et dans les listes ; (4) actions du menu d'un objet.
- Pourquoi : couvre les écrans de la 008 et de la 009 (onglet VLAN d'un préfixe,
  colonne, actions) et un onglet de visualisation (D6) ; une liste fermée se versionne.
- Rejeté : vues et onglets seuls (008 et 009 ne pourraient enrichir listes ni
  actions) ; slots nommés libres (contrat flou, impossible à versionner).
- Décidé par : recommandation acceptée

## W6 — Écrans des modules métier
- Décision : **plugins de première partie**. Chaque module métier compilé (VLAN,
  Machines, puis Politique et Flux) a son plugin sous `web/plugins/<module>/`, écrit
  avec le SDK public, livré dans le paquet et activé seulement si le module est
  présent (`modules` des capacités). Le shell ne dépend d'aucun module.
- Pourquoi : le SDK est éprouvé par un vrai usage dès la 008 ; un module ajouté ou
  retiré ne touche pas au shell.
- Rejeté : écrans intégrés au shell (SDK éprouvé par le seul exemple, shell recompilé à
  chaque module).
- Décidé par : recommandation acceptée

## W7 — SDK à l'exécution et versions
- Décision : **import map et semver**. Le shell déclare une import map où
  `@kartenn/sdk` désigne le module qu'il sert ; les plugins le marquent `external` à
  leur build et partagent ainsi l'instance unique du contexte. Le paquet `@kartenn/sdk`
  (MPL-2.0) fournit types, client généré et bac à sable de test. Le manifeste déclare la
  plage attendue (`sdk: "^1.2"`) ; le shell refuse un plugin incompatible et le
  signale. Un plugin embarque son propre framework s'il en utilise un.
- Pourquoi : un contexte partagé sans variable globale ; imports statiques typés ;
  compatibilité vérifiée au chargement plutôt que découverte à l'usage.
- Rejeté : objet injecté par `activate(sdk)` (pas d'import statique, typage séparé) ;
  SDK embarqué par plugin (copies divergentes, contexte par événements DOM).
- Décidé par : recommandation acceptée

## W8 — Client API
- Décision : **types générés et couche `fetch` propre au SDK**. openapi-typescript tire
  les seuls types de `specs/006-api-rest/openapi-v1.json` (aucun code à l'exécution) ;
  une couche fine écrite dans le SDK gère `QUERY`, `ETag`/`If-Match`,
  `Idempotency-Key`, Problem Details (RFC 9457) et curseurs. Les types sont régénérés
  et comparés en CI au contrat de référence.
- Pourquoi : une dépendance de développement ; comportement HTTP maîtrisé là où l'API a
  des conventions propres (`QUERY`, `If-Match` obligatoire).
- Rejeté : openapi-fetch (conventions par intercepteurs) ; générateur complet (Orval,
  hey-api : code généré volumineux, `QUERY` absent).
- Décidé par : recommandation acceptée

## W9 — Contexte temporel
- Décision : **dans l'URL, avec un bandeau**. `valid_at`, `as_of` et les filtres sont
  portés par l'URL (liens partageables et rejouables) ; hors de « maintenant », un
  bandeau permanent le dit et ramène au présent d'un clic. Un `as_of` passé désactive
  les écritures ; un `valid_at` futur reste une lecture (on écrit au futur par la date
  d'effet du changeset). Le SDK expose ce contexte aux plugins, qui le suivent.
- Pourquoi : la confusion entre présent et passé est le risque principal d'une
  interface bitemporelle ; un lien doit montrer ce qu'a vu son auteur.
- Rejeté : état local hors URL (liens non reproductibles, contexte invisible).
- Décidé par : recommandation acceptée

## W10 — Parcours d'écriture
- Décision : **modification rapide et brouillon local**. (a) Modification rapide : un
  formulaire de fiche envoyé par la route de la ressource (changeset d'une opération,
  motif, `If-Match`). (b) Brouillon : les opérations s'accumulent dans le navigateur
  (IndexedDB, par utilisateur et onglet), la simulation est relancée à chaque
  changement (différentiel, anomalies), l'ensemble est soumis atomiquement. Le
  brouillon ne quitte le navigateur qu'à la soumission ; les plugins y ajoutent des
  opérations par le SDK.
- Pourquoi : un changement unitaire reste simple ; un changement de structure se
  prépare et se vérifie avant d'être soumis ; l'API n'a pas à changer.
- Rejeté : brouillon seul (lourd pour un changement unitaire) ; brouillon serveur
  partagé (rouvre 004 et 006 : schéma, droits, purge).
- Conséquence : le brouillon contient des données du référentiel ; il est effacé à la
  déconnexion et rattaché à l'identité, jamais relu par un autre utilisateur.
- Décidé par : recommandation acceptée

## W11 — Mécanique de l'i18n
- Décision : **vue-i18n dans le shell, exposé par le SDK**. Catalogues JSON par langue ;
  le SDK fournit `locale`, `t()` et les formats (`Intl`, fuseau affiché en clair). Chaque
  plugin livre ses catalogues, déclarés dans son manifeste, sous son espace de noms.
  Langue : choix de l'utilisateur gardé localement, sinon celle du navigateur, sinon
  l'anglais. Les erreurs de l'API se traduisent par leur `type` (Problem Details),
  `detail` brut en repli. Un test vérifie que FR et EN ont les mêmes clés.
- Pourquoi : pluriels, interpolation et formats déjà résolus ; l'API reste neutre en
  langue (006 inchangée).
- Rejeté : i18n maison (pluriels et interpolation à réécrire) ; messages traduits par
  le serveur (`Accept-Language`, touche la 006, le shell a de toute façon ses textes).
- Décidé par : recommandation acceptée

## W12 — Composants d'interface
- Décision : **primitives sans style et jetons CSS**. Reka UI pour les primitives
  accessibles (dialogues, menus, onglets, listes déroulantes) ; CSS propre fondé sur des
  jetons de design (propriétés personnalisées, modes clair et sombre) qui traversent le
  Shadow DOM, donc hérités par les plugins sans dépendre de Vue ; TanStack Virtual pour
  les longues listes et l'arbre.
- Pourquoi : accessibilité des composants interactifs acquise ; peu de dépendances ;
  un rendu dense adapté à un référentiel réseau ; cohérence offerte aux plugins non Vue.
- Rejeté : bibliothèque complète (PrimeVue, Vuetify : thème à surcharger, lourde,
  inutilisable par un plugin non Vue) ; tout maison (accessibilité à réécrire).
- Décidé par : recommandation acceptée

## W13 — Budgets du navigateur
- Décision : **budgets bloquants en CI**, mesurés par Playwright sur le jeu à 10⁶
  objets, CPU ralenti ×4 : shell initial ≤ 300 Ko gzip hors plugins (découpé par
  route) ; premier écran utile ≤ 2 s après connexion ; rendu d'une page de 100 lignes
  ≤ 100 ms après la réponse ; arbre à 10⁵ sous-réseaux chargé à la demande et
  virtualisé, défilement sans tâche longue (> 50 ms) ; un plugin en échec ou au-delà de
  5 s de chargement est écarté sans bloquer le shell.
- Pourquoi : D14 ne couvre que le serveur ; un budget non bloquant dérive.
- Rejeté : mesures non bloquantes ; pas de budget en v0.1.
- Décidé par : recommandation acceptée

## W14 — Accessibilité
- Décision : **WCAG 2.2 niveau AA** (base de l'EN 301 549 et du RGAA) : navigation
  complète au clavier, arbre et tables compris ; contrastes vérifiés sur les jetons ;
  axe-core bloquant dans chaque parcours Playwright. Les plugins de première partie y
  sont soumis ; pour les tiers, c'est une recommandation.
- Pourquoi : outil de travail quotidien, exigence fréquente des marchés publics ; un
  contrôle automatique bloquant évite la dérive.
- Rejeté : bonnes pratiques sans cible (aucun engagement vérifiable).
- Décidé par : recommandation acceptée
- Correction (2026-10-19, [note de référence](../../docs/reference/plateforme-web.md)) :
  l'EN 301 549 V3.2.1 et le RGAA 4.1.2, en vigueur, reposent sur WCAG 2.1 ; WCAG 2.2 AA
  est une cible compatible avec eux, non leur base. axe-core ne couvre de WCAG 2.2 que
  2.5.8, par une règle désactivée par défaut (REQ-007-53).

## W15 — Tests de bout en bout
- Décision : **contre le vrai serveur et PostgreSQL** de devenv : parcours Playwright
  avec un émetteur OIDC de développement (W16) et un jeu de données chargé par l'API.
  Vitest couvre composants et SDK avec un `fetch` simulé, typé par le contrat.
- Pourquoi : cookie, CSP, SRI, CSRF et import map ne s'éprouvent qu'avec le vrai
  serveur ; une API simulée dérive du contrat.
- Rejeté : API simulée (MSW) pour les E2E (sécurité non éprouvée, dérive) ; mélange
  simulé et fumée réelle (deux jeux de parcours à maintenir).
- Décidé par : recommandation acceptée

## W16 — Émetteur OIDC de développement
- Décision : **binaire de développement propre**, crate `kartenn-dev-idp` tiré de
  l'émetteur de `crates/kartenn-rest/tests/sessions.rs`, jamais livré : découverte,
  JWKS, `authorize` avec choix de l'utilisateur, `token` ; utilisateurs et groupes en
  TOML ; processus devenv.
- Pourquoi : groupes et cas d'erreur maîtrisés pour éprouver le RBAC par groupe ; pas
  de dépendance nouvelle.
- Rejeté : Dex (utilisateurs statiques sans groupes) ; comptes de secours seuls
  (parcours OIDC du navigateur jamais éprouvé).
- Décidé par : recommandation acceptée

## W17 — Choix de conduite tranchés sans question
Peu coûteux à défaire, invisibles de l'extérieur ; à reprendre dans le design.
- **Changements de l'API** : additifs seulement (006/H12). `ui_plugins` gagne l'URL du
  point d'entrée, l'empreinte SRI, la plage de SDK, les points d'extension et les
  catalogues ; la configuration gagne `[web]` (`root`, `plugins_dir`) ; le serveur sert
  le shell (repli sur `index.html` pour les routes du client) et pose CSP, `nosniff`,
  `frame-ancestors 'none'` et les en-têtes de cache (assets à empreinte `immutable`).
- **Droits dans l'interface** : `GET /v1/me` (liaisons, permissions globales) sert à
  masquer ou griser ; c'est une aide, le serveur reste seul juge (403 traité).
- **Session** : un 401 ouvre une reconnexion qui ramène à la même URL ; le brouillon
  survit (W10). Déconnexion : brouillon et préférences sensibles effacés.
- **État** : l'URL et des composables Vue ; pas de magasin global (Pinia) tant qu'aucun
  besoin ne le montre. Routage : vue-router.
- **Navigateurs** : deux dernières versions de Firefox, Chromium et Safari, et Firefox
  ESR (import maps et Custom Elements natifs) ; E2E sur Chromium et Firefox.
- **Développement** : Vite relaie `/v1` vers kartenn-server pour garder une seule
  origine ; le processus devenv `web` existe déjà.
- **Licences** : `web/sdk` et `web/plugins/*` en MPL-2.0 (en-tête par fichier), `web/shell`
  en AGPL-3.0.
- Décidé par : Claude (délégation des choix réversibles)

## Arbitrages des questions ouvertes des exigences

Le 2026-10-19, l'utilisateur a délégué les questions ouvertes des exigences : « poursuis au
maximum en fonction de tes recommandations ». W18 à W27 appliquent les recommandations de
`requirements.md` et de la [note de référence](../../docs/reference/plateforme-web.md).

## W18 — Ajouts à l'API pour le shell
- Décision : en plus de W17, `GET /v1/me` gagne `permissions` (entrées des rôles tenus sur
  au moins un périmètre, motifs compris) ; `GET /v1/auth/issuers`, sans authentification,
  donne `id` et `label` des émetteurs ; `[[auth.issuers]]` gagne `label` (défaut : `id`).
- Pourquoi : sans eux, le shell ne peut ni filtrer les extensions (REQ-007-19) ni
  proposer la connexion par émetteur (REQ-007-44) ; ajouts seulement (006/H12).
- Rejeté : liste des émetteurs injectée dans `index.html` (hors contrat, inutile au CLI) ;
  actions permises par objet dans chaque représentation (touche toute la 006).
- Décidé par : Claude, par délégation de l'utilisateur

## W19 — CSP des styles
- Décision : **`style-src 'self'` strict** : le CSS du shell est extrait au build, les
  plugins livrent des fichiers CSS (`<link>` dans leur racine d'ombre ou feuilles
  construites), les plugins Vue de première partie extraient leur CSS. Le repli
  `style-src-attr 'unsafe-inline'` n'est admis que si une bibliothèque du shell l'exige,
  constaté par les violations relevées en E2E, et se consigne.
- Pourquoi : pas d'injection CSS ; la contrainte pèse sur le build, pas sur l'usage.
- Rejeté : `'unsafe-inline'` pour tous les styles (injection CSS possible).
- Décidé par : Claude, par délégation de l'utilisateur

## W20 — `index.html` rendu par kartenn
- Décision : kartenn produit `index.html` à partir du gabarit du build : une seule import
  map (`@kartenn/sdk` et la clé `integrity` de chaque module du shell et des plugins
  retenus), un bloc de données JSON des plugins retenus, et la CSP qui autorise l'import
  map par l'empreinte SHA-256 de ses octets. Un document porte ainsi un ensemble de
  plugins figé ; le shell ne charge que ceux de son document.
- Pourquoi : `import()` n'est contrôlé que par l'import map, unique et figée avant le
  premier import ; une import map figée au build ne couvrirait pas les plugins à chaud.
- Rejeté : import map figée au build ; `modulepreload` seul (ne couvre pas les fragments) ;
  nonce (unique par réponse, sans gain sur l'empreinte).
- Décidé par : Claude, par délégation de l'utilisateur

## W21 — Plugins réservés aux sessions
- Décision : sans session valide, `index.html` ne liste aucun plugin et `/plugins/…`
  répond 404 ; les fichiers du shell restent publics (page de connexion).
- Pourquoi : ne pas livrer l'inventaire, les versions ni le code des plugins à un
  visiteur anonyme ; la connexion est une navigation complète, qui recharge le document.
- Rejeté : plugins publics (révèlent ce qui est installé et ses versions).
- Décidé par : Claude, par délégation de l'utilisateur

## W22 — Effacement à la déconnexion
- Décision : le shell efface ses bases IndexedDB et celles des plugins (énumération de
  l'origine), Cache Storage et `localStorage` dans chaque onglet (`versionchange`,
  `BroadcastChannel`), puis appelle la déconnexion, dont la réponse porte
  `Clear-Site-Data: "storage"` ; il réécrit ensuite langue et thème. Pas de
  `navigator.storage.persist()` (Firefox le demande à l'utilisateur).
- Pourquoi : deux couches, l'une couvrant les défauts de l'autre ; `"storage"` est pris
  en charge par toute la cible (note, point 8).
- Rejeté : `"cache"`, `"*"` (blocages de Chromium), `"executionContexts"` (plus pris en
  charge).
- Décidé par : Claude, par délégation de l'utilisateur

## W23 — Navigateurs des mesures et des parcours
- Décision : budgets (W13) mesurés sur Chromium seul (ralentissement du CPU, tâches
  longues) ; parcours fonctionnels sur Chromium et Firefox (W17) ; les parcours
  d'intégrité et de CSP aussi sur WebKit si le Playwright de Nix le fournit, sinon le
  manque est consigné comme risque.
- Pourquoi : Safari diverge sur l'intégrité des modules ; les mesures de performance
  n'existent que sur Chromium.
- Décidé par : Claude, par délégation de l'utilisateur

## W24 — Surface publique du SDK
- Décision : `@microsoft/api-extractor` (dépendance de développement) produit le
  rapport de la surface publique (`web/sdk/etc/sdk.api.md`) ; un script fait échouer la
  CI si une ligne du rapport de référence est retirée ou modifiée sans version majeure.
  Le branchement du shell sur le SDK est marqué `@internal`, hors surface.
- Pourquoi : atteignabilité depuis le point d'entrée et retrait des déclarations
  internes déjà résolus ; aucun outil ne classe une modification selon SemVer, le script
  reste prudent.
- Rejeté : comparaison des `.d.ts` émis par `tsc` (déclarations internes incluses).
- Décidé par : Claude, par délégation de l'utilisateur

## W25 — Livraison
- Décision : la 007 livre le répertoire de build du web (`web/dist/` : gabarit, fichiers
  à empreinte, plugins de première partie sous `plugins/`), que `[web] root` désigne.
  Les paquets OCI et le module NixOS de D35 ne sont pas encore spécifiés : ils
  l'embarqueront. **D35 est amendée** : l'image « web statique » disparaît, le build est
  servi par kartenn-server (W2).
- Pourquoi : W2 sert le shell par le serveur ; aucun paquet n'existe à ce jour.
- Décidé par : Claude, par délégation de l'utilisateur ; amendement de D35 à confirmer
  par l'utilisateur à la relecture.

## W26 — Développement et parcours
- Décision : en développement, Vite (http, `127.0.0.1:5173`) relaie `/v1` et `/plugins`
  à kartenn-server en ajoutant `Forwarded: proto=https`, la boucle locale étant proxy de
  confiance de la configuration de développement ; il sert son propre `index.html` avec
  une import map de développement, sans intégrité. Les parcours E2E tournent sur le
  build servi par kartenn-server en TLS (certificat de développement), seul chemin qui
  éprouve CSP, intégrité et import map.
- Pourquoi : HMR en développement ; la vérité de la sécurité dans les E2E (W15).
- Risque : un navigateur qui refuserait un cookie `Secure` sur `http://127.0.0.1` ;
  repli : Vite en https avec un certificat local.
- Décidé par : Claude, par délégation de l'utilisateur

## W27 — Choix de rédaction des exigences
- Décision : validés tels qu'écrits dans `requirements.md` : `Referrer-Policy:
  same-origin`, `form-action 'self'` ; fichiers des plugins en `no-cache` avec ETag ;
  plugin invalide écarté sans refuser le démarrage ; noms `k` et `kartenn` réservés,
  noms de Custom Elements déclarés au manifeste ; relecture toutes les 30 s et sur
  `SIGHUP` ; sous-ensemble des plages node-semver ; SDK publié en `1.0.0` ; brouillon
  effacé après 7 jours ; `incomplete` d'axe-core non bloquant ; émetteur de
  développement sur la boucle locale ; plugin d'exemple non livré. S'y ajoutent des
  bornes : fichier d'un plugin ≤ 8 Mio, plugin ≤ 32 Mio, ensemble ≤ 128 Mio.
- Décidé par : Claude, par délégation de l'utilisateur
