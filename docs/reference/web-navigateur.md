# Navigateur : CSP, SRI, modules, éléments personnalisés, cookies, stockage, accessibilité

Note de lecture pour la spec [007-web-shell-plugins](../../specs/007-web-shell-plugins/).
Recherche documentaire du 2026-10-20.

Méthode : les pages rendues de w3.org, w3c.github.io, whatwg.org, drafts.csswg.org,
rfc-editor.org, datatracker et MDN n'étaient pas joignables depuis l'environnement de
recherche (DNS). Les textes ont été lus dans les sources des dépôts officiels (Bikeshed,
Wattsi, Markdown) sur GitHub, à la date de leur dernier commit relevé. Les renvois sont
donc des **ancres** (`#…`) des documents publiés, pas des numéros de section ; elles
viennent des sources et restent à recouper sur les pages rendues. Les citations entre
guillemets sont littérales (fragments courts), le reste est paraphrasé. Ce qui n'a pas pu
être relu à la source est marqué **non vérifié**. « Observé » désigne le comportement
d'un moteur relevé dans les tests de conformité (WPT) et leurs attentes par moteur, ou
dans le code ; [BCD] est une donnée de compatibilité, pas une norme.

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [CSP3] [Content Security Policy Level 3](https://w3c.github.io/webappsec-csp/) (Editor's Draft) ; [TR](https://www.w3.org/TR/CSP3/) | ED (source `w3c/webappsec-csp`, dernier commit lu 2026-09-16) ; Working Draft republié en 2026, pas une Recommendation (dates **non vérifiées**) | 2026-10-20 |
| [SRI] [Subresource Integrity Level 2](https://w3c.github.io/webappsec-subresource-integrity/) ; [Level 1](https://www.w3.org/TR/2016/REC-SRI-20160623/) | Level 2 : ED (source `w3c/webappsec-subresource-integrity`) ; Level 1 : Recommendation (2016-06-23), **non relue** | 2026-10-20 |
| [HTML] [HTML Living Standard](https://html.spec.whatwg.org/multipage/) | Living Standard (source `whatwg/html`, dernier commit lu 2026-10-19) | 2026-10-20 |
| [Fetch] [Fetch Standard](https://fetch.spec.whatwg.org/), [DOM] [DOM Standard](https://dom.spec.whatwg.org/) | Living Standards (Review Drafts 2026-06) | 2026-10-20 |
| [CSSOM] [CSSOM](https://drafts.csswg.org/cssom-1/) | ED (lu en partie) | 2026-10-20 |
| [CSS-VAR] [CSS Custom Properties Level 1](https://www.w3.org/TR/css-variables-1/) | ED lu ; dernière CR relevée 2022-06-16, statut TR courant **non vérifié** | 2026-10-20 |
| [CSS-SHADOW] [CSS Shadow Module Level 1](https://drafts.csswg.org/css-shadow-1/) | ED ; reprend CSS Scoping 1, dont la source renvoie désormais vers lui | 2026-10-20 |
| [whatwg/html#10327](https://github.com/whatwg/html/pull/10327), [#10528](https://github.com/whatwg/html/pull/10528) | modifications de HTML, fusionnées le 2026-07-15 et le 2024-11-14 | 2026-10-20 |
| [w3c/webappsec-csp#8](https://github.com/w3c/webappsec-csp/issues/8), [#212](https://github.com/w3c/webappsec-csp/issues/212) | tickets ouverts | 2026-10-20 |
| [Fetch Metadata](https://w3c.github.io/webappsec-fetch-metadata/) | ED (source `w3c/webappsec-fetch-metadata`) | 2026-10-20 |
| [RFC 6265bis] [draft-ietf-httpbis-rfc6265bis-22](https://datatracker.ietf.org/doc/draft-ietf-httpbis-rfc6265bis/) (source `httpwg/http-extensions`, tag `draft-ietf-httpbis-rfc6265bis-22`) | Internet-Draft du 2025-12-09, transmis à l'IESG puis à la publication (**non vérifié**) ; pas encore de numéro de RFC ; remplacera RFC 6265 | 2026-10-20 |
| [draft-ietf-httpbis-layered-cookies](https://datatracker.ietf.org/doc/draft-ietf-httpbis-layered-cookies/) | Internet-Draft concurrent, remplacerait aussi RFC 6265 (version **non vérifiée**) | 2026-10-20 |
| [Secure Contexts](https://w3c.github.io/webappsec-secure-contexts/) | ED (source `w3c/webappsec-secure-contexts`, dernier commit 2023-11-10) ; statut TR **non vérifié** | 2026-10-20 |
| [Storage] [Storage Standard](https://storage.spec.whatwg.org/) | Living Standard (dernier commit lu 2026-10-06) | 2026-10-20 |
| [IndexedDB] [Indexed Database API 3.0](https://w3c.github.io/IndexedDB/) | ED | 2026-10-20 |
| [WebCrypto] [Web Cryptography Level 2](https://w3c.github.io/webcrypto/) | ED | 2026-10-20 |
| Code de Chromium (`chromium/chromium` : `net/cookies/`, `net/base/`), de Firefox (`mozilla-firefox/firefox` : `netwerk/cookie/`, `dom/security/`), de WebKit ([PR #61338](https://github.com/WebKit/WebKit/pull/61338)) | code source des branches principales ; PR fusionnée le 2026-04-03 | 2026-10-20 |
| [httpwg/http-extensions#2605](https://github.com/httpwg/http-extensions/issues/2605) | discussion du groupe de travail (2023), close en 2024 | 2026-10-20 |
| [WCAG 2.2](https://www.w3.org/TR/WCAG22/) ([version datée](https://www.w3.org/TR/2024/REC-WCAG22-20241212/)) | Recommendation du 2023-10-05, republiée le 2024-12-12 ; lue au tag `WCAG22-20241212` de `w3c/wcag` ; errata **non relus** | 2026-10-20 |
| [Understanding WCAG 2.2](https://www.w3.org/WAI/WCAG22/Understanding/) | non normatif, même tag | 2026-10-20 |
| [ARIA] [WAI-ARIA 1.2](https://www.w3.org/TR/wai-aria-1.2/) ; [1.3](https://w3c.github.io/aria/) | 1.2 : Recommendation (2023-06-06, **non relue**) ; les définitions citées sont lues dans l'ED 1.3, équivalence avec la 1.2 **non vérifiée** | 2026-10-20 |
| [ARIA in HTML](https://w3c.github.io/html-aria/) | ED | 2026-10-20 |
| [APG] [ARIA Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/) | guide non normatif, non daté (source `w3c/aria-practices`) | 2026-10-20 |
| [axe-core](https://github.com/dequelabs/axe-core) (`doc/API.md`, `doc/rule-descriptions.md`, `doc/developer-guide.md`) | outil MPL-2.0, branche de développement (4.14.0) | 2026-10-20 |
| [vue-i18n](https://vue-i18n.intlify.dev/) (source `intlify/vue-i18n`), [`@intlify/unplugin-vue-i18n`](https://github.com/intlify/bundle-tools) | documentation officielle (branche principale = 12.0.0-alpha.7) ; code des tags v11.1.10 et v9.14.5 ; npm : `latest` 11.4.16, unplugin 11.2.7 | 2026-10-20 |
| [BCD] [browser-compat-data](https://github.com/mdn/browser-compat-data) | données de compatibilité et dates de sortie, pas une norme | 2026-10-20 |
| [WPT] [web-platform-tests](https://github.com/web-platform-tests/wpt) et attentes par moteur (`testing/web-platform/meta` de Firefox, `-expected.txt` de WebKit) | tests de conformité | 2026-10-20 |
| [Playwright `addInitScript`](https://playwright.dev/docs/api/class-page#page-add-init-script) | documentation officielle | 2026-10-20 |

## Ce que disent les sources

### CSP Level 3

**Import map en ligne et empreinte**

- [HTML] `#prepare-the-script-element` : le type `importmap` est fixé avant le contrôle
  CSP ; le texte source est le « child text content » de l'élément. Sans `src`, si
  « Should element's inline behavior be blocked by Content Security Policy? » rend
  « Blocked » (type CSP `script`), l'algorithme s'arrête. Une import map est donc
  contrôlée comme un script en ligne, insérée par l'analyseur ou par script. Avec `src`,
  événement `error` : « External import maps and speculation rules are not currently
  supported. »
- [CSP3] `#should-block-inline`, `#match-element-to-source-list` : pour le type `script`,
  la liste est celle de `script-src-elem`, à défaut `script-src`, à défaut `default-src`
  (`#directive-fallback-list`). Une `hash-source` correspond quand le SHA-256, 384 ou 512
  de l'encodage UTF-8 du texte, en base64, égale sa valeur (`-` et `_` lus comme `+` et
  `/`). `#allow-all-inline` : une `hash-source` ou une `nonce-source` présente rend
  `'unsafe-inline'` inopérant.
- Texte haché : [DOM] « child text content », concaténation des nœuds `Text` ; espaces et
  retours à la ligne entre `>` et `</script>` comptent. [HTML]
  `#preprocessing-the-input-stream` : « Before the tokenization stage, the input stream
  must be preprocessed by normalizing newlines » (CR et CRLF → LF) ; dans l'état « script
  data », U+0000 devient U+FFFD.
- Gestionnaires en ligne (`onclick=…`) : type « script attribute », `script-src-attr` puis
  `script-src` ; une `hash-source` n'y joue qu'avec `'unsafe-hashes'`.
- [CSP3] `#script-pre-request` : un script externe dont l'intégrité est un sous-ensemble
  non vide des `hash-source` est aussi autorisé (conditions de CORS **non relues**).
- Observé : [WPT] `import-maps/csp/hash.html`, `hash-failure.html` : une import map
  refusée est ignorée, les spécificateurs se résolvent sans elle. Résultats par moteur
  **non relus**.

**`style-src`, attributs `style` et CSSOM**

- [CSP3] `#directive-style-src` : couvre les requêtes de feuilles (`<link>`, `@import`,
  `Link`) par `style-src-elem`, les blocs `<style>` (contrôle en ligne) et l'attribut
  `style` par `style-src-attr`, chacun avec repli sur `style-src` puis `default-src`.
- [HTML] « update a style block » : contrôle CSP en ligne d'un `<style>` connecté, sans
  cas particulier pour une racine d'ombre (inférence : un `<style>` dans une racine
  d'ombre est soumis à `style-src-elem`). [HTML] « The style attribute » : la valeur est
  analysée à l'ajout ou au changement ; si la CSP bloque, ses règles ne s'appliquent pas.
  `setAttribute('style', …)`, `innerHTML` et l'analyseur sont donc bloqués. Observé :
  [WPT] `style-src-inline-style-attribute-blocked.html`, `violatedDirective` =
  `style-src-attr`.
- [CSSOM] `cssText` et `setProperty` passent par « update style attribute for », qui
  pose un drapeau évitant la réanalyse : pas de contrôle CSP ; CSSOM ne mentionne pas la
  CSP. Feuilles constructibles (`new CSSStyleSheet()`, `replaceSync`,
  `adoptedStyleSheets`) : aucun crochet CSP. Observé : [WPT]
  `style-src-inline-style-with-csstext.html`, aucune violation.
- Contradiction : [CSP3] `#directive-style-src` soumet à `'unsafe-eval'` « insert a CSS
  rule », « parse a CSS declaration block »… (« all invocations of CSSOM's various
  cssText setters and insertRule methods »), sous un ISSUE (#212, ouvert) ; les moteurs
  ne l'appliquent pas (WPT ci-dessus).

**`form-action` et redirections**

- [CSP3] `#directive-form-action` : contrôle avant une navigation de type
  « form-submission » ; la directive n'est pas dans `#directive-fallback-list`.
- [HTML] `#process-a-navigate-fetch` : la requête de navigation est en mode `manual` ;
  chaque tour de la boucle de redirection refait le contrôle CSP de navigation avec le
  type « form-submission ». La redirection qui suit un `POST` de formulaire est donc
  soumise à `form-action` (lecture paraphrasée, **non vérifiée** mot à mot). CSP3 ne dit
  rien des redirections ; w3c/webappsec-csp#8 est ouvert depuis 2015.
- Observé : [WPT] `form-action-src-redirect-blocked.sub.html` (POST de même origine
  redirigé vers un autre hôte, violation attendue, `blockedURI` = URL d'origine) :
  Firefox `expected: FAIL`, WebKit FAIL ; Chromium sans attente d'échec, donc bloque
  (**non vérifié** par exécution).

**Repli de `default-src`, `<meta>`, `'self'`**

- [CSP3] `#directive-fallback-list` : `script-src` (et `-elem`, `-attr`), `style-src` (et
  `-elem`, `-attr`), `img-src`, `font-src`, `connect-src`, `manifest-src`, `media-src`,
  `object-src`, `frame-src` et `worker-src` (par `child-src`) se replient sur
  `default-src`. `base-uri`, `form-action`, `frame-ancestors` et `sandbox` non.
- [CSP3] `#meta-element` : `frame-ancestors` MUST être ignorée dans une `<meta>` ;
  `report-uri` et `sandbox` n'y sont pas pris en charge ; une `<meta>` ne couvre pas ce
  qui la précède.
- [CSP3] `#match-url-to-source-expression` : `'self'` couvre la même origine et ses mises
  à niveau. Page `http://h:p` : `http`, `https`, `ws`, `wss` de même hôte et port ; page
  `https` : `wss` mais ni `ws` ni `http`. `blob:` n'est pas couvert ; `data:` exige le
  schéma explicite.

**Signalement et `eval`**

- [CSP3] `#report-violation` : en tâche (après la fin du script fautif) ; événement
  `securitypolicyviolation` sur l'élément s'il est connecté, sinon sur le document ;
  `bubbles` et `composed` (il sort des racines d'ombre). Champs : `effectiveDirective`,
  `violatedDirective` (alias), `blockedURI` (`inline`, `eval`, `wasm-eval` ou URL),
  `sourceFile`, `lineNumber`, `disposition`, `sample` (vide sans `'report-sample'`).
- [Playwright] `addInitScript` s'exécute « after the document was created but before any
  of its scripts were run ».
- [CSP3] `#can-compile-strings` : sans `'unsafe-eval'` dans `script-src` (ou
  `default-src`), `eval`, `new Function` et `setTimeout` avec une chaîne lèvent
  `EvalError` et une violation (`blockedURI` `eval`). `#can-compile-wasm-bytes` :
  WebAssembly exige `'unsafe-eval'` ou `'wasm-unsafe-eval'`.

### SRI, modules et import maps

- [SRI] `#the-integrity-attribute` : valeur vide ou au moins une métadonnée
  `<algo>-<base64>` ; options `?…` inconnues : « the user agent MUST ignore all
  unrecognized ». `#hash-functions` : SHA-256, SHA-384 et SHA-512 MUST. `#agility` : seul
  l'algorithme le plus fort présent compte, une correspondance parmi ses valeurs suffit ;
  un algorithme inconnu ou une valeur inexploitable équivaut à l'absence de contrôle.
- [SRI] `#handling-integrity-violations` : écart → erreur réseau et événement `error`.
  Hors de l'origine, CORS est requis (`#cross-origin-data-leakage`) ; en même origine,
  aucune exigence (réponse `basic`, corps lisible).
- [HTML] `integrity` vaut pour `<script>` externe et pour `<link>` `stylesheet`,
  `preload`, `modulepreload`.
- **Module racine seulement** : [HTML] `#prepare-the-script-element` applique
  l'`integrity` de l'élément au module racine. « get the descendant script fetch
  options » donne à chaque import (statique ou `import()`) l'intégrité de l'import map
  (« resolve a module integrity metadata » : « If map's integrity[url] does not exist,
  then return the empty string. »). L'intégrité du parent ne se transmet pas.
- **`modulepreload`** : [HTML] `#link-type-modulepreload`, « fetch a modulepreload module
  script graph » : intégrité de l'attribut, sinon de l'import map. Le module « places the
  result into the appropriate module map ». Clé (URL, type de module), ni destination ni
  mode d'identification ; une entrée présente est réutilisée sans comparer les options,
  intégrité comprise ; une entrée en cours fait attendre l'`import`. `load` et `error` ne
  portent que sur ce module. Ses dépendances : « optionally fetch the descendants »,
  avec l'intégrité de l'import map, jamais celle du lien.
- **Échec** : depuis whatwg/html#10327 (« Don't cache HTTP errors in the module map »,
  2026-07-15), une récupération en échec retire l'entrée de la carte des modules : un
  `import` ultérieur de la même URL refait la requête, avec l'intégrité de l'import map
  ou sans. La PR vise les erreurs HTTP ; un écart d'intégrité est une erreur réseau, même
  branche (inférence). La PR liste des correctifs en cours dans les trois moteurs : le
  comportement dépend de la version (ancien : échec gardé, l'`import` échoue).
- **Import maps** : [HTML] `#import-maps` : en ligne seulement ; membres `imports`,
  `scopes`, `integrity` (clés : URL, pas spécificateurs). Plusieurs import maps, « either
  before or after any modules have been imported » (whatwg/html#10528) : une règle qui
  changerait une résolution déjà faite est rejetée ; une entrée `integrity` déjà présente
  n'est pas remplacée.
- [Fetch] `RequestInit.integrity` → métadonnées d'intégrité de la requête ; écart →
  erreur réseau → `fetch` rejette un `TypeError`, sans distinguer la cause ; réponse
  opaque (corps nul) → échec.

### Custom Elements, Shadow DOM, propriétés personnalisées

- [HTML] `#valid-custom-element-name` : première lettre minuscule ASCII, aucune majuscule
  ASCII, un `-`, hors noms réservés (`annotation-xml`, `color-profile`, `font-face`,
  `font-face-src`, `font-face-uri`, `font-face-format`, `font-face-name`,
  `missing-glyph`).
- [HTML] `#dom-customelementregistry-define` : `TypeError` (pas un constructeur),
  `SyntaxError` (nom invalide), `NotSupportedError` (nom déjà défini, constructeur déjà
  employé, définition en cours). Une définition ne se retire pas. `whenDefined` rejette
  un nom invalide. Registres à portée (`new CustomElementRegistry()`, option
  `customElementRegistry` d'`attachShadow`) dans la spec ; [BCD] : Chromium 146,
  Safari 26, pas dans les Firefox visés.
- [DOM] `#dom-element-attachshadow` : modes `open` et `closed` ; hôte : élément
  personnalisé autonome ou élément d'une liste fermée. `composed` vaut `false` par
  défaut ; [DOM] « Interface ShadowRoot » : un événement non composé s'arrête à la racine
  d'ombre. Émis dedans, il faut `composed: true` pour que l'hôte le reçoive (cible
  reciblée) et `bubbles: true` en plus pour ses ancêtres. Émis sur l'hôte, seul
  `bubbles` compte.
- [CSS-VAR] `#defining-variables` : « Inherited: yes ». [CSS-SHADOW] `#flattening` :
  l'héritage suit l'arbre aplati, où les enfants d'une racine d'ombre ont l'hôte pour
  parent. `var(--k-color-bg)` résout donc dans la racine d'ombre d'un plugin. `:host`
  désigne l'hôte ; `::part()` vise les éléments `part` d'un hôte. Une feuille liée depuis
  un arbre d'ombre s'applique à cet arbre ([CSSOM] `#css-style-sheet-collections`).

### Cookies sur le bouclage

- [RFC 6265bis] `#storage-model`, étape 13 : « If the request-uri does not denote a
  "secure" connection (as defined by the user agent), and the cookie's secure-only-flag
  is true, then abort these steps and ignore the cookie entirely. » Étape 21 : un nom
  commençant par `__Host-` (casse ignorée) est ignoré sauf si le cookie est `Secure`,
  sans `Domain` (« host-only-flag ») et porte un attribut `Path` valant `/`.
- `#retrieval-algorithm` : un cookie `Secure` n'est renvoyé que si l'URI « must denote a
  "secure" connection (as defined by the user agent) » (en minuscules : critère de
  l'algorithme, pas mot-clé de la RFC 8174). Note : « The notion of a "secure"
  connection is not defined by this document. » ; « most user agents consider "https"
  to be a scheme that denotes a secure protocol and "localhost" to be trusted host. » Le
  texte ne nomme ni le bouclage ni `127.0.0.1`.
- Section « The "__Host-" Prefix » : « Ports are the only piece of the origin model that
  `__Host-` cookies continue to ignore. » `#same-site-requests` : « same-site » au sens
  de HTML (schéma et hôte) ; un cookie `Strict` part sur les requêtes de même site,
  jamais sur une requête intersite.
- [Secure Contexts] `#is-origin-trustworthy`, étape 4 : « If origin's host matches one of
  the CIDR notations 127.0.0.0/8 or ::1/128 », l'origine est « Potentially
  Trustworthy », sans condition. `#localhost` : `localhost` seulement MAY, sous les
  règles de résolution de « let-localhost-be-localhost ».
- Observé, Chromium (`net/cookies/cookie_util.cc`, `cookie_base.cc`) :
  `ProvisionalAccessScheme` classe `127.0.0.0/8`, `::1` et `localhost` en
  `kTrustworthy` ; un cookie `Secure` y est accepté et renvoyé, avec un simple
  avertissement ; `__Host-` est accepté sur une telle URL quand `Path` vaut `/`. En 2023
  (httpwg/http-extensions#2605), Chromium refusait encore les préfixes sur
  `http://localhost` ; version du changement non trouvée.
- Observé, Firefox (`netwerk/cookie/CookieValidation.cpp`, `CookiePrefixes.cpp`,
  `CookieService.cpp`) : `Secure` refusé à la pose et à l'envoi seulement si l'origine
  n'est pas potentiellement fiable (`nsMixedContentBlocker::IsPotentiallyTrustworthyOrigin`,
  bouclage `127.0.0.0/8` et `::1` compris) ; `__Host-` exige `Secure`, l'hôte seul et
  `Path=/` explicite (bug 2066910, 2026-08).
- Observé, WebKit (PR #61338, 2026-04) : `Secure` sur le bouclage dans les builds Cocoa
  seulement (macOS 26 et suivants) ; ni dans les ports Linux, donc ni dans le WebKit de
  Playwright.

### Stockage : IndexedDB, BroadcastChannel, sessionStorage

- [IndexedDB] `#database-construct` : des bases par clé de stockage ; version initiale 0,
  changée seulement par une transaction de mise à niveau, exclusive
  (`#upgrade-transaction-construct`). `#database-connection` : `versionchange` est émis
  aux connexions ouvertes ; une connexion peut être fermée par l'agent utilisateur
  (effacement du stockage), événement `close`. `#handling-versionchange` : `blocked`
  tant qu'un autre client garde une connexion.
- [Storage] `#storage-keys` : clé de stockage = origine (« This is expected to change »,
  partitionnement). `#buckets` : seau « best-effort » par défaut. `#storage-pressure` :
  sous pression, l'agent utilisateur « should clear » les seaux best-effort, chacun en
  entier. `#persistence` : un seau persistant ne s'efface pas sans l'origine ou
  l'utilisateur (`navigator.storage.persist()`, permission « persistent-storage »).
  `#usage-and-quota` : le quota ne dépend pas de l'espace libre de l'appareil (MUST NOT).
- Observé : Firefox active IndexedDB en navigation privée, chiffrée
  (`dom.indexedDB.privateBrowsing.enabled`) ; Chromium et Safari **non vérifiés**.
- [HTML] `#broadcasting-to-other-browsing-contexts` : reçoit un objet « eligible for
  messaging » (document pleinement actif : pas en bfcache, et le message n'est pas gardé
  pour lui) de même clé de stockage et de même nom ; « Remove source from
  destinations. » : seul l'émetteur est exclu, les autres objets du même document
  reçoivent ; ordre seulement partiel ; remise en tâche.
- [HTML] `#copy-session-storage` : un contexte de premier niveau créé avec un opener
  reçoit une copie du stockage de session ; avec `noopener`, pas d'opener, pas de copie.
  [Storage] `#storage-sheds` : cette copie est « considered legacy » ; les seaux de
  session sont effacés à la fermeture de l'onglet. Ni HTML ni Storage ne parlent de la
  duplication d'onglet.
- Observé : Chromium (`DuplicateTabAt` puis `NavigationControllerImpl::CopyStateFrom`,
  qui clone l'espace de stockage de session) et Firefox (`SessionStore.duplicateTab`,
  `includePrivateData: true`) copient le `sessionStorage` dans l'onglet dupliqué.
- [WebCrypto] `[SecureContext] DOMString randomUUID();` (`getRandomValues` n'est pas
  réservé) : disponible sur `http://127.0.0.1`, contexte sécurisé.

### Accessibilité : WCAG 2.2 AA

Ancres : `https://www.w3.org/TR/WCAG22/#<nom-du-critère>`.

- 1.3.1 (A) structure et relations déterminables par programme ; 1.4.3 (AA) texte
  ≥ 4,5:1, grand texte ≥ 3:1 ; 1.4.10 (AA) pas de défilement à deux dimensions à 320 px
  de large, tables de données exceptées ; 1.4.11 (AA) composants, leurs états et
  graphiques ≥ 3:1 contre les couleurs adjacentes ; 1.4.12 (AA) espacement du texte
  forcé sans perte ; 1.4.13 (AA) contenu au survol ou au focus masquable, survolable,
  persistant.
- 2.1.1 (A) tout au clavier ; 2.1.2 (A) pas de piège ; 2.4.3 (A) ordre du focus ;
  2.4.7 (AA) focus visible ; **2.4.11** (AA) focus non masqué entièrement par le contenu
  de l'auteur ; **2.5.7** (AA) tout glissement réalisable au pointeur sans glisser (le
  clavier ne suffit pas, Understanding) ; **2.5.8** (AA) cibles ≥ 24 × 24 px CSS, sauf
  espacement ou équivalent.
- 3.3.1 (A) erreur identifiée et décrite en texte ; 3.3.3 (AA) suggestion de correction ;
  **3.3.8** (AA) authentification sans test cognitif ; 3.2.6 et 3.3.7 sont nouveaux mais
  de niveau A. 4.1.2 (A) nom, rôle, valeur ; 4.1.3 (AA) messages d'état sans prise de
  focus. 4.1.1 est « Obsolete and removed ».
- Définitions : rapport de contraste (L1 + 0,05) / (L2 + 0,05) ; luminance relative
  0,2126 R + 0,7152 G + 0,0722 B, composante c/12,92 si c ≤ **0,04045**, sinon
  ((c + 0,055)/1,055)^2,4 ; grand texte : 18 pt, ou 14 pt gras.
- Understanding 1.4.3 : pas d'arrondi (4,499:1 échoue). Understanding 1.4.11 : la bordure
  d'un champ, quand elle seule le signale, et l'indicateur de focus doivent atteindre
  3:1.

### WAI-ARIA, APG, axe-core

- [ARIA] `#aria-setsize` : MUST égal au nombre d'éléments ; « If the total number of
  items is unknown, authors SHOULD set the value of aria-setsize to -1 ».
  `#aria-posinset` : exige `aria-setsize`. `#treeitem` : sans `group` imbriqués,
  « authors MUST ensure all treeitem elements in the tree have explicit values for
  aria-level, aria-posinset, and aria-setsize ». `#tree` : MUST gérer le focus des
  descendants.
- [ARIA] `#aria-rowcount` : « If the total number of rows is unknown, authors MUST set
  the value of aria-rowcount to -1 » ; `#aria-rowindex` : entier ≥ 1, croissant, SHOULD
  sur chaque ligne. `#row` : une ligne de `table` ou `grid` MUST NOT porter
  `aria-expanded`, `aria-posinset`, `aria-setsize` ni `aria-level` (réservés à
  `treegrid`).
- [ARIA] `#aria-activedescendant` : l'élément désigné MUST être un descendant
  d'accessibilité de l'élément qui a le focus ; SHOULD être visible.
- [ARIA in HTML] : `<table>` a le rôle implicite `table` et admet tout rôle (`grid`
  compris), changement déconseillé ; `tr`, `td`, `th` n'admettent que le leur.
- [APG] Tree View : Bas, Haut ; Droite ouvre ou descend au premier enfant ; Gauche ferme
  ou remonte au parent ; Origine, Fin ; Entrée active ; recherche par premier caractère
  « recommended for all trees » ; `*` facultatif. `aria-level`, `aria-setsize`,
  `aria-posinset` sur chaque nœud quand l'arbre n'est pas entier dans le DOM. Aucune
  consigne pour une ligne « charger la suite ». Pratique « Keyboard interface » : focus
  itinérant (`tabindex`) ou `aria-activedescendant`, l'auteur dessinant alors
  l'indicateur et gérant le défilement.
- [APG] Table : structure statique, cellules non focalisables, aucun clavier ; pour du
  tabulaire interactif, Grid. Grid : un seul arrêt de tabulation, focus par cellule,
  flèches, Page, Origine, Fin, Ctrl+Origine, Ctrl+Fin ; le focus de ligne n'y est pas
  décrit. Treegrid : seul motif qui prévoit le focus de ligne.
- [axe-core] `doc/API.md` : chaque règle porte le tag de la version qui l'introduit ;
  `wcag2a`, `wcag2aa`, `wcag21a`, `wcag21aa`, `wcag22aa` sont tous nécessaires. La seule
  règle 2.2 est `target-size` (2.5.8), désactivée par défaut mais lancée quand son tag
  est demandé (`lib/core/utils/rule-should-run.js`, **non vérifié** en navigateur).
  Aucune règle pour 1.4.11, 2.4.7 ni 2.4.11 ; les règles `best-practice` ne tournent pas
  avec ces seuls tags. README : « on average 57% of WCAG issues ». Les cas non tranchés
  vont dans `incomplete`, pas dans `violations`. `doc/developer-guide.md` : racines
  d'ombre ouvertes prises en charge, fermées non.

### vue-i18n sans `eval`

- [vue-i18n] README : v11 stable ; page « Maintenance » : v9 et v10 « in maintenance mode
  after 2025 July » ; Legacy API dépréciée en v11, retirée en v12 (alpha, Vue ≥ 3.6).
- « Optimization » : avant 9.3, le build complet ne marche pas sous CSP (`eval`) ;
  « From v9.3, JIT compilation works around the CSP issue » ; JIT par défaut en v10
  (« breaking10 »). Code : v9.14.5 `compileToFunction` appelle `new Function` ; v11.1.10
  `core-base/src/compilation.ts` compile en AST, sans `eval` ni `new Function` (deux
  fichiers lus, pas tout le paquet).
- Build : le compilateur de messages n'est présent que dans le build complet et sans
  `__INTLIFY_DROP_MESSAGE_COMPILER__`. Sans lui, un message en chaîne est rendu tel quel,
  avec un avertissement en développement seulement.
- [`@intlify/unplugin-vue-i18n`] : `runtimeOnly` vaut `true` par défaut (build sans
  compilateur en production) ; `dropMessageCompiler` vaut `false` (« les ressources
  chargées dynamiquement ne marcheront pas » s'il est vrai) ; `strictMessage` (`true`)
  et `escapeHtml` (`false`) agissent au build sur les fichiers inclus.
- HTML dans les messages : « Only use HTML interpolation on trusted content » ;
  `escapeParameter` vaut `false`, `warnHtmlMessage` avertit en développement.

### Prise en charge par les navigateurs (REQ-007-37)

Versions au 2026-10-20 selon [BCD] (calendriers officiels **non relus**) : Chromium 155
(156 le jour même), Firefox 158 et 157, Firefox ESR 153, Safari 27 et 26.6. Chromium et
Firefox sortiraient une version toutes les deux semaines depuis Chromium 153 et
Firefox 155 (**non vérifié**) ; l'ESR 140 serait hors support (**non vérifié**).

| Fonctionnalité | Chromium | Firefox | Safari | Dans les versions visées |
|---|---|---|---|---|
| import maps | 89 | 108 | 16.4 | oui |
| `integrity` d'import map | 127 | 138 | 18 | oui |
| plusieurs import maps | 133 | derrière `dom.multiple_import_maps.enabled` | 18.4 | **non** (Firefox) |
| `modulepreload` | 66 | 115 | 17 | oui ; `integrity` sur `modulepreload` **non vérifié** hors Chromium |
| `integrity` sur `<script type=module>` | 45 | 43 | 11.1 | oui |
| Custom Elements autonomes, Shadow DOM | 54, 53 | 63, 63 | 10.1, 10 | oui |
| `adoptedStyleSheets`, `ElementInternals` | 73, 77 | 101, 93 | 16.4, 16.4 | oui |
| ES2022 : champs et méthodes privés, blocs statiques, `at`, `Object.hasOwn`, `cause` | ≤ 94 | ≤ 93 | ≤ 16.4 | oui |
| `await` de premier niveau | 89 | 89 | 15 partiel, 27 complet | **non** (Safari 26) |
| `BroadcastChannel`, `structuredClone` | 54, 98 | 38, 94 | 15.4, 15.4 | oui |
| `AbortSignal.timeout`, `crypto.randomUUID` | 124, 92 | 100, 95 | 16, 15.4 | oui (`randomUUID` en contexte sécurisé) |
| `Intl.supportedValuesOf('timeZone')` | 99 | 93 | 15.4 | oui, sans `UTC` sous Chromium |
| `Sec-Fetch-Site` | 76 | 90 | 16.4 | oui |
| `prefers-color-scheme`, `:focus-visible`, imbrication CSS, `color-mix()` | ≤ 120 | ≤ 117 | ≤ 17.2 | oui |

- [Fetch Metadata] « append the Fetch metadata headers for a request » : « If |r|'s
  current URL is not an potentially trustworthy URL, return. » : pas de `Sec-Fetch-*`
  vers une URL `http` hors bouclage.
- [WPT] et [BCD] : l'empreinte CSP d'une import map a ses tests (`import-maps/csp/`),
  résultats par moteur **non relus**. Safari 26.6 bloque les modules d'une import map
  sous l'en-tête `Integrity-Policy` (que kartenn n'emploie pas).

## Pièges

- L'empreinte de l'import map porte sur le texte DOM, après normalisation des fins de
  ligne : un `index.html` en CRLF haché en octets ne correspond pas.
- Une import map refusée par la CSP est ignorée en silence (seule la violation la
  signale) : les imports nus (`@kartenn/sdk`) échouent ensuite.
- `'unsafe-inline'` n'a plus d'effet dès qu'une empreinte est présente.
- `style-src` bloque `style="…"` posé par l'analyseur, `innerHTML` ou `setAttribute`, pas
  `el.style.*` ; l'ED dit autre chose pour le CSSOM (#212). La directive rapportée est
  `style-src-attr` ou `style-src-elem`.
- `form-action` sur une redirection : Chromium la contrôle, Firefox et WebKit non ; un
  test de déconnexion vers une autre origine ne prouve rien sur Firefox.
- `securitypolicyviolation` part en tâche : lire les violations après une macrotâche.
- L'`integrity` d'un `<script type=module>` ne couvre pas ses imports ; `load` d'un
  `modulepreload` ne dit rien de ses dépendances ; un préchargement en échec ne protège
  plus l'URL (whatwg/html#10327).
- Une valeur `integrity` mal formée désactive le contrôle sans erreur.
- `fetch` rejette un `TypeError` sans dire si c'est le réseau ou l'empreinte.
- Registre des éléments global : une définition ne se retire pas ; deux plugins qui
  visent le même nom se heurtent à `NotSupportedError`.
- Un événement émis dans une racine d'ombre sans `composed: true` n'atteint pas l'hôte.
- ARIA : `aria-setsize="-1"` est un SHOULD, `aria-rowcount="-1"` un MUST ; un arbre plat
  virtualisé exige `aria-posinset` sur chaque nœud ; une `row` de table ne porte jamais
  `aria-level` ni `aria-expanded`.
- `<table>` à ligne focalisable et flèches ne correspond à aucun motif de l'APG tel quel.
- axe-core : un test « zéro violation » ne voit pas les `incomplete` (contrastes douteux,
  Shadow DOM, pseudo-éléments) ; une racine d'ombre fermée lui échappe.
- vue-i18n : avec `runtimeOnly: true` (défaut) ou `dropMessageCompiler: true`, les
  messages des plugins s'affichent bruts (`{name}` non remplacé), sans erreur en
  production. `strictMessage` ne couvre pas un JSON récupéré par `fetch`.
- La documentation de la branche principale de vue-i18n décrit la v12 alpha.
- Safari 15 à 26 : un module à `await` de premier niveau importé par plusieurs modules
  peut échouer (le SDK partagé par plusieurs plugins y serait exposé).
- « Deux dernières versions » à une sortie toutes les deux semaines couvre environ un
  mois (**non vérifié**).
- 6265bis ne garantit rien sur le bouclage : « secure » est laissé à l'agent
  utilisateur ; la garantie vient du code des moteurs.
- `localhost`, `127.0.0.1` et `[::1]` sont trois origines (stockage, cookies) ;
  `localhost` n'est potentiellement fiable que par un MAY.
- Les cookies ignorent le port : `__Host-kartenn` posé par `:5173` et par `:9443` est le
  même cookie dans un même profil.
- Vite atteint par une adresse du réseau local : origine non fiable, cookie `Secure`
  refusé sans erreur visible, `crypto.randomUUID` absent.
- WebKit sous Linux (Playwright) refuse un cookie `Secure` en HTTP sur le bouclage.
- Sans `db.close()` sur `versionchange`, la mise à niveau d'IndexedDB lancée par un autre
  onglet reste `blocked`.
- `BroadcastChannel` n'a pas d'ordre global : deux onglets dupliqués peuvent se détecter
  en même temps ; un document en bfcache ne reçoit rien.

## Implications pour kartenn (propositions, à trancher dans la spec 007)

Légende : **[N]** normatif ; **[T]** choix à trancher.

CSP (design « Service du shell ») :

1. [N] L'empreinte de l'import map suffit : `script-src-elem` absent retombe sur
   `script-src`, aucune autre source n'est nécessaire. Confirme le design.
2. [N] Empreinte calculée sur le texte entre `>` et `</script>` après CR et CRLF → LF
   (et U+0000 → U+FFFD), en UTF-8, base64 avec `=`. Précise le design ; test avec un
   `index.html` en CRLF (tâche 3).
3. [N] `style-src 'self'` bloque `<style>`, racines d'ombre comprises (inférence), et les
   attributs `style` analysés ; pas le CSSOM ni `adoptedStyleSheets` (moteurs). Confirme
   le design « Styles » ; les parcours, qui échouent sur toute violation, surveillent
   l'écart avec l'ED. Pour un plugin, c'est une règle de thème, pas de sécurité (W4).
4. [N] Pas de `form-action` : justifié, HTML soumet la redirection d'un formulaire à
   `form-action` et Chromium l'applique. [T] Alternatives, si l'on veut la directive :
   liste des origines des émetteurs calculée à la configuration, ou 303 vers une route
   de même origine puis navigation par script.
5. [T] Parcours : écouteur `securitypolicyviolation` sur `window` posé par
   `addInitScript`, en capture, lu après une macrotâche ; `effectiveDirective` et
   `blockedURI` au message d'échec.
6. [T] `blob:` (images) et WebAssembly sont refusés par la CSP du design ; à rouvrir
   seulement si un plugin en a besoin.

SRI et chargement des plugins (design « Chargement des plugins par le shell ») :

7. [N] L'`integrity` du `<script>` d'entrée ne couvre que lui ; un autre module n'est
   vérifié que par son `modulepreload` à empreinte, entré le premier dans la carte des
   modules, ou par une entrée `integrity` d'import map.
8. [T] Ordre : insérer d'un coup un `modulepreload` à empreinte par module du plugin
   (chunks compris), puis le `<script>` d'entrée seulement après leurs `load` à tous ;
   un `error` écarte le plugin sans insérer le script. Sans cet ordre, un `import`
   suivant un préchargement en échec referait la requête sans empreinte.
9. [T] `integrity` d'import map : couvrirait aussi les modules récupérés hors
   préchargement. Elle exigerait une import map rendue par réponse (empreinte CSP par
   réponse) ou une seconde import map (Firefox : derrière un drapeau). Écartée pour
   l'instant, à rouvrir quand Firefox acceptera plusieurs import maps.
10. [T] Le shell vérifie la forme des empreintes publiées (`sha384-` et 64 caractères
    base64) et écarte un plugin mal décrit, puisqu'une valeur invalide désactive le
    contrôle.
11. [N] `kartenn-close` émis dans une racine d'ombre : `bubbles: true, composed: true`
    (SDK et documentation des plugins).
12. [N] `definePlugin` relaie `TypeError`, `SyntaxError` et `NotSupportedError` de
    `define` ; le préfixe `<name>-` des éléments (design REQ-007-5) évite les collisions
    entre plugins. [T] Racines d'ombre ouvertes pour les plugins de première partie
    (axe-core).

Cookies et stockage (design « Session », « Écriture », « Bout en bout et
développement ») :

13. [N] Cookie `__Host-kartenn` : `Secure; HttpOnly; SameSite=Strict; Path=/`, `Path`
    explicite, sans `Domain` (6265bis, étape 21) ; c'est déjà la forme de
    `kartenn-rest::cookies`. Sur `http://127.0.0.1:5173`, contexte sécurisé, Chromium et
    Firefox posent et renvoient ce cookie (code) ; 6265bis ne le garantit pas.
    [T] Vite écoute sur `127.0.0.1` seul : ni `localhost` (autre origine, simple MAY), ni
    une adresse du réseau local (origine non fiable, cookie refusé sans erreur visible).
    L'acceptation de REQ-007-36 (connexion à travers Vite) sert de garde ; repli : Vite en
    HTTPS avec le certificat de développement. WebKit seulement en HTTPS. Les cookies
    ignorent le port : le cookie de `:5173` et celui de `:9443` se remplacent dans un
    même profil de navigateur, d'où des profils séparés en usage manuel.
14. [T] Brouillon : `db.close()` sur `versionchange`, reprise sur `close` ; le seau est
    best-effort, donc évictable : demander `navigator.storage.persist()` ou avertir.
    Identifiant d'onglet : annoncé sur `BroadcastChannel` au chargement et sur `pageshow`
    (`persisted`), avec un nonce ; en cas de collision, le plus récent tire un nouvel
    identifiant (pas d'ordre global entre onglets). Les onglets ouverts par le shell
    portent `noopener`. `crypto.randomUUID` est disponible ; hors contexte sécurisé, le
    shell échoue explicitement.

Accessibilité (design « Thème, composants, arbre ») :

15. [N] Contrastes : REQ-007-33 est conforme (4,5:1 ; 3:1 pour bordures de champ et
    anneau de focus ; sans arrondi). [T] 4,5:1 pour tout texte, sans l'exception du
    grand texte ; anneau mesuré contre ses deux voisins (fond de page et du composant).
16. [N] Arbre virtualisé plat : `aria-posinset` s'ajoute à `aria-level` et
    `aria-setsize` sur chaque nœud (MUST) ; `aria-setsize="-1"` tant que la dernière page
    n'est pas lue, puis la taille réelle ; la ligne active reste dans le DOM, avec un `id`
    stable, même hors de la fenêtre virtuelle (`aria-activedescendant`).
17. [T] Ligne « suite » (absente de l'APG) : `treeitem` du même niveau, hors de la
    numérotation `aria-posinset`, Entrée charge la page, annonce dans une région d'état
    (4.1.3).
18. [T] Tables à ligne focalisable : rôle `table` conservé avec liens natifs et Tab
    seulement (motif Table) ; `role="grid"` sur le `<table>` avec focus de ligne et
    flèches haut et bas (écart à l'APG Grid) ; ou Grid complet par cellule. Le design
    décrit la deuxième sans `role="grid"`.
19. [N] 2.5.8 (cibles ≥ 24 px : chevrons, actions) ; 2.5.7 (glisser-déposer doublé d'une
    action au pointeur) ; 2.4.11 (`scroll-padding` face aux en-têtes collants).
20. [T] `expectAccessible` : cinq tags (lance `target-size`) ; échec aussi sur les
    `incomplete` de `color-contrast` sauf liste motivée ; `best-practice` en option ;
    parcours en mode clair et sombre.

i18n et navigateurs (design « i18n », « Bout en bout et développement ») :

21. [N] vue-i18n ≥ 10 n'emploie ni `eval` ni `new Function` (JIT). [T] Viser la 11, la 10
    étant en maintenance ; composition API seulement.
22. [N] Build de production avec le compilateur de messages : `runtimeOnly: false`,
    `dropMessageCompiler: false`. [T] Test qu'un message de plugin à paramètre est
    interpolé dans le build de production.
23. [T] Catalogue de plugin contenant `<` refusé au chargement ; `escapeParameter`
    activé ; pas de `v-html` sur un message.
24. [T] REQ-007-37 : plancher de versions daté, tiré du tableau ci-dessus, plutôt que
    « deux dernières versions » ; ESR = branche en cours (153).
25. [T] Pas d'`await` de premier niveau dans le shell, le SDK et les plugins tant que
    Safari 26 est visé (règle ESLint).
26. [T] Safari : `modulepreload` à empreinte non établi par les sources ; contrôle
    manuel en l'absence de parcours.
27. [T] `Sec-Fetch-Site` manque vers une origine `http` hors bouclage : le repli sur
    `Origin` ([api-http.md](api-http.md), point 14) vise aussi ce cas.

## Risques du design (spec 007)

- **Cookie `Secure` sur `http://127.0.0.1`** : levé pour Chromium et Firefox (point 13).
  `127.0.0.0/8` est potentiellement fiable sans condition (Secure Contexts) et leur code
  pose et renvoie `Secure` et `__Host-` sur une telle origine ; 6265bis laisse la notion
  à l'agent utilisateur. WebKit sous Linux ne le fait pas (HTTPS seulement).
- **vue-i18n et CSP** : levé, à condition que le build de production garde le
  compilateur de messages (points 21 et 22).
- **`modulepreload` avec empreinte** : reformulé. Le risque n'est pas un navigateur qui
  ignorerait `integrity` (Chromium et Firefox l'appliquent, Safari non établi), mais un
  module récupéré hors de son préchargement : dépendance préchargée en parallèle ou
  `import` après un préchargement en échec (points 7 à 9).

## Non trouvé ou non relu

- Pages rendues des normes : non joignables ; numéros de section non relevés, ancres à
  recouper. SRI Level 1, WAI-ARIA 1.2 (REC), errata de WCAG 2.2 : non relus.
- CSP3 : statut exact du WD ; étapes de `#should-block-inline` ; raison pour laquelle une
  navigation de premier niveau échappe à `connect-src` ; tests de `<style>` et `<link>`
  en racine d'ombre et de `adoptedStyleSheets` sous `style-src` (aucun trouvé).
- Trackers de Chromium, Firefox et WebKit non joignables : `form-action` sur redirection
  déduit des attentes WPT ; correctifs de whatwg/html#10327 par moteur non suivis.
- Comportement réel, à éprouver par les parcours : préchargement en échec puis `import` ;
  récupération des descendants d'un `modulepreload` par Firefox et Safari ;
  `integrity` sur `modulepreload` hors Chromium ; empreinte CSP d'une import map par
  moteur.
- APG : motifs Dialog, Menu, Combobox, Tabs ; prise en charge de `aria-setsize="-1"`
  par les lecteurs d'écran.
- vue-i18n : absence de `new Function` dans tout le paquet 11.4 ; nom du fichier du build
  complet et alias de l'unplugin 11.2.7.
- Calendriers officiels de Chromium, Firefox et Safari ; fin de l'ESR 140 et de l'ESR 153.
- Cookies : version de Chromium qui accepte les préfixes sur le bouclage, de Firefox qui
  accepte `Secure` sur `localhost`, de Safari qui porte la PR #61338 ; `__Host-` sous
  WebKit ; tests WPT des cookies et leurs attentes (aucun ne vise le bouclage : WPT sert
  sur un nom de domaine) ; état de publication de 6265bis.
- Stockage : IndexedDB en navigation privée sous Chromium et Safari ; quotas par moteur ;
  heuristiques de `persist()` ; `BroadcastChannel` et bfcache par moteur.
