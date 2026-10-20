---
status: draft
updated: 2026-10-20
---

# 007 — Shell Web et plugins : exigences

## Résumé

Le shell Web est l'interface de kartenn dans le navigateur : Vue 3 et TypeScript (D17),
client de la seule API REST (D7), servi par kartenn-server sur la même origine que l'API
(W2). Il couvre les données du noyau, le cycle des changesets et l'administration (W1),
avec un contexte temporel porté par l'URL (W9), une écriture rapide ou par brouillon local
simulé (W10) et une interface en français et en anglais (W11). Il accueille à chaud des
plugins UI (D6) : modules ES déposés par l'exploitant dans un répertoire (W3), code de
confiance (W4), branchés sur quatre points d'extension (W5) par un SDK public partagé
(W7, W8). Les écrans des modules métier sont des plugins de première partie (W6). Des
budgets de navigateur (W13) et l'accessibilité WCAG 2.2 AA (W14) sont vérifiés en
bloquant ; les parcours de bout en bout tournent contre le vrai serveur (W15) avec un
émetteur OIDC de développement (W16).

Décisions : [`decisions.md`](decisions.md) (W1–W18) ; D6, D7, D14, D17, D31, D37
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; Q-ouv-1 (SDK et
plugins en MPL-2.0), Q-ouv-6 (i18n de l'interface) ; ADR 0011, 0020, 0022, 0024.
Contrat de l'API : [`../006-api-rest/openapi-v1.json`](../006-api-rest/openapi-v1.json).
Sources : `docs/reference/web-navigateur.md` (à écrire, tâche 1).

### Vocabulaire

- **Shell** : application Vue servie à la racine de l'origine, hors `/v1`, `/plugins` et
  `/health`.
- **Plugin** : répertoire versionné de modules ES, de feuilles de style et de catalogues,
  décrit par un manifeste, qui enregistre des Custom Elements.
- **Manifeste** : `manifest.json` d'un plugin : nom, version, plage de SDK, point
  d'entrée, points d'extension, catalogues, module requis.
- **Point d'extension** : emplacement du shell où un plugin insère un élément (W5) : vue,
  onglet, panneau ou colonne, action.
- **Plugin de première partie** : plugin d'un module métier compilé, écrit avec le SDK
  public et livré avec kartenn (W6).
- **SDK** : paquet `@kartenn/sdk`, contrat public versionné des plugins (W7).
- **Contexte temporel** : couple (`valid_at`, `as_of`) de la vue ; absent, il vaut
  « maintenant » (D10).
- **Présent** : contexte temporel sans `valid_at` ni `as_of`.
- **Modification rapide** : écriture d'un objet par la route de sa ressource (006/H2).
- **Brouillon** : opérations accumulées dans le navigateur, simulées en continu et
  soumises en un seul changeset (W10).
- **Jetons de design** : propriétés CSS personnalisées du thème, héritées à travers le
  Shadow DOM (W12).
- **Écarté** : se dit d'un plugin que le shell n'a pas chargé (incompatible, en échec ou
  trop lent) ; le shell continue sans lui.

### Conventions

- Forme EARS en français, comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).
- « kartenn » désigne le serveur, « le shell » le code exécuté dans le navigateur.
- Les références entre crochets renvoient aux décisions (W*, D*) et aux sources :
  [CSP3] Content Security Policy Level 3, [SRI] Subresource Integrity, [HTML] HTML
  Living Standard (import maps, `modulepreload`, Custom Elements), [DOM] Shadow DOM,
  [WCAG 2.2], [APG] ARIA Authoring Practices, [RFC 6265bis] cookies, [Fetch Metadata].
  Les sections précises sont fixées par la note de référence (tâche 1).
- Les REQ-006-* citées sont celles de [`../006-api-rest/requirements.md`](../006-api-rest/requirements.md).

## Exigences

### A. Service du shell

#### REQ-007-1 — Shell servi par kartenn
Quand la configuration porte `[web] root`, kartenn doit servir les fichiers de ce
répertoire sur la même origine que l'API ; une requête `GET` ou `HEAD` hors de `/v1`,
`/plugins` et `/health` dont le chemin n'a pas d'extension et ne désigne aucun fichier
doit recevoir `index.html` (routes du client). [W2, W17]

Acceptation :
- `GET /prefixes/{uuid}` → 200, `text/html`, contenu de `index.html`.
- `GET /v1/inconnu` → 404 `application/problem+json` (inchangé) ; `GET /assets/absent.js`
  → 404.
- `GET /../etc/passwd`, `/%2e%2e/…` ou un fichier caché (`/.env`) → 404.
- Sans `[web]`, tout chemin hors `/v1` et `/health` répond 404 comme aujourd'hui.

#### REQ-007-2 — En-têtes du shell
kartenn doit poser, sur toute réponse servie hors de `/v1` et `/health`, une CSP stricte
(`script-src 'self'` plus l'empreinte de l'import map du shell, sans `unsafe-inline` ni
`unsafe-eval` ; `connect-src 'self'` ; `frame-ancestors 'none'` ; aucun cadre ni objet),
`X-Content-Type-Options: nosniff` et des en-têtes de cache : `immutable` pour les fichiers
à empreinte du build, revalidation pour `index.html` et les fichiers des plugins.
[W2, W4, W17, CSP3, RFC 8246]

Acceptation :
- La CSP de `index.html` contient `'sha256-…'` égal à l'empreinte de son import map ;
  un build remplacé sans redémarrage donne la nouvelle empreinte à la requête suivante.
- `GET /assets/index-<empreinte>.js` → `Cache-Control: public, max-age=31536000,
  immutable` ; `GET /` → `Cache-Control: no-cache` et `ETag`.
- Un script en ligne ajouté à `index.html` est refusé par Chromium et Firefox (violation
  CSP relevée par le parcours).

#### REQ-007-3 — Configuration `[web]`
kartenn doit lire une section `[web]` (`root` et `plugins_dir`, chacun facultatif ; sans
`root`, seuls les plugins sont servis, comme en développement derrière Vite) et refuser
au démarrage un champ inconnu, un `root` sans `index.html` ou un `index.html` sans import
map. [W2, W3, W17]

Acceptation :
- `[web] rooot = …` → refus au démarrage, message qui nomme le champ.
- `root` sans `index.html` → refus au démarrage.

### B. Plugins

#### REQ-007-4 — Répertoire des plugins
Quand `[web] plugins_dir` est configuré, kartenn doit servir chaque plugin
`<plugins_dir>/<nom>/<version>/` sous `/plugins/<nom>/<version>/` et publier, pour chaque
nom, la plus haute version valide ; les autres versions présentes restent servies pour
les onglets déjà ouverts. [W3]

Acceptation :
- `example/1.0.0` et `example/1.1.0` présents → `ui_plugins` publie `1.1.0` ;
  `/plugins/example/1.0.0/index.js` reste servi.
- Un chemin qui sort du répertoire de la version → 404.

#### REQ-007-5 — Manifeste
kartenn doit valider le manifeste de chaque plugin (nom égal au répertoire, version
semver égale au répertoire, plage de SDK, point d'entrée présent, points d'extension
connus, noms d'éléments préfixés par le nom du plugin, catalogues présents) ; si un
manifeste est invalide, alors kartenn doit écarter ce plugin, le journaliser
(`web.plugin.rejected`, motif) et servir les autres. [W3, W5]

Acceptation :
- Un manifeste par motif de refus (version différente du répertoire, point inconnu,
  élément `autre-onglet` dans le plugin `example`, catalogue absent…) → plugin absent de
  `ui_plugins`, un log par plugin écarté, les autres publiés.

#### REQ-007-6 — Empreintes SRI
kartenn doit calculer l'empreinte SRI (`sha384`) de chaque module, feuille de style et
catalogue d'un plugin et la publier ; le shell doit charger chacun de ces fichiers avec
son empreinte, de sorte qu'un fichier modifié après la lecture du répertoire soit refusé
par le navigateur. [W3, W4, SRI]

Acceptation :
- L'empreinte publiée égale `openssl dgst -sha384 -binary | base64` du fichier.
- Un module réécrit après la lecture du répertoire → plugin écarté par le shell, le
  shell reste utilisable.

#### REQ-007-7 — Plugins dans les capacités
kartenn doit publier dans `ui_plugins` de `GET /v1/capabilities`, pour chaque plugin
retenu : nom, version, URL du point d'entrée, empreintes, plage de SDK, points
d'extension et catalogues ; ces membres s'ajoutent au contrat sans en retirer. [W3, W17,
REQ-006-33, 34, 35]

Acceptation :
- Le comparateur du contrat (006, tâche 22) passe : seuls des membres sont ajoutés.

#### REQ-007-8 — Rechargement à chaud
Quand l'exploitant ajoute, met à jour ou retire un plugin, kartenn doit refléter le
répertoire dans `ui_plugins` sans reconstruction ni redémarrage, au plus tard 10 s après,
ou dès la réception de `SIGHUP`. [W3, D6]

Acceptation :
- Dépôt d'une version 1.2.0 → publiée en ≤ 10 s ; après `SIGHUP`, dès la réponse
  suivante ; répertoire retiré → plugin absent en ≤ 10 s.
- Le shell ouvert après la mise à jour charge la nouvelle version.

#### REQ-007-9 — Chargement par le shell
Le shell doit charger chaque plugin publié dont la plage de SDK accepte sa version ; si
la plage est incompatible, si le chargement échoue, ou s'il n'est pas terminé en 5 s,
alors le shell doit écarter ce plugin sans bloquer le reste et le signaler (page « À
propos » : plugins chargés et écartés, avec le motif). [W7, W13]

Acceptation :
- Plugins de test : compatible, `sdk: "^9"`, empreinte fausse, exception au chargement,
  enregistrement jamais fait → seul le premier est actif ; le premier écran utile tient
  son budget (REQ-007-32) ; chaque motif est affiché.

#### REQ-007-10 — Plugin, code de confiance
Le shell doit exécuter un plugin dans le même document, avec tous les droits de
l'utilisateur et aucun de plus ; aucune origine tierce ne doit être joignable ni
chargeable (CSP). La documentation de l'exploitant doit dire qu'installer un plugin
revient à lui confier la session de chaque utilisateur. [W4, INV-23]

Acceptation :
- Un plugin de test qui appelle `fetch('https://ailleurs.example/')` ou charge un script
  tiers est bloqué par la CSP.
- La description de `[web] plugins_dir` dans la spec système (`docs/spec/interfaces.md`,
  qui tient lieu de documentation d'exploitation tant qu'il n'y en a pas d'autre) porte
  l'avertissement.

#### REQ-007-11 — Plugins de première partie
kartenn doit livrer les plugins des modules métier compilés (`web/plugins/<module>/`),
écrits avec le seul SDK public, et ne publier un plugin dont le manifeste nomme un module
que si ce module figure dans `modules` des capacités ; le shell ne doit dépendre d'aucun
module. [W6]

Acceptation :
- Un plugin de test qui déclare `module: "vlan"` sur une build sans ce module → absent de
  `ui_plugins`.
- Le build du shell n'importe rien de `web/plugins/`.

### C. SDK

#### REQ-007-12 — Paquet `@kartenn/sdk`
kartenn doit fournir le paquet `@kartenn/sdk` (MPL-2.0), versionné en semver : types et
client de l'API, contexte, enregistrement des extensions, i18n, brouillon, jetons de
design et bac à sable de test ; à l'exécution, le shell doit le servir comme une instance
unique, désignée par son import map, que les plugins importent sans l'embarquer. Le SDK
ne doit dépendre d'aucun framework. [W7, W12, Q-ouv-1]

Acceptation :
- Deux plugins qui importent `@kartenn/sdk` voient le même contexte (même objet).
- Un plugin écrit sans Vue (Custom Elements nus) utilise toutes les fonctions du SDK.

#### REQ-007-13 — Points d'extension
Le SDK doit offrir quatre points d'extension, déclarés au manifeste et enregistrés à
l'exécution : (1) vue pleine page avec entrée de navigation, (2) onglet de la fiche d'un
type d'objet, (3) panneau de fiche ou colonne de liste d'un type d'objet, (4) action du
menu d'un objet ; le shell ne doit montrer une extension que pour les types d'objet
qu'elle déclare et, si elle nomme une permission, que si l'utilisateur la détient. [W5]

Acceptation :
- Un onglet déclaré pour `prefix` n'apparaît pas sur une adresse.
- Une action qui demande `core.address.write` est absente pour un lecteur.
- Un enregistrement d'élément non déclaré au manifeste est refusé.

#### REQ-007-14 — Contexte offert aux plugins
Le SDK doit exposer aux extensions l'utilisateur (identité, nom, permissions résumées),
le contexte temporel, l'objet courant (type et identifiant), la langue, et notifier leurs
changements. [D17, W9, W17]

Acceptation :
- Un changement de `valid_at` dans l'URL est notifié à l'onglet affiché, qui relit ses
  données.

#### REQ-007-15 — Client de l'API
Le SDK doit fournir un client typé par les types tirés du contrat de référence
(openapi-typescript, sans code généré à l'exécution), qui gère `QUERY`, `ETag` et
`If-Match` (exigé par le type pour `PATCH` et `DELETE` d'un objet), `Idempotency-Key`,
les problèmes (RFC 9457) et les curseurs ; les types régénérés doivent être comparés au
contrat de référence par un test. [W8, REQ-006-7, 10, 14, 15, 20]

Acceptation :
- Un contrat modifié sans régénération des types → test en échec.
- Un 412 donne une erreur typée qui porte le `type` du problème et le `Request-Id`.
- L'itération d'une liste suit les curseurs jusqu'à la dernière page.

#### REQ-007-16 — Bac à sable de test
Le SDK doit fournir un bac à sable qui monte l'élément d'une extension avec un contexte
choisi et un `fetch` simulé typé par le contrat, pour les tests de plugins. [W15]

Acceptation :
- Les tests Vitest du plugin d'exemple passent par ce bac à sable.

### D. Écrans

#### REQ-007-17 — Données du noyau
Le shell doit permettre de consulter les domaines, l'arbre des préfixes, les plages, les
adresses, les entités et les sites : listes filtrées et paginées par curseur, fiche
d'objet (attributs, propriétés résolues et leur explication, historique, anomalies),
occupation et prochain libre d'un préfixe, anomalies et leur acquittement. [W1, REQ-006-4]

Acceptation :
- Parcours : d'un domaine à un préfixe par l'arbre, puis à une adresse, sa propriété
  héritée expliquée, son historique.

#### REQ-007-18 — Changesets
Le shell doit montrer les changesets (liste filtrée, différentiel, état) et permettre,
selon les droits, leur validation, rejet, retrait et annulation, ainsi que les
propositions en attente de l'utilisateur. [W1, 004]

Acceptation :
- Parcours : une proposition soumise par un utilisateur est validée par un autre, puis
  annulée ; chaque étape est visible dans la liste.

#### REQ-007-19 — Administration
Le shell doit permettre d'administrer les rôles, liaisons, rattachements d'émetteurs,
comptes de service et secrets (montrés une seule fois), d'examiner les identités (état,
pseudonymisation), de rechercher et d'exporter l'audit, et de suivre les imports. [W1, 005,
012, REQ-006-17 à 19]

Acceptation :
- Parcours : création d'un compte de service et d'un secret, secret affiché une fois puis
  plus jamais ; recherche d'audit filtrée qui retrouve l'événement de la création.

#### REQ-007-20 — Droits dans l'interface
Le shell doit masquer ou griser ce que `GET /v1/me` dit hors des droits de l'utilisateur ;
le serveur restant seul juge, un 403 ou une violation `core.object.forbidden` doit être
présenté comme un refus, sans perte de saisie. [W17, REQ-006-23]

Acceptation :
- Un lecteur ne voit pas « Modifier » ; une requête forcée reçoit 403 et le shell
  l'affiche.

#### REQ-007-21 — Plugin d'exemple
kartenn doit fournir un plugin d'exemple (`web/plugins/example/`), écrit sans framework,
qui utilise les quatre points d'extension, ses propres catalogues, le client et le
brouillon. [W1, W7]

Acceptation :
- Le parcours de bout en bout du plugin d'exemple passe sur Chromium et Firefox.

### E. Contexte temporel

#### REQ-007-22 — Contexte dans l'URL
Le shell doit porter `valid_at`, `as_of` et les filtres de la vue dans l'URL, de sorte
qu'un lien ouvert par un autre utilisateur montre la même vue, aux droits près. [W9]

Acceptation :
- Un lien copié avec `valid_at` et `as_of` passés, ouvert dans un autre navigateur,
  affiche les mêmes valeurs.

#### REQ-007-23 — Bandeau hors présent
Tant que la vue n'est pas au présent, le shell doit afficher un bandeau permanent qui
nomme le contexte et ramène au présent d'un clic ; un `as_of` doit désactiver toute
écriture ; un `valid_at` hors présent reste une lecture, l'écriture au futur passant par
la date d'effet d'un changeset. [W9]

Acceptation :
- Avec `as_of`, les boutons d'écriture sont désactivés et la soumission du brouillon
  refusée par le shell.
- Le clic sur le bandeau retire `valid_at` et `as_of` de l'URL.

#### REQ-007-24 — Plugins et contexte temporel
Le SDK doit donner aux plugins le contexte temporel et faire porter par son client les
paramètres `valid_at` et `as_of` aux lectures datées. [W9]

Acceptation :
- L'onglet du plugin d'exemple affiche la valeur au `valid_at` de l'URL.

### F. Écriture

#### REQ-007-25 — Modification rapide
Le shell doit permettre de créer, modifier ou clore un objet par sa route de ressource,
avec un motif obligatoire et l'`If-Match` de la lecture ; un 412 doit proposer de relire
l'objet en gardant la saisie, un 202 mener au changeset en attente, un 422 placer les
violations sur les champs. [W10, REQ-006-12 à 14, 21]

Acceptation :
- Deux onglets modifient le même préfixe : le second reçoit 412 et retrouve sa saisie
  après relecture.

#### REQ-007-26 — Brouillon local simulé
Le shell doit accumuler les opérations d'un brouillon dans le navigateur (IndexedDB, par
utilisateur et par onglet), relancer la simulation à chaque changement (différentiel,
violations, cascades), et soumettre l'ensemble en un seul changeset atomique, avec une
clé d'idempotence gardée jusqu'à la réponse ; rien ne quitte le navigateur avant la
soumission, hors simulations. Les plugins doivent pouvoir ajouter des opérations au
brouillon par le SDK. [W10, REQ-004-9, REQ-006-11, 15]

Acceptation :
- Trois opérations, dont une invalide : la simulation montre la violation ; corrigée, le
  changeset est appliqué en une fois.
- Coupure du réseau pendant la soumission puis nouvel essai → un seul changeset créé.

#### REQ-007-27 — Brouillon et identité
Le shell doit rattacher le brouillon à l'identité, le garder à travers une reconnexion de
la même identité, l'effacer à la déconnexion, et ne jamais le montrer à une autre
identité. [W10, W17]

Acceptation :
- Session expirée, reconnexion → brouillon intact ; déconnexion → IndexedDB vide de ce
  brouillon ; un autre utilisateur dans le même navigateur ne le voit pas.

### G. Session

#### REQ-007-28 — Connexion et reconnexion
Le shell doit offrir la connexion par les émetteurs configurés et par compte de secours ;
quand l'API répond 401, il doit proposer une reconnexion qui ramène à la même URL ; la
déconnexion doit effacer le brouillon et les préférences sensibles avant d'appeler
`POST /v1/auth/logout`. [W17, REQ-006-24, 25]

Acceptation :
- Session forcée expirée sur une fiche : reconnexion → retour sur la même fiche, même
  contexte temporel.

### H. Langue

#### REQ-007-29 — i18n
Le shell doit afficher ses textes en français et en anglais (vue-i18n), choisir la langue
de l'utilisateur gardée localement, sinon celle du navigateur, sinon l'anglais, formater
dates et nombres par `Intl` en nommant le fuseau affiché, traduire les erreurs de l'API
par leur `type` avec `detail` brut en repli, et charger les catalogues de chaque plugin
sous son espace de noms. Les catalogues FR et EN du shell et des plugins de première
partie doivent avoir les mêmes clés. [W11, Q-ouv-6]

Acceptation :
- Test : clés FR = clés EN, pour le shell et chaque plugin de `web/plugins/`.
- Un problème d'un `type` sans traduction affiche son `detail`.

### I. Rendu

#### REQ-007-30 — Jetons de design
Le shell doit définir son thème par des jetons de design (modes clair et sombre) hérités
par les éléments des plugins à travers le Shadow DOM, et bâtir ses composants interactifs
sur des primitives accessibles (Reka UI). [W12]

Acceptation :
- Un élément de plugin en Shadow DOM prend la couleur de fond du mode sombre sans code
  propre.

#### REQ-007-31 — Listes longues et arbre
Le shell doit virtualiser les longues listes et l'arbre, et charger les enfants d'un nœud
à la demande, page par page. [W12, W13]

Acceptation :
- Un conteneur à 10⁵ enfants directs n'a jamais plus de quelques centaines de nœuds dans
  le DOM.

### J. Budgets

#### REQ-007-32 — Budgets du navigateur
Le shell doit tenir, mesuré par Playwright sur le jeu à 10⁶ objets, CPU ralenti ×4 :
shell initial ≤ 300 Ko gzip hors plugins (découpé par route) ; premier écran utile ≤ 2 s
après la connexion ; rendu d'une page de 100 lignes ≤ 100 ms après la réponse ; arbre à
10⁵ sous-réseaux défilé sans tâche longue (> 50 ms) ; un plugin écarté au-delà de 5 s
sans retarder le shell. Le banc doit échouer si un budget est dépassé. [W13, D14]

Acceptation :
- Banc `bench-web` : chaque mesure sous son budget, sortie non nulle sinon.

### K. Accessibilité

#### REQ-007-33 — WCAG 2.2 AA
Le shell et les plugins de première partie doivent satisfaire WCAG 2.2 niveau AA :
navigation complète au clavier, arbre et tables compris ; contrastes des jetons vérifiés
dans les deux modes ; axe-core sans violation dans chaque parcours Playwright, en
bloquant. [W14, WCAG 2.2, APG]

Acceptation :
- Un parcours entier au clavier seul : arbre (flèches, Origine, Fin), table, dialogue,
  menu d'actions.
- Un test calcule le contraste de chaque paire de jetons texte et fond (≥ 4,5:1, ≥ 3:1
  pour les composants).

### L. Tests et outillage

#### REQ-007-34 — Parcours de bout en bout
Les parcours Playwright doivent tourner contre le vrai kartenn-server et PostgreSQL,
avec l'émetteur de développement et un jeu de données chargé par l'API, sur Chromium et
Firefox ; composants et SDK sont testés par Vitest avec un `fetch` simulé typé par le
contrat. [W15, W17]

Acceptation :
- `k-web-e2e` lance serveur, émetteur et parcours, sur les deux navigateurs.

#### REQ-007-35 — Émetteur OIDC de développement
kartenn doit fournir un émetteur OIDC de développement (crate `kartenn-dev-idp`) :
découverte, JWKS, `authorize` avec choix de l'utilisateur, `token`, utilisateurs et
groupes lus en TOML ; il ne doit écouter que sur le bouclage et n'être livré dans aucun
paquet. [W16]

Acceptation :
- Connexion du navigateur par l'émetteur, groupes du TOML visibles dans `/v1/me`.
- Adresse d'écoute hors bouclage → refus au démarrage.

#### REQ-007-36 — Développement sur une origine
Le serveur de développement Vite doit relayer `/v1` et `/plugins` vers kartenn-server,
de sorte que le shell en développement garde une seule origine (cookies, CSRF). [W17]

Acceptation :
- `devenv up` : connexion par l'émetteur de développement à travers Vite, puis lecture
  d'une liste.

#### REQ-007-37 — Navigateurs
Le shell doit fonctionner sur les deux dernières versions de Firefox, Chromium et Safari
et sur Firefox ESR. [W17]

Acceptation :
- La cible du build n'emploie que ce que ces navigateurs ont tous ; parcours sur
  Chromium et Firefox.

#### REQ-007-38 — Licences
`web/sdk` et `web/plugins/*` doivent être sous MPL-2.0 avec l'en-tête de la licence dans
chaque fichier source, `web/shell` sous AGPL-3.0 ; un test doit le vérifier. [W17, D37,
Q-ouv-1]

Acceptation :
- Un fichier sans en-tête dans `web/sdk` → test en échec.

### M. Ajouts à l'API

Ajouts que les écrans de W1 demandent et que W17 n'énumère pas, arbitrés par W18
(Q-007-1 à 3). Additifs (006/H12), sous `/v1`.

#### REQ-007-39 — Racines de l'arbre d'un domaine
kartenn doit restituer les nœuds sans parent déduit d'un domaine (préfixes, plages et
adresses de premier niveau), datés, paginés par curseur et filtrés par les droits comme
les enfants d'un préfixe. [D34, REQ-002-42, REQ-006-5, 7, 8]

Acceptation :
- `GET /v1/domains/{id}/roots` → mêmes éléments et même ordre que les enfants d'un
  préfixe ; page de 100 sous 150 ms au p95 sur le jeu de `bench-core`.

#### REQ-007-40 — Imports de l'appelant
kartenn doit lister les imports de l'appelant, du plus récent au plus ancien, paginés par
curseur ; ceux d'un autre restent invisibles. [REQ-006-17 à 19]

Acceptation :
- `GET /v1/imports` ne rend que les imports de l'appelant.

#### REQ-007-41 — Émetteurs de connexion
kartenn doit publier, sans authentification, l'identifiant local de chaque émetteur
configuré, pour la page de connexion. [REQ-006-24]

Acceptation :
- `GET /v1/auth/issuers` sans session → 200 `[{id}]`, sans URL ni identifiant de client.

## Non-objectifs

- Écrans VLAN et Machines : plugins de première partie des specs 008 et 009 (W6).
- Vue graphique (budget D14 à 10⁴ nœuds) : plugin de visualisation ultérieur (D6).
- Plugins non fiables, isolation par iframe, permissions propres à un plugin (W4).
- Téléversement de plugins par l'API (W3) ; brouillon serveur partagé (W10) ; messages
  traduits par le serveur (W11).
- Paquets OCI et Nix : ils n'existent pas encore ; la 007 fixe la disposition du build
  qu'ils embarqueront (`web/dist/`).
- Hors ligne, application installable.
- Parcours de bout en bout sur Safari (support visé, non éprouvé en CI).

## Questions ouvertes

- [x] Q-007-1 — Racines de l'arbre (REQ-007-39) : route additive
  `GET /v1/domains/{id}/roots`, ou lecture de `GET /v1/prefixes?domain=…` en entier,
  incompatible avec un arbre à 10⁵ chargé à la demande (W13). (utilisateur) — la route,
  le 2026-10-20 (W18).
- [x] Q-007-2 — Suivi des imports (REQ-007-40) : route additive `GET /v1/imports`, ou
  suivi des seuls imports ouverts dans ce navigateur. (utilisateur) — la route, limitée à
  l'appelant, le 2026-10-20 (W18).
- [x] Q-007-3 — Émetteurs de la page de connexion (REQ-007-41) : route publique additive
  `GET /v1/auth/issuers`, ou liste `[web] issuers` qui double `[[auth.issuers]]`.
  (utilisateur) — la route publique, identifiants seuls, le 2026-10-20 (W18).
- [x] Q-007-4 — Fichiers servis hors quota par identité (design « Service du shell ») :
  le chargement d'un shell (plusieurs dizaines de fichiers, anonymes) dépasserait le
  quota anonyme par adresse (20/s). (utilisateur) — quotas sur `/v1` seulement,
  délestage sur tout, le 2026-10-20 (W18).
