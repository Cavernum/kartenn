# Plate-forme Web : CSP, SRI, import maps, composants, accessibilité

Note de lecture pour la spec [007-web-shell-plugins](../../specs/007-web-shell-plugins/).
Recherche documentaire du 2026-10-19 ; ce qui n'a pas pu être relu à la source est marqué
**non vérifié**. Errata : **non vérifiés**, sauf WCAG 2.2 (voir Sources). Cette note ne
répète pas [api-http.md](api-http.md) (Fetch Metadata, cookies `__Host-`,
`Cache-Control: immutable`, RFC 8246).

**Limite de lecture.** Les hôtes w3.org, whatwg.org, rfc-editor.org, datatracker,
mozilla.org, etsi.org et semver.org étaient injoignables pendant la recherche. Les
sources ont été lues dans les dépôts publics des groupes de travail (CSP3 et SRI
`index.bs`, HTML `source`…) et dans `mdn/browser-compat-data` (BCD) : ce sont les
brouillons d'éditeur (ED) et les standards vivants, pas les instantanés datés du TR. Les
citations sont des fragments, à revérifier dans le TR avant de les figer.

Légende : **[N]** exigence normative d'une source ; **[R]** recommandation ou texte non
normatif ; **[D]** déduction de cette note, absente des sources ; **[T]** choix à
trancher. CSP3 et SRI emploient MUST/SHOULD, gardés en anglais (leur section
Conformance n'a pas été relue) ; le standard HTML écrit « must » en minuscules, avec sa
propre définition de conformité.

## Sources

Aucune RFC n'est source d'une exigence de cette note.

| Document | Statut | Consulté |
|---|---|---|
| [CSP Level 3](https://www.w3.org/TR/CSP3/) ; ED : [w3c.github.io/webappsec-csp](https://w3c.github.io/webappsec-csp/) | Working Draft ; dernier TR daté trouvé : 2026-08-13 (non ouvert) | 2026-10-19 |
| [Trusted Types](https://www.w3.org/TR/trusted-types/) ; ED : [w3c.github.io/trusted-types](https://w3c.github.io/trusted-types/dist/spec/) | Working Draft ; dernier TR daté trouvé : 2025-01-10 (non ouvert) | 2026-10-19 |
| [Subresource Integrity](https://www.w3.org/TR/SRI/) (Level 1) et [Level 2](https://www.w3.org/TR/sri-2/) ; ED Level 2 lu | Level 1 : Recommandation (2016-06-23, non ouvert) ; Level 2 : Working Draft, dernier TR daté trouvé 2026-03-20 | 2026-10-19 |
| [HTML Standard](https://html.spec.whatwg.org/multipage/) (source `whatwg/html`, `main`) | Living Standard | 2026-10-19 |
| [Fetch Standard](https://fetch.spec.whatwg.org/) (lu en partie) | Living Standard | 2026-10-19 |
| [Storage Standard](https://storage.spec.whatwg.org/) | Living Standard (review draft 2026-02) | 2026-10-19 |
| [Clear Site Data](https://www.w3.org/TR/clear-site-data/) ; ED : [w3c.github.io/webappsec-clear-site-data](https://w3c.github.io/webappsec-clear-site-data/) | Working Draft ; dernier TR daté trouvé : 2017-11-30 ; ED avec ISSUE ouvertes | 2026-10-19 |
| [Reporting API](https://w3c.github.io/reporting/) | ED (statut TR non relevé) | 2026-10-19 |
| [Indexed Database API 3.0](https://w3c.github.io/IndexedDB/) | ED | 2026-10-19 |
| [CSS Custom Properties 1](https://www.w3.org/TR/css-variables-1/) ; [CSS Cascading and Inheritance 5](https://www.w3.org/TR/css-cascade-5/) | EDs lus ; CR cités : 2022-06-16 et 2022-01-13 | 2026-10-19 |
| [WCAG 2.2](https://www.w3.org/TR/WCAG22/) (sources `w3c/wcag`) | Recommandation du 2023-10-05, republiée le 2024-12-12 (errata incorporées) ; ne remplace pas WCAG 2.0 ni 2.1 | 2026-10-19 |
| [ARIA Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/) (motifs `treeview`, `treegrid`, `grid`, dépôt `w3c/aria-practices`) | Guide non normatif ; statut formel **non vérifié** | 2026-10-19 |
| EN 301 549 V3.2.1 (2021-03) en vigueur ; V4.1.1 | V4.1.1 publiée le 2026-09-02 d'après des commentaires non primaires : **non vérifié** ; seul un projet V4.1.0 (2025-11) vu chez l'organisme | 2026-10-19 |
| RGAA 4.1.2 (base WCAG 2.1) ; RGAA 5 annoncé fin 2026 sur WCAG 2.2 | Commentaires seulement : **non vérifié** | 2026-10-19 |
| axe-core (`rule-descriptions.md` 4.14, `API.md`, `README.md`, `lib/rules/target-size.json`, branche `develop`) | Documentation de l'outil | 2026-10-19 |
| [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html) ; [node-semver, « Ranges »](https://github.com/npm/node-semver#ranges) | Spécification stable ; README de l'outil | 2026-10-19 |
| [MDN browser-compat-data](https://github.com/mdn/browser-compat-data) (`html/elements/script.json`, `link.json`, `api/CustomElementRegistry.json`, `ShadowRoot.json`, `StorageManager.json`, `http/headers/*`, `javascript/operators/import.json`, `browsers/*.json`) | Données de compatibilité, `main` au 2026-10-19 | 2026-10-19 |
| Vue : [Web Components](https://vuejs.org/guide/extras/web-components.html), [Tooling](https://vuejs.org/guide/scaling-up/tooling.html) | Documentation officielle | 2026-10-19 |

Navigateurs au 2026-10-19 (BCD) : Chrome 155, Firefox 158 (2026-10-13), Firefox ESR 153
(2026-07-21), Safari 27 (2026-09-14). Le passage de Firefox à un cycle de 2 semaines en
septembre 2026 vient de la presse (**non vérifié**) ; les dates BCD de Firefox 155 à 157
lui sont cohérentes.

## Ce que disent les sources

### CSP3

- `script-src` (`#directive-script-src`) : « The script-src directive restricts the
  locations from which scripts may be executed. » Les requêtes de script passent par le
  contrôle de requête, `import()` compris **[D]**.
- `'self'` (`#match-url-to-source-expression`) : « If url's scheme is "blob", return
  "Does Not Match". » **[N]** ; `import()` d'une URL `blob:` est donc impossible sous
  `'self'` **[D]**.
- `'unsafe-eval'` couvre `eval()`, `Function()`, `setTimeout()` et `setInterval()` à
  argument non appelable ; `'wasm-unsafe-eval'` « only permits WebAssembly and does not
  affect JavaScript ».
- En ligne (`#should-block-inline`) : quatre types, `"script"`, `"script attribute"`,
  `"style"`, `"style attribute"`. « Nonces only apply to inline <script> and inline
  <style>, not to attributes of either element » (`#match-element-to-source-list`).
  `#security-nonces` : « the server MUST generate a unique value each time it transmits
  a policy » **[N]**.
- `'strict-dynamic'` (`#strict-dynamic-usage`, non normatif) : `'unsafe-inline'` et
  `'self'` sont ignorés au chargement des scripts. Son effet sur `import()` : **non
  vérifié**.
- `style-src` : « The styles will be blocked unless every policy allows inline style ».
  `style-src-elem` gouverne les styles hors attributs, `style-src-attr` les attributs
  `style`. Le texte range sous `unsafe-eval` « all invocations of CSSOM's various cssText
  setters and insertRule methods », sans citer `replace` ni `replaceSync` ; une ISSUE
  reste ouverte. Écarts réels entre navigateurs : **non vérifiés**.
- `worker-src` : repli `worker-src`, `child-src`, `script-src`, `default-src`.
- `frame-ancestors` : « MUST be ignored when contained in a policy declared via a <meta>
  element » et « will not fall back to the default-src directive's value » **[N]**.
  Absence de repli de `base-uri` et `form-action` : **non vérifiée**.
- `object-src 'none'` bloque les objets. `report-uri` est déprécié au profit de
  `report-to`, dont le point (`Reporting-Endpoints`) « MUST be potentially trustworthy ».
- Trusted Types (spec séparée) : `require-trusted-types-for` n'accepte que `'script'` ;
  mode report-only possible.
- BCD : `strict-dynamic` Chrome 52, Firefox 52, Safari 15.4 ; `wasm-unsafe-eval` 97,
  102, 16 ; `style-src-attr`/`-elem` 75, 108, 15.4 (Safari : `-elem` sans effet avant
  26.2) ; `frame-ancestors` 40, 58, 10 ; `worker-src` 59, 58, 15.5 ; `report-to` 70,
  149, 16.4 ; Trusted Types 83, 148, 26.

### SRI et intégrité des modules (SRI ED Level 2, HTML, Fetch)

- `#hash-functions` : « MUST support the SHA-256, SHA-384, and SHA-512 cryptographic
  hash functions » **[N]** ; parmi plusieurs empreintes, « the user agent will choose the
  strongest » ; « validation using unsupported hash functions acts like no integrity
  value was provided » (`#agility`) ; un jeu de métadonnées vide vaut succès.
- « Subresource Integrity requires CORS » ; le cas de la même origine n'est pas décrit
  par SRI. Fetch : « If bytes do not match request's integrity metadata, then run
  processBodyError ». Règle d'éligibilité par type de réponse : **non vérifiée**.
- SRI ajoute `integrity` à `link` et `script` ; `modulepreload` n'y est pas cité. BCD :
  `rel=modulepreload` Chrome 66, Firefox 115, Safari 17.
- HTML, « The script element » : `integrity` « sets the integrity metadata used for the
  initial fetch of an external script ». Pour un module sans attribut, les métadonnées
  sont « the result of resolving a module integrity metadata from the URL ».
- HTML, « Fetching scripts » : « resolve a module integrity metadata » lit
  `integrity[url]` dans l'import map et, sans entrée, « return the empty string ». Les
  options descendantes, utilisées par `import()` (`HostLoadImportedModule`), reprennent
  ce résultat.
- La table des modules est indexée par `(url, moduleType)` : « It is intentional that
  the module map is keyed by the request URL » ; l'empreinte n'est pas dans la clé.
- `Integrity-Policy` (SRI ED) bloque les destinations `script` et `style` sans
  empreinte. BCD : Chrome 138, Firefox 145 (partiel), Safari 26, avec un **écart connu**
  en Safari : les modules sont bloqués même quand l'import map fournit `integrity`.

### Import maps (HTML, « Import maps »)

- JSON « with at most the three keys "imports", "scopes", and "integrity" », fourni par
  un `<script type="importmap">` **en ligne** : « External import maps and speculation
  rules are not currently supported ». Les clés de `integrity` sont des URL, pas des
  spécificateurs.
- Un script en ligne est soumis à « Should element's inline behavior be blocked by
  Content Security Policy? », import map comprise (lecture par résumé, à confirmer par un
  test).
- Plusieurs import maps : « new import maps cannot define the module resolution for
  modules that were already defined by past import maps » ; une règle qui touche un
  module déjà résolu « gets dropped », avec au plus un avertissement console.
- BCD : import maps Chrome 89, Firefox 108, Safari 16.4 ; clé `integrity` Chrome 127,
  Firefox 138, Safari 18 ; plusieurs import maps Chrome 133, Safari 18.4, Firefox 150
  derrière `dom.multiple_import_maps.enabled` seulement ; `import()` 63, 67, 11.1.

### Custom Elements (HTML, « Custom elements »)

- `define()` lève `NotSupportedError` si le nom ou le constructeur est déjà défini,
  `SyntaxError` pour un nom invalide ; un nom valide contient U+002D (-). Il n'existe ni
  `undefine` ni `remove`.
- Registres à portée (`new CustomElementRegistry()`, `initialize(root)`) : dans HTML ;
  BCD : Chrome 146, Safari 26, Firefox 159 (bêta au 2026-10-19). `adoptedStyleSheets`
  73, 101, 16.4 ; `delegatesFocus` 53, 94, 15.

### Jetons CSS et Shadow DOM

- CSS Custom Properties 1 (`#defining-variables`) : `--*` « Inherited: yes ». CSS
  Cascade 5 (`#inheriting`) : « For a DOM tree with shadows, inheritance operates on the
  flattened element tree. » **[N]** Un élément assigné à un `<slot>` hérite du slot.
- `all` réinitialise tout sauf `direction`, `unicode-bidi` et les propriétés
  personnalisées (lecture par résumé).
- Vue, « SFC as Custom Element » : le composant « inlines its `<style>` tags as strings
  of CSS » dans la racine d'ombre ; la documentation ne parle ni de CSP ni de nonce.

### Clear-Site-Data et stockage

- `Clear-Site-Data = 1#( quoted-string )` ; types `"cache"`, `"cookies"`, `"storage"`,
  `"executionContexts"`, `"*"`. `"storage"` couvre `localStorage`, `sessionStorage`,
  IndexedDB et les service workers ; Cache Storage n'apparaît pas dans le texte lu
  (**non confirmé**). L'en-tête n'agit que sur une réponse venue du réseau.
- BCD : en-tête, `"storage"` et `"cookies"` Chrome 61, Firefox 63, Safari 17 ; `"cache"`
  partiel en Chrome (blocages de plusieurs secondes) ; `"executionContexts"` plus pris
  en charge nulle part.
- IndexedDB 3.0 : `deleteDatabase` reste bloqué tant que d'autres connexions sont
  ouvertes ; chacune reçoit `versionchange`.
- Storage Standard : bucket `"best-effort"` par défaut, effaçable sous pression ;
  `persist()` (Chrome 55, Firefox 57, Safari 15.2). La clé de stockage est l'origine :
  le shell et tous les plugins partagent le même IndexedDB **[D]**.

### WCAG 2.2, APG, référentiels, axe-core

- WCAG 2.2 retire 4.1.1 Parsing (« Obsolete and removed »). Nouveaux critères : niveau A
  3.2.6 Consistent Help, 3.3.7 Redundant Entry ; niveau AA 2.4.11 Focus Not Obscured
  (Minimum), 2.5.7 Dragging Movements, 2.5.8 Target Size (Minimum), 3.3.8 Accessible
  Authentication (Minimum) ; niveau AAA 2.4.12, 2.4.13, 3.3.9.
- 2.5.8 : cible d'au moins 24 × 24 px CSS, avec exceptions (espacement, équivalent, en
  ligne, contrôle du navigateur, essentiel). 2.4.11 : le composant focalisé n'est pas
  « entirely hidden due to author-created content ». 2.5.7 : tout glisser a une
  alternative à un seul pointeur.
- Critères utiles à une application dense : 1.4.3 (4,5:1), 1.4.11 (3:1), 2.1.1, 2.1.2,
  2.4.7, 4.1.2, 4.1.3 (messages d'état), 1.4.10 (320 px, hors contenus à deux
  dimensions comme les tables de données), 1.4.4 (200 %).
- APG Tree View : flèches droite et gauche ouvrent, ferment ou remontent ; `tree`,
  `treeitem`, `group`, `aria-expanded` ; `aria-level`, `aria-setsize`, `aria-posinset`
  requis quand tous les nœuds ne sont pas dans le DOM. Grid : un seul élément
  focalisable dans l'ordre de tabulation ; `aria-rowcount`, `aria-rowindex` pour les
  lignes hors DOM. Treegrid : `aria-expanded` sur la ligne.
- EN 301 549 V3.2.1 et RGAA 4.1.2 reposent sur WCAG 2.1 ; V4.1.1 et RGAA 5 viseront
  WCAG 2.2 (**non vérifié**).
- axe-core : tags `wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`, `wcag22aa`,
  `best-practice`, `EN-301-549`, `RGAAv4`… ; pas de tag `wcag22a`. Seule `target-size`
  (2.5.8) porte `wcag22aa`, et elle est désactivée par défaut (`"enabled": false`) ; on
  l'active par `rules: { "target-size": { enabled: true } }`. Un résultat `incomplete`
  « could neither be determined to definitively pass or definitively fail ». L'éditeur
  revendique « on average 57% of WCAG issues » détectés : revendication, pas mesure
  indépendante.

### SemVer et plages

- SemVer 2.0.0, point 4 : « Major version zero (0.y.z) is for initial development.
  Anything MAY change at any time. » Point 2 : `X.Y.Z` entiers sans zéros de tête.
- **Les plages (`^`, `~`, `x`) ne sont pas dans semver.org.** Elles sont définies par
  node-semver : `^1.2.3` = `>=1.2.3 <2.0.0-0`, `^0.2.3` = `>=0.2.3 <0.3.0-0`, `~1.2` =
  `>=1.2.0 <1.3.0-0` ; préversions exclues par défaut. Équivalence exacte de `^1.2` : à
  confirmer (le README liste `^1.2.x`).

## Pièges

- **SRI et `import()` dynamique.** SRI ne s'écrit pas sur `import()` : seul
  `<script integrity>` contrôle le module d'entrée, et les modules importés ne le sont
  que par la clé `integrity` de l'import map. Le navigateur est permissif : URL absente
  de la table, empreinte vide ou algorithme inconnu donnent un module exécuté sans
  contrôle. La table des modules étant indexée par URL, un module déjà chargé sans
  empreinte n'est pas revérifié **[D]**. Un plugin découpé en plusieurs fichiers a
  besoin d'une entrée par fichier. Safari bloque les modules sous `Integrity-Policy`
  même avec `integrity` (BCD).
- **Styles en ligne et CSP.** Sans `'unsafe-inline'`, tout `<style>` et tout attribut
  `style=""` venu du HTML est bloqué : le mode « SFC as Custom Element » de Vue, les
  bibliothèques qui injectent des `<style>` et le serveur de développement de Vite
  **[D]**. Les liaisons `:style` de Vue passent par le CSSOM, probablement permis
  (**non vérifié**). Safari avant 26.2 ignore `style-src-elem`.
- **Import map unique et figée avant le premier import.** Elle est en ligne, donc
  soumise à la CSP par l'empreinte de ses octets exacts ; l'empreinte change avec
  l'ensemble des plugins, donc `index.html` varie. Une seconde import map n'est prise en
  charge ni par Firefox 158 ni par l'ESR 153, et ne peut de toute façon pas remapper un
  spécificateur déjà résolu (`@kartenn/sdk`). Avec N instances en cours de mise à jour,
  `index.html` et les modules peuvent venir d'instances différentes **[D]**.
- **Collision de noms de Custom Elements.** `define` échoue sur un nom ou un
  constructeur déjà pris et rien ne se retire ; une exception au niveau supérieur d'un
  module laisse les éléments déjà définis **[D]**. Un préfixe tiré d'un nom de plugin à
  tirets est ambigu (`foo-bar-x` relève de `foo` comme de `foo-bar`) **[D]**. Les
  registres à portée manquent à Firefox stable et à l'ESR.
- **Shadow DOM et accessibilité.** Les références d'identifiant (`aria-labelledby`,
  `for`) ne traversent pas la frontière d'ombre (**non vérifié**).
- **`Clear-Site-Data`.** Il n'agit que sur une réponse reçue (pas sur une session qui
  expire sans déconnexion), efface aussi les préférences gardées dans `localStorage`, et
  ne couvre peut-être pas Cache Storage ; `deleteDatabase` reste bloqué tant qu'un
  autre onglet garde sa connexion.
- **axe-core.** `target-size` est désactivée par défaut ; 2.4.11, 2.5.7, 3.2.6, 3.3.7 et
  3.3.8 n'ont aucune règle ; un résultat `incomplete` n'est pas un succès.
- **`^0.y`.** En 0.x, `^0.1` vaut `>=0.1.0 <0.2.0` : chaque mineure peut casser.
- **Cycle de Firefox.** À 2 semaines (**non vérifié**), « deux dernières versions » ne
  couvre que 4 semaines.

## Implications pour kartenn (propositions, à trancher dans la spec 007)

1. **[N][T] CSP (W2, W4).** En en-tête (`frame-ancestors` est ignoré en `<meta>`) :
   `script-src 'self' 'sha256-…'` (empreinte de l'import map), sans `'strict-dynamic'`
   ni `unsafe-eval` ; `connect-src 'self'`, `object-src 'none'`, `base-uri 'none'`,
   `frame-ancestors 'none'`, `form-action 'self'` ; les autres directives de chargement
   en `'self'`, `default-src` à trancher.
2. **[T] Import map rendue par le serveur.** Recommandé : `index.html` produit par
   kartenn avec l'import map (`@kartenn/sdk`, modules du shell, plugins retenus) et
   l'empreinte CSP de ses octets exacts, ETag dérivé de l'ensemble ; l'empreinte plutôt
   qu'un nonce (unique par réponse). Une import map figée au build ne contrôlerait pas
   les plugins déposés à chaud (W3).
3. **[T] Intégrité.** Une entrée `integrity` (`sha384-…`) par module du shell et de
   chaque plugin ; aucun module servi sans empreinte publiée. `modulepreload`
   facultatif ; ne pas poser `Integrity-Policy` (écart Safari). Éprouver le module
   altéré aussi sur WebKit si le Playwright de Nix le fournit (**non vérifié**).
4. **[T] Une seule import map**, jamais ajoutée par un plugin. Base minimale des
   navigateurs : import maps avec `integrity` (Chrome 127, Firefox 138, Safari 18).
5. **[T] Styles.** (a) `style-src 'self'` strict : CSS extrait au build, fichiers CSS
   des plugins ; (b) en repli, `style-src-attr 'unsafe-inline'` ; (c) `'unsafe-inline'`
   pour tous les styles, le plus simple, l'injection CSS restant bornée par `img-src` et
   `font-src` en `'self'` **[D]**. Piste : (a), violations mesurées en E2E
   (`securitypolicyviolation`). Les plugins Vue de première partie extraient leur CSS.
6. **[T] Noms de Custom Elements (W5, W7).** Le manifeste déclare les noms de balise ;
   le serveur refuse les doublons entre plugins, le shell attrape `NotSupportedError` et
   marque le plugin en échec ; noms réservés au shell. Registres à portée à revoir quand
   l'ESR les aura.
7. **[N] Jetons CSS (W12).** Définis sur `:root` : hérités dans l'arbre aplati, sans
   code ni `adoptedStyleSheets` ; noms `--k-*` documentés par le SDK.
8. **[T] Effacement à la déconnexion (W10).** Effacement par le shell (`deleteDatabase`
   avec fermeture sur `versionchange` dans chaque onglet, `localStorage`, Cache
   Storage), et `Clear-Site-Data: "storage"` sur la réponse de déconnexion (pris en
   charge par toute la cible) ; pas de `"cache"`, `"executionContexts"` ni `"*"`.
   `navigator.storage.persist()` pour le brouillon : à décider.
9. **[T] Navigateurs (W17).** Garder la cible de W17 et y ajouter la base minimale du
   point 4.
10. **[T] Trusted Types et rapports.** `require-trusted-types-for 'script'` en
    report-only au plus, tant que les plugins tiers n'y sont pas conformes ; pas de
    point de rapport serveur en v0.1, collecte par `securitypolicyviolation` en E2E.
11. **[T] SemVer (W7).** Écrire le sous-ensemble de plages accepté, défini par
    node-semver ; viser un SDK 1.0 avant d'ouvrir aux tiers.
12. **[T] Accessibilité (W14).** WCAG 2.2 AA couvre 2.0 et 2.1, mais les référentiels en
    vigueur (EN 301 549 V3.2.1, RGAA 4.1.2) reposent sur WCAG 2.1 : « compatible avec ces
    référentiels » plutôt que « leur base ». axe-core : tags `wcag2a`, `wcag2aa`,
    `wcag21a`, `wcag21aa`, `wcag22aa` et `target-size` activée ; tests dédiés pour
    2.4.11 (focus non masqué par le bandeau de W9), 2.5.7, 3.2.6, 3.3.7 et 3.3.8. Tree
    View avec `aria-level`, `aria-setsize`, `aria-posinset` ; Grid avec `aria-rowcount`,
    `aria-rowindex` ; l'élément focalisé reste monté dans une liste virtualisée **[D]**.

## Non trouvé ou non relu

- Statuts et dates des TR : titres de résultats de recherche, non ouverts. Statut non
  relu : Trusted Types, Reporting API, IndexedDB 3.0, CSS Scoping (404), APG.
- Non relus à la source : Conformance de CSP3 et SRI ; `#directive-connect-src` ; repli
  de `base-uri` et `form-action` ; `'strict-dynamic'` et `import()` ; calcul de
  l'empreinte d'un script en ligne ; éligibilité SRI des réponses de même origine ;
  « valid custom element name » en entier ; `modulepreload` et `integrity` dans HTML ;
  « delete a database » ; `replaceSync` et `insertRule` sous CSP, par navigateur ;
  références ARIA dans le Shadow DOM ; `@property` et héritage.
- Non vérifiés (commentaires non primaires) : EN 301 549 V4.1.1, RGAA 4.1.2 et 5,
  cycle de 2 semaines de Firefox.
- Sans source primaire : comportement sans « multiple import maps », plafond de
  stockage de Safari, effet de `"storage"` sur Cache Storage, part des critères WCAG
  détectables automatiquement.
