# Langues : étiquettes, négociation et cache d'une réponse par langue

Base du dictionnaire de libellés (spec 018, R9, R21, R22 ; ADR 0040). Complète le point
« §12.5.5 Vary : non relu » de [`api-http.md`](api-http.md).

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 5646](https://www.rfc-editor.org/rfc/rfc5646) — Tags for Identifying Languages (BCP 47) | Best Current Practice | 2026-10-23 (§2.1, 2.1.1, 4.4.1, 4.4.2 en partie) |
| [RFC 4647](https://www.rfc-editor.org/rfc/rfc4647) — Matching of Language Tags (BCP 47) | Best Current Practice | 2026-10-23 (§2.1, 3.4, 3.4.1) |
| [RFC 9110](https://www.rfc-editor.org/rfc/rfc9110) — HTTP Semantics | Internet Standard (STD 97) | 2026-10-23 (§8.5, 8.8.3, 12.4.2, 12.5.4, 12.5.5, 13.1.2) |
| [RFC 9111](https://www.rfc-editor.org/rfc/rfc9111) — HTTP Caching | Internet Standard (STD 98) | 2026-10-23 (§4.1, 5.2.2.1, 5.2.2.4, 5.2.2.7) |

## Ce que disent les sources

### Étiquettes (RFC 5646)
- §2.1, ABNF : `langtag = language ["-" script] ["-" region] *("-" variant)
  *("-" extension) ["-" privateuse]` ; `language = 2*3ALPHA ["-" extlang] / 4ALPHA /
  5*8ALPHA` ; `script = 4ALPHA` ; `region = 2ALPHA / 3DIGIT` ; `variant = 5*8alphanum /
  (DIGIT 3alphanum)` ; `extension = singleton 1*("-" (2*8alphanum))` ; `privateuse = "x"
  1*("-" (1*8alphanum))`. La production complète admet aussi les étiquettes
  `grandfathered`.
- §2.1 : « All subtags have a maximum length of eight characters. » La grammaire ne fixe
  pas de longueur totale.
- §2.1.1 (normatif) : « At all times, language tags and their subtags, including private
  use and extensions, are to be treated as case insensitive » ; les conventions de casse
  « MUST NOT be taken to carry meaning ». L'équivalence est celle de l'ASCII.
- §4.4.1 : « Implementations SHOULD NOT truncate language tags unless the meaning of the
  tag is purposefully being changed » (tampons de taille limitée).

### Appariement (RFC 4647)
- §2.1 : une plage de base a la syntaxe d'une étiquette, ou est le seul caractère `*`.
- §3.4, schéma *Lookup* : « the language range is progressively truncated from the end
  until a matching language tag is located » ; un singleton découvert en fin de plage
  part avec la sous-étiquette qui le précède ; « If no language tag matches the request,
  the "default" value is returned. » Chaque plage de la liste est essayée avant le
  défaut (« MUST process each language range in the list before seeking or calculating
  the default »).
- §3.4.1 : l'application « MUST define the defaulting behavior when no tag matches the
  language priority list ».

### HTTP (RFC 9110)
- §8.5 `Content-Language` : décrit le public visé de la représentation (« the natural
  language(s) of the intended audience »), pas toutes les langues qu'elle contient.
- §12.4.2 : poids de 0 à 1, 1 par défaut ; `q=0` signifie « not acceptable ».
- §12.5.4 `Accept-Language` : l'appariement renvoie aux schémas de la RFC 4647 ; un agent
  qui ne laisse pas l'utilisateur choisir sa langue « MUST NOT send » ce champ ; le champ
  révèle des préférences (vie privée).
- §12.5.5 `Vary` : l'origine « SHOULD generate a Vary header field on a cacheable
  response » dont la sélection dépend de champs de la requête.
- §8.8.3 : un ETag est fort sauf préfixe `W/`. §13.1.2 : `If-None-Match` se compare par
  la fonction faible ; si une étiquette correspond, l'origine répond 304 à un `GET`.

### Cache (RFC 9111)
- §4.1 : une réponse stockée dont les champs nommés par `Vary` ne correspondent pas
  « MUST NOT » servir sans revalidation.
- §5.2.2.1 `max-age` : réponse fraîche jusqu'à cet âge, en secondes.
- §5.2.2.4 `no-cache` : la réponse « MUST NOT be used to satisfy any other request
  without forwarding it for validation » ; elle peut être stockée.
- §5.2.2.7 `private` : un cache partagé « MUST NOT store the response » ; un cache privé
  le peut.

## Pièges

- *Lookup* rend une seule langue, *Filtering* toutes celles qui correspondent : une
  réponse servie dans une langue relève de Lookup.
- Troncature : `zh-Hant-TW` → `zh-Hant` → `zh` ; `de-CH-1996-x-a` : le singleton `x`
  part avec ce qui le suit avant d'aller plus loin.
- Les plages se trient par poids décroissant ; `q=0` retire la plage (ce n'est pas la
  plus faible priorité) ; `*` seul n'exprime aucune préférence.
- Comparer sans casse ASCII (`fr-FR` = `fr-fr`), et normaliser avant de calculer un ETag
  ou une clé de cache.
- Syntaxe valide ≠ sous-étiquette enregistrée : la grammaire accepte `qaa-Latn-ZZ`.
- `private` ne dispense pas de `Vary` (un navigateur indexe sa copie par les champs
  listés) ; un paramètre d'URL (`?lang=`) n'a pas besoin de `Vary`, il fait partie de la
  clé de cache.
- `no-cache` n'interdit pas de stocker (c'est `no-store`).
- Même URL, contenus différents selon l'appelant : `private` est requis (voir
  [`api-http.md`](api-http.md), cache et RBAC).

## Implications pour kartenn (choix propres, pas des exigences des RFC)

1. `?lang=` (une étiquette, sans liste ni poids) l'emporte sur `Accept-Language`.
2. Lookup parmi les langues disponibles, plages triées par poids (`q=0` écartées), puis
   la langue par défaut de l'instance (`[i18n] default_locale`) : c'est le comportement
   par défaut que demande la RFC 4647 §3.4.1. Jamais de 406 : une langue est toujours
   servie.
3. `Content-Language` = la langue servie (une étiquette), jamais celle demandée.
4. `Vary: Accept-Language` sur `/v1/labels` seulement : aucune autre route ne varie
   selon la langue.
5. ETag fort `"<révision>.<empreinte>"` (forme de 006/H19), l'empreinte couvrant la
   langue servie, les espaces rendus et le filtrage par droits, pour qu'un 304 ne révèle
   rien d'un espace non lu.
6. `Cache-Control: private, max-age=86400` à `as_of` passé (contenu figé, sauf version du
   serveur), `private, no-cache` au présent (revalidation par 304).
7. `lang` hors grammaire de la §2.1 : 400 `bad-request`. Une plage d'`Accept-Language`
   mal formée est ignorée (le champ vient souvent de l'agent, pas de l'appelant).
   Les étiquettes `grandfathered` ne sont pas admises ; la casse est normalisée en
   minuscules.

## Non trouvé ou non relu

- RFC 9110 §12.5.4 : le comportement de l'origine quand rien ne correspond n'a pas été
  relevé mot à mot (l'ancienne RFC 7231 disait qu'elle « MAY » ignorer le champ).
- RFC 9110 §12.4.2 : limite de trois décimales et ABNF d'`Accept-Language` non relues.
- RFC 5646 §4.4.1 : la longueur minimale qu'une implémentation doit accepter (35
  caractères) est à revérifier mot à mot.
- RFC 4647 §3.4 : exemples complets non reproduits.
- Errata des quatre RFC non consultés.
