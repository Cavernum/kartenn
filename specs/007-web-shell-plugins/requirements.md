---
status: approved
updated: 2026-10-19
---

# 007 — Shell Web et plugins : exigences

## Résumé

Le shell est l'application Web de kartenn : il rend exploitables sans outil annexe les
données du noyau, le cycle complet des changesets et l'administration (droits, audit,
imports). Il ne parle qu'à l'API REST (D7), sur la même origine qu'elle, avec la
session du navigateur de la 005 (ADR 0020). Il accueille des plugins : modules ES
déposés par l'exploitant, chargés à chaud, qui enregistrent des Custom Elements aux
points d'extension d'un SDK public et versionné (D6, D17, ADR 0011). Les écrans des
modules métier (008, 009, puis Politique et Flux) seront de tels plugins. Cette spec
fixe le service du shell et des plugins par kartenn-server, leur modèle de confiance,
le SDK et son client HTTP, le contexte temporel, les parcours d'écriture, les vues de
la v0.1, l'internationalisation, l'accessibilité, les budgets du navigateur et les
tests de bout en bout.

Décisions : [`decisions.md`](decisions.md) (W1–W28) ; D6, D7, D14, D17, D31, D37
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; Q-ouv-1 (SDK en
MPL-2.0), Q-ouv-6 (i18n, FR et EN) ; ADR 0011, 0020, 0022, 0024 ; 006 (surface et
conventions de l'API). Sources :
[`docs/reference/plateforme-web.md`](../../docs/reference/plateforme-web.md),
[`docs/reference/api-http.md`](../../docs/reference/api-http.md).

### Vocabulaire

- **Shell** : application Web Vue 3 servie par kartenn-server ; elle porte la
  navigation, les vues de la v0.1 et l'hôte des plugins.
- **Exploitant** : personne qui installe et configure kartenn (fichiers, paquets) ; il
  n'agit pas par l'API.
- **Plugin** : répertoire déposé par l'exploitant (manifeste et modules ES) dont le
  module d'entrée enregistre des Custom Elements.
- **Plugin de première partie** : plugin écrit dans le dépôt (`web/plugins/<module>/`)
  avec le seul SDK public, livré dans le paquet.
- **Manifeste** : fichier JSON d'un plugin : nom, version, module d'entrée, plage de SDK
  attendue, extensions, catalogues.
- **SDK** : paquet `@kartenn/sdk` (MPL-2.0) : types, client API, contexte, points
  d'extension, i18n, brouillon, bac à sable de test.
- **Extension** : contribution d'un plugin à un point d'extension (W5).
- **Contexte temporel** : couple (`valid_at`, `as_of`) de la vue ; « maintenant » quand
  les deux sont absents (D10, REQ-006-5).
- **Modification rapide** : écriture d'un objet depuis sa fiche par la route de la
  ressource, un changeset d'une opération (REQ-006-12).
- **Brouillon** : opérations accumulées dans le navigateur, simulées en continu,
  soumises en un seul changeset (W10).
- **Jeton de design** : propriété personnalisée CSS (`--k-…`) définie par le shell.

### Conventions

- Forme EARS en français, comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).
  « kartenn » désigne le serveur ; « le shell » l'application qui s'exécute dans le
  navigateur.
- Les références entre crochets renvoient aux décisions (W*) et aux sources de la note
  de référence, par l'ancre de leur brouillon d'éditeur ou le titre de leur section :
  [CSP3 §ancre] Content Security Policy Level 3, [SRI §ancre] Subresource Integrity,
  [HTML §section] WHATWG HTML (import maps, Custom Elements), [CSS Cascade 5 §ancre],
  [WCAG 2.2 x.y.z] critère de succès, [APG] WAI-ARIA Authoring Practices, [SemVer §n]
  Semantic Versioning 2.0.0, [node-semver] plages de versions, [Clear-Site-Data], et
  celles de la 006.
- Les budgets se mesurent par Playwright sur Chromium, CPU ralenti ×4, sur le jeu de
  données de `bench-core` (10⁶ objets) (W13).

## Exigences

### A. Service du shell

#### REQ-007-1 — Shell servi par kartenn
Là où `[web] root` est configuré, kartenn doit servir le build du shell depuis ce
répertoire, sur l'origine publique, hors `/v1`, `/health` et `/plugins` ; sans
`[web]`, kartenn ne doit servir que l'API. Un `root` absent ou sans `index.html` doit
refuser le démarrage. [W2, ADR 0020]

Acceptation :
- `GET /` renvoie `index.html` du build ; `GET /v1/capabilities` reste servi par l'API.
- Configuration sans `[web]` : `GET /` → 404.

#### REQ-007-2 — Routes du client
Quand une navigation (`GET` d'un document) vise un chemin hors `/v1`, `/health`,
`/plugins` et hors fichier du build, kartenn doit répondre `index.html` (200) ; une
requête d'un fichier absent (sous-ressource) doit recevoir 404, et un chemin inconnu
sous `/v1` le problème 404 de l'API. Aucun chemin ne doit sortir du répertoire servi.
[W17, RFC 9110 §15.5.5]

Acceptation :
- `GET /prefixes/<uuid>` en navigation → `index.html` ; `GET /assets/absent.js` → 404 ;
  `GET /v1/absent` → `application/problem+json`.
- `GET /assets/..%2f..%2fetc/passwd` et un lien symbolique sortant du répertoire → 404.

#### REQ-007-3 — En-têtes de sécurité
kartenn doit poser sur `index.html`, en en-tête et non en `<meta>`, une CSP qui
n'autorise que des scripts de la même origine (ni `unsafe-eval`, ni `'strict-dynamic'`,
aucun script en ligne hors l'import map désignée par son empreinte), des connexions vers
la seule origine (`connect-src 'self'`), aucun objet (`object-src 'none'`), aucun
ancêtre (`frame-ancestors 'none'`), `base-uri 'none'` et `form-action 'self'` ; et sur
toute réponse du shell et des plugins `X-Content-Type-Options: nosniff` et
`Referrer-Policy: same-origin`. Le shell ne doit charger aucune ressource d'une autre
origine (scripts, styles, polices, images, mesures), ni style en ligne (`style-src
'self'`). [W2, W4, W17, W19, CSP3
§directive-script-src, §directive-frame-ancestors, §should-block-inline, INV-23]

Acceptation :
- Parcours E2E : aucune requête vers une autre origine ; aucune violation de CSP
  signalée par le navigateur.
- Un plugin d'essai qui appelle `fetch` vers une autre origine, `eval` ou insère un
  `<script>` en ligne est bloqué par la CSP.

#### REQ-007-4 — Cache des fichiers
kartenn doit servir les fichiers du build dont le nom porte une empreinte avec
`Cache-Control: public, max-age=31536000, immutable`, `index.html` avec `no-cache` et un
ETag, et les fichiers des plugins avec `no-cache` et un ETag fort égal à leur empreinte ;
une requête conditionnelle inchangée doit recevoir 304. [W17, RFC 9111 §5.2.2, RFC 8246]

Acceptation :
- Après remplacement d'un plugin sous la même version, le navigateur reçoit les
  nouveaux octets à la requête suivante (pas d'ancienne copie en cache).

#### REQ-007-5 — Navigateurs pris en charge
Le shell doit fonctionner sur les deux dernières versions majeures de Firefox, Chromium
et Safari, et sur Firefox ESR, sans polyfill d'import maps ni de Custom Elements ; il
suppose au moins les import maps avec la clé `integrity` (Chromium 127, Firefox 138,
Safari 18). Sur un navigateur qui ne les prend pas en charge, il doit afficher un
message qui le dit, sans écran blanc. [W17, HTML §import maps]

#### REQ-007-6 — Nouvelle version du shell
Si le chargement d'un fragment du shell échoue parce que la version servie a changé,
alors le shell doit proposer de recharger la page, sans perdre le brouillon (REQ-007-29).
[W2]

Acceptation :
- Build remplacé pendant une session : la navigation vers une route non encore chargée
  affiche l'invitation à recharger ; après rechargement, le brouillon est intact.

### B. Plugins : installation et chargement

#### REQ-007-7 — Répertoire des plugins
Là où `[web] plugins_dir` est configuré, kartenn doit y lire un plugin par
sous-répertoire contenant un manifeste valide et servir ses fichiers sous
`/plugins/<nom>/<version>/`, nom et version tirés du manifeste ; seuls les types de
fichiers d'une liste fermée (modules ES, CSS, JSON, images, polices) doivent être
servis, avec leur type de média, et à une requête authentifiée seulement. Les octets
servis doivent être ceux dont l'empreinte est publiée (REQ-007-9). Sans session valide,
`index.html` ne doit lister aucun plugin. [W3, W21]

Acceptation :
- Un plugin `exemple` version `1.2.0` est servi sous `/plugins/exemple/1.2.0/` ; un
  fichier `.sh` du même répertoire → 404.
- Sans session : `GET /plugins/exemple/1.2.0/index.js` → 404, et `index.html` ne nomme
  pas `exemple`.

#### REQ-007-8 — Manifeste
kartenn doit refuser un manifeste qui n'est pas conforme au schéma du manifeste publié
avec le SDK : nom (`[a-z][a-z0-9-]*`, hors `k` et `kartenn`, réservés au shell),
version SemVer, module d'entrée, plage de SDK (REQ-007-12), extensions, noms de tous
ses Custom Elements, catalogues, module requis facultatif. Chaque nom de Custom Element
doit commencer par le nom du plugin suivi d'un tiret, et ses clés de traduction par son
espace de noms. Deux plugins de même nom, ou qui déclarent un même nom de Custom Element,
doivent être écartés tous les deux, sauf un plugin de première partie, qui garde ses
noms. [W3, W5, W6, W11, SemVer §2, HTML §custom elements]

Acceptation :
- Manifeste sans `sdk` → plugin écarté, log `error` qui nomme le fichier et le champ ;
  les autres plugins restent publiés.
- Un plugin tiers nommé comme un plugin de première partie est écarté.
- Plugins `foo` et `foo-bar` déclarant tous deux `foo-bar-vue` : les deux sont écartés.

#### REQ-007-9 — Publication dans les capacités
kartenn doit publier dans `ui_plugins` de `GET /v1/capabilities`, pour chaque plugin
retenu : nom, version, URL du module d'entrée, empreinte SRI de chaque module, plage de
SDK, extensions déclarées, catalogues, module requis. Un plugin écarté ne doit pas y
figurer ; la raison en est journalisée. [W3, W17, REQ-006-35, SRI]

Acceptation :
- Le contrat OpenAPI décrit chaque champ ; `ui_plugins` est vide sans `plugins_dir` et
  sans plugin de première partie.

#### REQ-007-10 — Rechargement à chaud
Quand l'exploitant ajoute, remplace ou retire un plugin, kartenn doit publier le nouvel
ensemble dans les 30 s, ou dès `SIGHUP`, sans redémarrer ni reconstruire ; un shell
déjà ouvert doit garder les plugins de son document jusqu'au rechargement de la page.
[W3, W20, D6]

Acceptation :
- Plugin copié dans le répertoire d'une instance en marche : il figure dans les
  capacités 30 s après au plus, et le shell le charge au rechargement suivant.
- Plugin retiré : il disparaît des capacités ; ses fichiers → 404.
- Plugin remplacé sur une instance sur deux : un shell qui reçoit `index.html` de l'une
  et les modules de l'autre marque le plugin en échec, sans autre effet ; tout rentre
  dans l'ordre au rechargement une fois les instances alignées.

#### REQ-007-11 — Intégrité des modules
kartenn doit publier une empreinte (SHA-384 au moins) pour chaque module servi, du
shell comme des plugins, module d'entrée et fragments ; le shell ne doit exécuter un
module de plugin que si ses octets ont l'empreinte publiée (REQ-007-9) ; si l'empreinte
diffère ou manque, alors le plugin doit être écarté. [W3, W4, SRI §hash-functions,
§agility, HTML §fetching scripts]

Acceptation :
- Un module altéré entre le serveur et le navigateur (proxy d'essai qui en change un
  octet) n'est pas exécuté ; le plugin est marqué en échec, le shell reste utilisable.

#### REQ-007-12 — Compatibilité du SDK
Si la plage de SDK d'un plugin n'inclut pas la version du SDK servie, alors le shell ne
doit pas le charger et doit le signaler dans la liste des plugins (REQ-007-13). Une
plage s'écrit selon node-semver, restreinte à une version exacte, `^`, `~` et aux
comparateurs `>=` et `<` ; `||`, les tirets et les préversions sont refusés par le
schéma du manifeste. [W7, SemVer §4, node-semver]

Acceptation :
- SDK servi `1.3.0` : plugin `^1.2` chargé ; `~1.2.0` et `^2.0` écartés avec la raison
  « SDK incompatible ».

#### REQ-007-13 — Échec isolé
Si le module d'entrée d'un plugin échoue, lève une exception ou n'a pas fini de se
charger en 5 s, alors le shell doit l'écarter sans retarder son premier écran ; si une
extension lève une exception au rendu, alors son emplacement doit afficher une erreur
et le reste de la page fonctionner ; si un plugin définit un Custom Element que son
manifeste ne déclare pas ou qui est déjà défini, alors le plugin doit être marqué en
échec. Le shell doit montrer à chaque utilisateur l'état de chaque plugin publié :
chargé, incompatible, en échec, expiré. [W13, W7, HTML §custom elements]

Acceptation :
- Plugin qui boucle 10 s à son chargement : premier écran dans son budget
  (REQ-007-57), plugin « expiré ».
- Onglet de plugin qui lève au rendu : message d'erreur dans l'onglet, les autres
  onglets s'ouvrent.

#### REQ-007-14 — Droits d'un plugin
Un plugin doit n'agir que par l'API, avec la session de l'utilisateur : le shell ne doit
lui donner aucun moyen qui dépasse les droits de cet utilisateur, et le serveur reste
seul juge de chaque requête. [W4, INV-23, D7]

Acceptation :
- Un plugin d'essai qui appelle une route que l'utilisateur n'a pas le droit d'utiliser
  reçoit le même refus que le shell (403 ou 404) ; il ne lit pas le cookie de session.
- La documentation de l'exploitant dit qu'installer un plugin revient à lui confier la
  session de chaque utilisateur (W4).

### C. SDK et client API

#### REQ-007-15 — SDK partagé
Le shell doit déclarer une seule import map, avant tout chargement de module, où
`@kartenn/sdk` désigne le module du SDK qu'il sert ; aucune import map ne doit être
ajoutée ensuite, par le shell ni par un plugin. Les plugins importent le SDK
statiquement et partagent ainsi une seule instance du contexte. [W7, HTML §import maps]

Acceptation :
- Deux plugins lisent le même contexte (utilisateur, contexte temporel) et voient le
  même changement au même instant.

#### REQ-007-16 — Contenu du SDK
Le SDK doit fournir : le client API (REQ-007-20, 21), le contexte (utilisateur et
droits, contexte temporel, objet courant, langue), l'enregistrement aux points
d'extension, la navigation du shell, l'i18n et les formats (REQ-007-50), l'ajout
d'opérations au brouillon (REQ-007-34), les notifications à l'utilisateur, et le bac à
sable de test (REQ-007-22). [W7, D17, W11]

#### REQ-007-17 — Contrat du SDK versionné
Le SDK doit suivre SemVer : retirer ou changer de façon incompatible une déclaration
publique (types, fonctions, jetons de design, schéma du manifeste) exige une version
majeure. Un test doit comparer la surface publique du SDK à une référence du dépôt et
échouer sur tout retrait ou changement incompatible sans version majeure. La première
version publiée du SDK doit être `1.0.0`, pour que `^1.y` borne les ruptures. [W7,
SemVer §4, §8, REQ-006-34]

#### REQ-007-18 — Points d'extension
Le SDK doit offrir quatre points d'extension, déclarés dans le manifeste : (1) route et
vue pleine page avec entrée de navigation ; (2) onglet sur la fiche d'un type d'objet ;
(3) panneau ou colonne sur la fiche et dans les listes d'un type d'objet ; (4) action du
menu d'un objet. Chacun est rendu par un Custom Element du plugin, qui reçoit l'objet
et le contexte. [W5]

Acceptation :
- Le plugin d'exemple (REQ-007-64) contribue à chacun des quatre points.

#### REQ-007-19 — Filtrage des extensions
Le shell doit n'afficher une extension que sur les types d'objets qu'elle déclare et
seulement si sa permission déclarée est accordée à l'utilisateur sur au moins un
périmètre (REQ-007-67) ; une action dont la permission n'est pas accordée sur le
périmètre de l'objet doit être masquée ou grisée quand le shell peut le savoir, et le
refus du serveur traité sinon. [W5, W17, 005/REQ-005-18]

Acceptation :
- Onglet déclaré pour `prefix` avec la permission `core.prefix.read` : absent de la fiche
  d'une adresse, absent pour un utilisateur sans cette permission.

#### REQ-007-20 — Client API typé
Le SDK doit fournir des types générés depuis le contrat de référence
(`specs/006-api-rest/openapi-v1.json`) sans code d'exécution généré ; un test doit
régénérer les types et échouer s'ils diffèrent de ceux du dépôt. [W8, REQ-006-34]

#### REQ-007-21 — Conventions HTTP du client
Le client du SDK doit : envoyer les requêtes à la même origine avec le cookie de session
et jamais d'en-tête `Authorization` ; recherches par `QUERY` (REQ-006-10) ; suivre les
curseurs (`Link: rel="next"`) ; envoyer `If-Match` avec l'ETag de la version lue
(REQ-006-14) ; donner une `Idempotency-Key` à chaque soumission et la réutiliser à
chaque nouvel essai de la même soumission (REQ-006-15) ; rendre chaque problème
(RFC 9457) comme une erreur typée par son `type`, avec l'identifiant de requête ;
transmettre un 401 au shell (REQ-007-44). [W8, H2, H5, H8, RFC 9457 §3]

Acceptation :
- Réponse 412 → erreur « conflit de version » ; coupure réseau puis nouvel essai →
  même `Idempotency-Key`.

#### REQ-007-22 — Bac à sable de test
Le SDK doit fournir de quoi monter l'extension d'un plugin hors du shell, avec un
contexte choisi et un `fetch` simulé typé par le contrat, pour les tests de composants
(Vitest). [W7, W15]

Acceptation :
- Les tests du plugin d'exemple utilisent le bac à sable, sans serveur.

### D. Contexte temporel et navigation

#### REQ-007-23 — État dans l'URL
Le shell doit porter dans l'URL la vue, l'objet affiché, `valid_at`, `as_of`, les
filtres et l'onglet ouvert ; ouvrir la même URL doit montrer la même vue au même
contexte temporel, aux droits du lecteur près ; précédent et suivant du navigateur
doivent rejouer la navigation. [W9, W17]

Acceptation :
- Une URL copiée avec `as_of` et deux filtres, ouverte par un autre utilisateur de
  mêmes droits, montre les mêmes lignes.

#### REQ-007-24 — Bandeau hors présent
Tant que `valid_at` ou `as_of` n'est pas « maintenant », le shell doit afficher un
bandeau permanent qui énonce les dates et leur fuseau, et qui ramène au présent en une
action, au clavier comme à la souris. [W9, WCAG 2.2 2.1.1]

#### REQ-007-25 — Passé en lecture seule
Tant que `as_of` est passé, le shell doit désactiver toute écriture (modification
rapide, actions qui écrivent, soumission du brouillon) et le SDK doit refuser les
écritures des plugins ; un `valid_at` futur doit rester une lecture, l'écriture au futur
passant par la période de validité du changeset. [W9, D10]

Acceptation :
- Avec `as_of` d'hier, les formulaires sont en lecture seule et l'appel d'écriture du
  SDK échoue sans requête.

#### REQ-007-26 — Contexte suivi par les plugins
Quand le contexte temporel, la langue ou l'objet courant change, le SDK doit le notifier
aux extensions affichées, qui le suivent sans rechargement de la page. [W9, W7, D17]

### E. Écritures

#### REQ-007-27 — Modification rapide
Le shell doit permettre de créer, modifier ou clore un objet depuis sa fiche par la
route de la ressource (`POST`, `PATCH` en Merge Patch, `DELETE`), avec motif, période de
validité et `If-Match` de la version affichée. [W10, REQ-006-12 à 14]

#### REQ-007-28 — Issues d'une écriture
Quand une écriture répond, le shell doit : 201 ou 200, afficher l'objet à jour ; 202,
dire que le changeset attend une approbation et y mener ; 422, afficher chaque violation
à son champ (pointeur JSON) ou à défaut en tête du formulaire ; 412, montrer la version
courante à côté de la saisie et permettre de resoumettre sur elle ; 403, dire le droit
manquant. La saisie ne doit être perdue dans aucun de ces cas. [W10, REQ-006-21,
REQ-006-14, REQ-006-23]

Acceptation :
- Deux onglets modifient la même adresse ; le second reçoit 412, voit la valeur du
  premier, resoumet et réussit.

#### REQ-007-29 — Brouillon local
Le shell doit permettre d'accumuler des opérations (créer, modifier, clore, propriétés)
dans un brouillon gardé dans le navigateur (IndexedDB), rattaché à l'identité : il
survit au rechargement et à la reconnexion de la même identité, n'est jamais lu pour
une autre identité, n'est ouvert que dans un onglet à la fois, et ne quitte le
navigateur qu'à la soumission. Il doit respecter la taille maximale d'un changeset
annoncée par les capacités. [W10, W17, REQ-006-35]

Acceptation :
- Brouillon de 5 opérations, page rechargée : les 5 opérations sont là.
- Connexion d'une autre identité dans le même navigateur : aucun brouillon de la
  première n'est lisible.

#### REQ-007-30 — Simulation continue
Quand le brouillon change, le shell doit relancer sa simulation (004/REQ-004-9) et
afficher validation, anomalies et différentiel ; un résultat ne doit jamais être montré
pour un état du brouillon autre que celui simulé (les simulations dépassées sont
abandonnées). [W10, REQ-006-11]

Acceptation :
- Trois modifications rapprochées : une seule simulation affichée, celle du dernier
  état ; aucun résultat intermédiaire n'est affiché après lui.

#### REQ-007-31 — Soumission atomique
Le shell doit soumettre le brouillon en un seul changeset (`POST /v1/changesets`), avec
motif et clé d'idempotence ; il ne doit effacer le brouillon qu'après une réponse 201 ou
202, et le garder avec ses violations sur 422 ; un nouvel essai après une coupure doit
réutiliser la même clé. [W10, REQ-006-11, REQ-006-15]

Acceptation :
- Réponse perdue puis nouvel essai : un seul changeset journalisé.

#### REQ-007-32 — Durée du brouillon
Le shell doit effacer un brouillon à la déconnexion (REQ-007-45), à la connexion d'une
autre identité, et après 7 jours sans modification. [W10 (conséquence), W17]

#### REQ-007-33 — Passé figé et brouillon
Si le contexte passe à un `as_of` passé, alors le brouillon doit être conservé mais sa
soumission désactivée (REQ-007-25). [W9, W10]

#### REQ-007-34 — Opérations des plugins
Le SDK doit permettre à un plugin d'ajouter au brouillon des opérations de l'API, qui
sont simulées et soumises comme les autres et montrées avec le nom du plugin qui les a
ajoutées. [W10, W7]

### F. Vues de la v0.1

#### REQ-007-35 — Objets du noyau
Le shell doit offrir, pour les entités, sites, domaines, préfixes, plages et adresses :
une liste paginée par curseur et filtrable (champs du noyau et propriétés, REQ-006-9),
une fiche (champs, propriétés résolues, liens), et les écritures de E. [W1,
REQ-006-4]

#### REQ-007-36 — Arbre des préfixes
Le shell doit montrer l'arbre des préfixes d'un domaine, chargé par niveau à la
demande, avec l'occupation de chaque préfixe et le prochain libre d'une taille demandée,
qui peut être ajouté au brouillon ou créé en modification rapide. [W1, 002/REQ-002-42 à
44]

#### REQ-007-37 — Propriétés
Le shell doit offrir les déclarations de propriétés et leurs listes de valeurs, les
affectations d'un objet, les valeurs résolues avec leur explication (origine, héritage),
la suggestion et les écarts entre valeur attendue et résolue. [W1, 003/REQ-003-14, 23,
24]

#### REQ-007-38 — Historique
Le shell doit montrer l'historique d'un objet (versions, changeset et auteur de chacune)
et permettre d'ouvrir l'objet à une version, ce qui fixe le contexte temporel
correspondant. [W1, W9, 004/REQ-004-18]

#### REQ-007-39 — Anomalies
Le shell doit lister les anomalies, filtrables par domaine, règle et sévérité, et
permettre leur acquittement avec motif. [W1, 002/REQ-002-36]

#### REQ-007-40 — Changesets
Le shell doit lister les changesets (filtres par état, auteur, objet et période), en
montrer un (opérations, résultat, différentiel, approbations), afficher la file des
propositions et approbations que l'utilisateur peut valider, et offrir valider,
rejeter (avec motif), retirer et annuler ; une annulation en conflit doit lister les
conflits. [W1, REQ-006-16, 004/REQ-004-21, 35, 41 à 43]

Acceptation :
- L'auteur d'un changeset en attente ne se voit pas proposer de le valider.

#### REQ-007-41 — Administration des droits
Le shell doit offrir l'administration des rôles, des liaisons, des rattachements
d'émetteurs, des comptes de service et de leurs secrets, et de l'état des identités,
chaque modification passant par un changeset (modification rapide ou brouillon). Un
secret créé doit être montré une fois, jamais gardé par le navigateur. [W1,
005/REQ-005-11, 25, 26]

#### REQ-007-42 — Audit
Le shell doit offrir la recherche dans le journal d'audit (acteur, objet, nom
d'événement, période), par pages, son export NDJSON en téléchargement et la
pseudonymisation d'une identité, après confirmation. [W1, 012/REQ-012-15, 17,
004/REQ-004-32, 33]

#### REQ-007-43 — Suivi des imports
Le shell doit lister les imports, montrer l'état de chacun (lots appliqués et restants,
rapport, échec), le rafraîchir au rythme indiqué par `Retry-After`, et offrir reprendre
et annuler à leur auteur. [W1, REQ-006-19]

### G. Session

#### REQ-007-44 — Connexion et reconnexion
Le shell doit offrir la connexion par chaque émetteur configuré et la connexion de
secours ; quand une requête reçoit 401, il doit ouvrir la reconnexion et, après elle,
ramener à la même URL du shell, brouillon intact. L'URL de retour ne doit être qu'un
chemin du shell sur la même origine. [W17, REQ-006-24, 005/REQ-005-4, 15]

Acceptation :
- Session expirée pendant l'édition d'un brouillon : reconnexion, retour à la même
  vue, brouillon intact.
- Une URL de retour vers une autre origine est ignorée (retour à l'accueil).

#### REQ-007-45 — Déconnexion
Lors d'une déconnexion, le shell doit effacer brouillons, caches et préférences autres
que la langue et le thème, dans tous ses onglets, puis appeler la déconnexion de l'API
(005/REQ-005-8), dont la réponse doit porter `Clear-Site-Data: "storage"` ; aucune
donnée du référentiel ne doit rester dans le stockage de l'origine, y compris celui
qu'ont rempli les plugins. [W10, W17, W22, Clear-Site-Data, IndexedDB]

Acceptation :
- Deux onglets ouverts, déconnexion dans l'un : IndexedDB (bases du shell et des
  plugins), Cache Storage et `localStorage` de l'origine ne contiennent plus que la
  langue et le thème.

#### REQ-007-46 — Rien de secret dans le navigateur
Le shell ne doit garder ni jeton, ni secret, ni phrase de passe, ni code TOTP dans le
stockage du navigateur ni dans une URL. [005/REQ-005-6, ADR 0020]

### H. Internationalisation

#### REQ-007-47 — Textes par clés
Le shell et les plugins de première partie ne doivent afficher aucun texte destiné à
l'utilisateur hors de leurs catalogues ; les catalogues français et anglais doivent avoir
les mêmes clés, et un test doit le vérifier. [W11, Q-ouv-6]

Acceptation :
- Une clé ajoutée au seul catalogue français fait échouer le test.

#### REQ-007-48 — Choix de la langue
Le shell doit prendre la langue choisie par l'utilisateur (gardée dans le navigateur),
sinon la première langue du navigateur qu'il prend en charge, sinon l'anglais ; un
changement de langue doit s'appliquer sans rechargement et mettre à jour l'attribut
`lang` du document. [W11, WCAG 2.2 3.1.1]

#### REQ-007-49 — Catalogues des plugins
Le shell doit charger les catalogues déclarés par un plugin sous l'espace de noms de
celui-ci ; un plugin ne doit pas pouvoir remplacer une clé du shell ni d'un autre
plugin ; une clé absente dans la langue courante doit retomber sur l'anglais. [W11]

#### REQ-007-50 — Dates, fuseaux et nombres
Le shell doit afficher dates et nombres selon la langue (`Intl`), avec le fuseau
affiché en clair, et envoyer toute date saisie en RFC 3339 avec décalage ; adresses,
préfixes et identifiants doivent rester dans leur forme canonique, jamais localisés.
[W11, REQ-006-5, 002/REQ-002-6]

Acceptation :
- En français, `2026-03-29T01:30:00Z` s'affiche à l'heure de Paris avec son décalage ;
  une date saisie à 10 h en hiver est envoyée `…T10:00:00+01:00`.

#### REQ-007-51 — Erreurs traduites
Le shell doit traduire chaque problème de l'API par son `type` et chaque violation par
sa règle, paramètres compris, et retomber sur `title` et `detail` pour un `type`
inconnu ; l'identifiant de requête doit rester affiché et copiable. [W11, RFC 9457 §3.1,
REQ-006-20, 21]

### I. Composants et accessibilité

#### REQ-007-52 — Jetons de design
Le shell doit définir couleurs, espacements, typographie et rayons comme jetons de
design, en modes clair et sombre (préférence du système, sinon choix de l'utilisateur) ;
ces jetons doivent être hérités par les extensions rendues dans un Shadow DOM, quel que
soit leur framework, et font partie de la surface du SDK (REQ-007-17). [W12, CSS Custom
Properties 1 §defining-variables, CSS Cascade 5 §inheriting]

Acceptation :
- Un Custom Element sans Vue, en Shadow DOM, qui utilise `var(--k-color-text)`, prend la
  couleur du shell dans les deux modes.

#### REQ-007-53 — WCAG 2.2 AA
Le shell et les plugins de première partie doivent satisfaire WCAG 2.2 niveau AA, cible
compatible avec l'EN 301 549 et le RGAA en vigueur (fondés sur WCAG 2.1). Chaque
parcours E2E doit passer axe-core sans violation, sur les tags `wcag2a`, `wcag2aa`,
`wcag21a`, `wcag21aa` et `wcag22aa`, la règle `target-size` activée ; les critères de
WCAG 2.2 sans règle axe-core (2.4.11, 2.5.7, 3.2.6, 3.3.7, 3.3.8) doivent avoir un test
dédié ou une ligne de la grille de revue manuelle ; un test doit vérifier les contrastes
de chaque paire de jetons texte/fond et des éléments non textuels dans les deux modes.
Pour les plugins tiers, c'est une recommandation. [W14, WCAG 2.2 1.4.3, 1.4.11, 2.5.8]

Acceptation :
- Une violation axe-core dans un parcours le fait échouer en CI ; les résultats
  `incomplete` sont rapportés sans faire échouer.

#### REQ-007-54 — Clavier
Le shell doit être entièrement utilisable au clavier, arbre des préfixes et tables
compris, selon les motifs « Tree View » et « Grid » des APG ; un nœud ou une ligne
chargé à la demande ou virtualisé doit porter sa position (`aria-level`,
`aria-setsize`, `aria-posinset` ; `aria-rowcount`, `aria-rowindex`), et l'élément qui a
le focus doit rester rendu. Le focus doit être toujours visible et jamais masqué par le
bandeau ou un panneau. [W14, WCAG 2.2 2.1.1, 2.4.7, 2.4.11, APG]

Acceptation :
- Parcours E2E sans souris : ouvrir un préfixe dans l'arbre, ajouter une modification au
  brouillon, le soumettre.

#### REQ-007-55 — Messages d'état
Le shell doit annoncer aux technologies d'assistance, sans déplacer le focus, le
résultat d'une simulation, d'une soumission et les erreurs. [W14, WCAG 2.2 4.1.3]

### J. Budgets du navigateur

#### REQ-007-56 — Poids du shell
Le JavaScript et le CSS chargés pour le premier écran, hors plugins, doivent peser au
plus 300 Ko compressés (gzip) ; le reste est découpé par route. Un test du build doit
échouer au-delà. [W13]

#### REQ-007-57 — Premier écran
Le premier écran utile (vue d'accueil avec ses données) doit être rendu au plus 2 s
après le retour de la connexion, plugins en échec ou lents compris. [W13]

#### REQ-007-58 — Page de liste
Le rendu d'une page de 100 lignes doit prendre au plus 100 ms après la réception de la
réponse. [W13, INV-31]

#### REQ-007-59 — Arbre à 10⁵ sous-réseaux
L'arbre d'un préfixe à 10⁵ sous-réseaux doit être chargé à la demande et virtualisé, et
son défilement ne doit produire aucune tâche longue (> 50 ms). [W13, W12, INV-32]

Acceptation :
- Défilement de bout en bout d'un niveau de 10⁵ enfants : aucune entrée `longtask`.

### K. Tests et outillage

#### REQ-007-60 — Parcours de bout en bout
Les parcours E2E (Playwright, Chromium et Firefox) doivent tourner contre le vrai
kartenn-server et PostgreSQL de devenv, avec l'émetteur de développement (REQ-007-61) et
un jeu de données chargé par l'API ; ils doivent couvrir connexion, lecture datée,
modification rapide, brouillon, cycle d'approbation, administration, audit, chargement
des plugins (intégrité, incompatibilité, échec) et CSP. Composants et SDK sont testés
par Vitest avec un `fetch` simulé typé par le contrat. [W15, W17]

#### REQ-007-61 — Émetteur OIDC de développement
Le dépôt doit fournir un émetteur OIDC de développement (crate `kartenn-dev-idp`) :
découverte, JWKS, autorisation avec choix de l'utilisateur, jeton ; utilisateurs et
groupes en TOML ; processus devenv ; il ne doit écouter que la boucle locale et ne doit
figurer dans aucun livrable. [W16, W25, RFC 8252 §7.3]

Acceptation :
- `kartenn-dev-idp` n'est une dépendance ni de `kartenn-server` ni d'aucune crate
  livrée ; un test de l'arbre des dépendances le vérifie.

### L. Modules, livraison et licences

#### REQ-007-62 — Plugins de première partie
Chaque module métier compilé doit avoir son plugin sous `web/plugins/<module>/`, écrit
avec le seul SDK public, livré dans le paquet et activé seulement si le module figure
dans `modules` des capacités ; le shell ne doit dépendre d'aucun module ni d'aucun
plugin. [W6]

Acceptation :
- Une importation d'un fichier de `web/shell` depuis `web/plugins/*` fait échouer le
  lint ; le build du shell ne contient aucun code d'un plugin.

#### REQ-007-63 — Répertoire de build
Le build du web doit produire un répertoire autonome (gabarit `index.html`, fichiers à
empreinte, plugins de première partie sous `plugins/`) que kartenn-server sert tel quel
par `[web] root` ; les paquets de D35 l'embarqueront. [W2, W6, W25, D35 amendée]

#### REQ-007-64 — Plugin d'exemple
Le dépôt doit fournir un plugin d'exemple (`web/plugins/exemple/`) qui contribue aux
quatre points d'extension, suit le contexte, livre des catalogues FR et EN et ajoute une
opération au brouillon ; il sert de modèle documenté, de test du SDK et de cas E2E, et
n'est pas livré dans les paquets. [W1, W7]

#### REQ-007-65 — Licences
`web/sdk` et `web/plugins/*` doivent être sous MPL-2.0 et `web/shell` sous AGPL-3.0,
avec un en-tête de licence par fichier source ; un test doit vérifier les en-têtes et
que les dépendances livrées dans le shell et le SDK ont des licences compatibles. [W17,
Q-ouv-1, D37]

### M. Ajouts à l'API

#### REQ-007-66 — Configuration `[web]`
kartenn doit accepter la section `[web]` : `root` (répertoire du build) et
`plugins_dir` (facultatif) ; un répertoire illisible doit refuser le démarrage, comme
les autres réglages (REQ-012-31). [W2, W3, W17]

#### REQ-007-67 — Ajouts additifs
kartenn doit faire, sans rien retirer ni changer de `/v1` (REQ-006-33), les ajouts dont
le shell a besoin : les champs de `ui_plugins` (REQ-007-9) ; dans `GET /v1/me`,
`permissions`, les entrées des rôles tenus sur au moins un périmètre ;
`GET /v1/auth/issuers`, sans authentification, l'identifiant et le libellé de chaque
émetteur (`label` facultatif de `[[auth.issuers]]`, défaut : `id`) ;
`Clear-Site-Data: "storage"` sur la réponse de la déconnexion, et sa variante JSON
(`Accept: application/json` → 200 et `end_session_url`). Le contrat de référence doit
être mis à jour avec eux. [W17, W18, W22, W28, H12, REQ-006-34]

## Non-objectifs

- Isolation des plugins non fiables (iframe, permissions déclarées) : une extension non
  fiable passe par l'API comme un connecteur (W4).
- Plugins téléversés par l'API ou chargés depuis une autre origine (W3).
- Brouillon partagé côté serveur (W10) ; mode hors ligne, service worker.
- Dépôt d'un import par l'interface : la v0.1 en assure le suivi (W1).
- Écrans du niveau de log à chaud et du contrat OpenAPI (CLI et `curl`).
- Vue graphique à 10⁴ nœuds (D14) : plugin de visualisation d'une spec ultérieure.
- Écrans VLAN et Machines : plugins de la 008 et de la 009.
- Canal temps réel (WebSocket, SSE) : les listes se relisent à la navigation ou à la
  demande.
- Télémétrie du navigateur vers le serveur, point de rapport des violations de CSP
  (`report-to`) : les violations sont relevées par les parcours E2E
  (`securitypolicyviolation`).
- Trusted Types imposés (`require-trusted-types-for 'script'`) : au plus en
  report-only tant que les plugins tiers n'y sont pas conformes.
- Registres de Custom Elements à portée : absents de Firefox ESR en 2026-10.
- Paquets OCI et module NixOS (D35) : spec à venir ; la 007 livre le répertoire de
  build qu'ils embarqueront (W25).
- Langues autres que le français et l'anglais ; documentation de l'interface en anglais
  (Q-ouv-6, reste ouverte pour la documentation).

## Questions ouvertes

Toutes tranchées le 2026-10-19, par délégation de l'utilisateur, selon la recommandation
([`decisions.md`](decisions.md), W18 à W27).

- [x] Ajouts à l'API hors W17 (REQ-007-67). (utilisateur, délégué) — W18.
- [x] CSP des styles (REQ-007-3). (utilisateur, délégué) — W19 : `style-src 'self'`
  strict.
- [x] Production de l'import map (REQ-007-11, 15). (Claude, design) — W20 :
  `index.html` rendu par kartenn.
- [x] Effacement à la déconnexion (REQ-007-45). (Claude, design) — W22.
- [x] Mesure des budgets hors Chromium ; WebKit dans les E2E. (Claude, design) — W23.
- [x] Outil de comparaison de la surface publique du SDK (REQ-007-17). (Claude, design ;
  utilisateur, délégué) — W24 : api-extractor.
- [x] Choix tranchés par Claude en écrivant les exigences. (utilisateur, délégué) — W27.
- [ ] Amendement de D35 (image « web statique » remplacée par le build servi par
  kartenn-server, W25). (utilisateur, à la relecture)
