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
