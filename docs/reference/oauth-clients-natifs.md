# Clients OAuth natifs publics : bouclage, PKCE, flux d'appareil, rotation

Note de lecture pour la spec [023-client-python-cli](../../specs/023-client-python-cli/).

Complète [authentification-autorisation.md](authentification-autorisation.md) (côté
serveur, spec 005) sans la répéter. Ce qui n'a pas été relu à la source est marqué
**à vérifier**.

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 8252](https://www.rfc-editor.org/rfc/rfc8252) OAuth 2.0 for Native Apps | BCP 212, met à jour la RFC 6749 ; pas de « Updated by » ni « Obsoleted by » vu ; errata **à vérifier** | 2026-10-23 |
| [RFC 7636](https://www.rfc-editor.org/rfc/rfc7636) PKCE | Proposed Standard (2015) ; errata **à vérifier** | 2026-10-23 |
| [RFC 8628](https://www.rfc-editor.org/rfc/rfc8628) Device Authorization Grant | Proposed Standard (août 2019) ; pas de « Updated by » vu ; errata **à vérifier** | 2026-10-23 |
| [RFC 9700](https://www.rfc-editor.org/rfc/rfc9700) Security BCP | BCP 240, met à jour les RFC 6749, 6750 et 6819 | 2026-10-23 |
| [RFC 9068](https://www.rfc-editor.org/rfc/rfc9068) JWT Profile for Access Tokens | Proposed Standard | 2026-10-23 |
| [RFC 8707](https://www.rfc-editor.org/rfc/rfc8707) Resource Indicators | Proposed Standard | 2026-10-23 |
| [RFC 8414](https://www.rfc-editor.org/rfc/rfc8414) AS Metadata | Proposed Standard (juin 2018) | 2026-10-23 |
| [OIDC Core 1.0](https://openid.net/specs/openid-connect-core-1_0.html), [OIDC Discovery 1.0](https://openid.net/specs/openid-connect-discovery-1_0.html) | Spécifications finales OpenID ; errata set **à vérifier** | 2026-10-23 |

## Ce que disent les sources

Les citations sont des fragments recoupés mot pour mot dans la page du rfc-editor.org
ou d'openid.net.

**RFC 8252**
- §4 : le client natif fait la demande « in an external user-agent (typically the
  browser) ». §8.12 (*Embedded User-Agents*) : « native apps MUST NOT use embedded
  user-agents to perform authorization requests ».
- §6 : « Public native app clients MUST implement the Proof Key for Code Exchange (PKCE
  [RFC7636]) extension to OAuth, » et « authorization servers MUST support PKCE for such
  clients ».
- §7.3 : « The authorization server MUST allow any port to be specified at the time of
  the request for loopback IP redirect URIs, » pour les clients qui prennent un port
  éphémère. Les URI de bouclage utilisent le schéma `http` et le littéral IP
  (`http://127.0.0.1:{port}/{path}`, `http://[::1]:{port}/{path}`). « Clients SHOULD NOT
  assume that the device supports a particular version of the Internet Protocol. »
- §8.3 : « the use of localhost is NOT RECOMMENDED ».
- §8.1 : « Authorization servers SHOULD reject authorization requests from native apps
  that don't use PKCE ».
- §8.2 : l'implicit flow est « NOT RECOMMENDED » pour un client natif.
- §8.4 : le serveur « MUST require clients to register their complete redirect URI »,
  mais §7.3 impose d'accepter n'importe quel port pour le bouclage.
- §8.5 : « it is NOT RECOMMENDED for authorization servers to require client
  authentication of public native apps clients using a shared secret ». Un secret
  embarqué dans une appli distribuée n'est pas confidentiel.
- §8.9 : « it is similarly RECOMMENDED that native apps include a high-entropy secure
  random number in the "state" parameter ».

**RFC 7636**
- §4.1 : `code_verifier` de 43 à 128 caractères parmi
  `[A-Z] / [a-z] / [0-9] / "-" / "." / "_" / "~"`. « It is RECOMMENDED that the output
  of a suitable random number generator be used to create a 32-octet sequence. » Encodé
  en base64url, il donne 43 caractères.
- §4.2 : `S256` = `BASE64URL-ENCODE(SHA256(ASCII(code_verifier)))`. « If the client is
  capable of using "S256", it MUST use "S256" ».
- §4.3 : `code_challenge_method` vaut `plain` par défaut s'il est absent. Il faut donc
  l'envoyer.
- §7.2 : « "plain" SHOULD NOT be used ».

**RFC 8628**
- §3.1 : POST sur le `device_authorization_endpoint` ; `client_id` requis pour un client
  qui ne s'authentifie pas.
- §3.2 : `device_code`, `user_code`, `verification_uri`, `expires_in` REQUIRED ;
  `verification_uri_complete` et `interval` OPTIONAL. `interval` : « If no value is
  provided, clients MUST use 5 as the default. »
- §3.3.1 : le client peut présenter `verification_uri_complete` de façon non textuelle,
  mais « Clients MUST still display the "user_code" » (**à vérifier**, citation
  reconstituée de deux fragments).
- §3.4 : `grant_type` = `urn:ietf:params:oauth:grant-type:device_code`, plus
  `device_code` et `client_id`.
- §3.5 :
  - `authorization_pending` : le client SHOULD répéter la requête ; il MUST attendre au
    moins `interval` secondes entre deux requêtes.
  - `slow_down` : « the interval MUST be increased by 5 seconds for this and all
    subsequent requests ».
  - `access_denied` : « The authorization request was denied. »
  - `expired_token` : le client MAY relancer une demande, mais SHOULD attendre une action
    de l'utilisateur avant de redémarrer.
- §4 : métadonnée optionnelle `device_authorization_endpoint`, avec
  `grant_types_supported` contenant le type `device_code`.
- §5, titres : 5.1 User Code Brute Forcing ; 5.2 Device Code Brute Forcing (« a very high
  entropy code SHOULD be used ») ; 5.3 Device Trustworthiness ; 5.4 Remote Phishing ;
  5.5 Session Spying ; 5.6 Non-Confidential Clients ; 5.7 Non-Visual Code Transmission.
- §5.4 : « it is RECOMMENDED to inform the user that they are authorizing a device during
  the user-interaction step ». C'est l'avertissement visé par le design de la 023.
- §5.6 : un client d'appareil est traité comme public.

**RFC 9700**
- §2.1.1 : « Public clients MUST use PKCE [RFC7636] to this end » ; « the PKCE challenge
  or OpenID Connect nonce MUST be transaction-specific and securely bound to the
  client ». Les clients SHOULD choisir une méthode qui n'expose pas le verifier ; S256
  est la seule.
- §2.2.2 : « Refresh tokens for public clients MUST be sender-constrained or use refresh
  token rotation as described in Section 4.14. »
- §4.14 (*Refresh Token Protection*, 4.14.2 *Recommendations*) : avec la rotation, le
  serveur émet un nouveau jeton à chaque rafraîchissement et « The previous refresh
  token is invalidated. » En cas de rejeu, le serveur ne sait pas lequel des deux
  porteurs a présenté le jeton invalidé : il révoque le jeton actif, et le client
  légitime doit obtenir un nouveau grant (paraphrase, **à vérifier** mot pour mot).

**OpenID Connect Core**
- §3.1.2.1 : « Sufficient entropy MUST be present in the nonce values used to prevent
  attackers from guessing values. »
- §3.1.3.7, 11 : « a nonce Claim MUST be present and its value checked to verify that it
  is the same value ». Même section : `iss` identique à l'émetteur, `aud` contenant le
  `client_id`.
- §2 : l'ID Token porte des claims sur l'authentification. Il n'est pas un jeton d'accès
  (déduction ; aucune phrase « MUST NOT » trouvée).
- §15.5.2 : « The nonce parameter value needs to include per-session state and be
  unguessable to attackers. »
- §11 : `offline_access` demande un refresh token ; « The OP MUST always obtain
  consent ».

**Découverte**
- OIDC Discovery §4 : document à `<issuer>/.well-known/openid-configuration` ; §4.3 :
  `issuer` retourné « MUST be identical to the Issuer URL » utilisée pour la requête, et
  au `iss` des ID Tokens.
- RFC 8414 §3 : chemin par défaut `/.well-known/oauth-authorization-server` ; §3.3 :
  `issuer` identique à l'identifiant utilisé. §2 : sans
  `code_challenge_methods_supported`, le serveur est réputé ne pas supporter PKCE.
- La présence de `device_authorization_endpoint` dans le document OIDC relève de la
  RFC 8628 §4 (**à vérifier** : OIDC Discovery ne la liste pas).

**RFC 9068 et audience**
- §2.1 : « JWT access tokens MUST be signed » ; « MUST NOT use "none" » ; « the "typ"
  value used SHOULD be "at+jwt" ».
- §2.2 : `iss`, `exp`, `aud`, `client_id`, `jti` REQUIRED.
- §4 : « The resource server MUST verify that the "typ" header value is "at+jwt" or
  "application/at+jwt" » ; `aud` doit contenir l'identifiant de la ressource.
- §3 : `aud` SHOULD égaler le paramètre `resource` de la demande.
- §6 : « The client MUST NOT inspect the content of the access token ».
- RFC 8707 §2 : `resource` est un URI absolu sans fragment ; plusieurs occurrences MAY
  être envoyées ; erreur `invalid_target`.
- Un paramètre `audience` n'est défini dans aucune de ces sources : c'est une extension
  propre à certains émetteurs (**à vérifier** par émetteur).

## Pièges

- `localhost` : NOT RECOMMENDED (§8.3). Il peut se résoudre en `::1` ou être détourné.
  Utiliser le littéral `127.0.0.1`.
- IPv6 : le littéral s'écrit `[::1]`. L'URI de redirection de la demande et celle du
  `POST` de jeton doivent être identiques caractère pour caractère, port compris.
  Beaucoup d'émetteurs n'acceptent pas un port libre pour `[::1]` : **à vérifier** par
  émetteur.
- Port éphémère : le MUST de §7.3 pèse sur le serveur. Un émetteur qui compare le port
  viole la RFC ; il faut alors un port fixe, ce qui est un choix de profil.
- Rôles de `state` et de PKCE : `state` lie la réponse à la demande (CSRF, §8.9) ; PKCE
  lie le code à l'instance qui l'a demandé (injection de code, 9700 §2.1.1). L'un ne
  remplace pas l'autre. `nonce` lie l'ID Token à la demande.
- Navigateur : l'écoute de bouclage est lisible par tout processus local ; PKCE est la
  parade. Fermer l'écouteur après le premier appel valide.
- `slow_down` est cumulatif : +5 s pour la requête en cours et toutes les suivantes,
  sans retour à `interval`. Mémoriser l'intervalle courant.
- Sondage d'appareil : `authorization_pending` ne doit pas déclencher de nouvelle
  `device_authorization` ; `expired_token` termine la session. Respecter `expires_in`
  côté client aussi.
- `verification_uri_complete` : pratique mais favorise l'hameçonnage à distance (§5.4).
  Le code reste affiché (§3.3.1) et l'utilisateur doit vérifier qu'il correspond.
- Rotation de refresh token et invocations concurrentes : deux processus de la CLI qui
  rafraîchissent avec le même jeton produisent un rejeu, et l'émetteur peut révoquer
  toute la lignée (§4.14.2). Il faut un verrou inter-processus autour de « lire,
  rafraîchir, écrire », avec relecture du magasin après l'acquisition du verrou.
- Jeton d'identité : ne jamais l'envoyer à l'API (audience du client, pas de kartenn).
  Le jeton d'accès est opaque pour le client (RFC 9068 §6) : ne pas en lire les claims
  pour décider.
- Découverte : comparer `issuer` à l'URL de configuration avant de faire confiance aux
  points de terminaison.
- Secret statique : un `client_secret` embarqué dans la bibliothèque n'est pas un secret
  (§8.5) ; client public, `token_endpoint_auth_method` = `none`.

## Implications pour kartenn

Propositions ; elles ne décident rien. Liens : T10, REQ-023-6, 7, 8, 40, D13.

- **T10 / REQ-023-7** : vérifier avant tout flux que le document de découverte annonce
  `S256` dans `code_challenge_methods_supported` ; sinon refuser plutôt que de retomber
  sur `plain` (RFC 7636 §7.2, 9700 §2.1.1).
- Toujours envoyer `code_challenge_method=S256` (défaut `plain`, §4.3), `state` et
  `nonce` d'au moins 32 octets aléatoires ; contrôler `state` avant l'échange du code.
- Redirection `http://127.0.0.1:<port>/callback`, `[::1]` en repli ; le `redirect_uri`
  envoyé à `/token` est identique à celui de la demande.
- Audience par `resource` (RFC 8707) quand l'émetteur le supporte ; un paramètre
  `audience` n'est qu'une extension d'émetteur. Le contrôle `typ`/`aud` reste côté
  serveur (005).
- **Flux d'appareil** : afficher le code même avec `verification_uri_complete`, avec
  l'avertissement de §5.4 ; `interval` par défaut 5 s ; `slow_down` cumulatif ; arrêt sur
  `access_denied` et `expired_token`.
- **REQ-023-40** : verrou de fichier inter-processus autour du rafraîchissement ; exiger
  de l'émetteur la rotation (9700 §2.2.2) et traiter `invalid_grant` comme fin de session
  (effacer les jetons, demander `kartenn login`).
- **REQ-023-6 / D13** : les comptes de service n'utilisent pas ces flux ; ne pas
  confondre le secret d'un compte de service avec le `client_id` public de la CLI (§8.5).
- Écart relevé : la 023 (T10) cite RFC 8252 §8.12 pour le navigateur externe ; §8.12 est
  *Embedded User-Agents* (interdiction du webview), l'exigence d'agent externe est en
  §4. Les renvois RFC 8628 §5.2 et §5.3 et RFC 9700 §4.14 sont confirmés.

## Non trouvé, à vérifier

- Errata des RFC 7636, 8252, 8628, 9700 (non consultés).
- Texte exact de 9700 §4.14.2 ; OIDC §3.1.2.1 sur la validation du nonce par le client
  public.
- Aucune source primaire sur la concurrence de plusieurs invocations d'un client
  public : le verrou est une proposition de kartenn.
