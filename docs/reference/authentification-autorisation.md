# Authentification et autorisation

Note de lecture pour la spec [005-authn-rbac](../../specs/005-authn-rbac/). Recherche
documentaire du 2026-10-17 ; ce qui n'a pas pu être relu à la source est marqué **non
vérifié**. Les exigences normatives (MUST, SHOULD au sens des RFC 2119 et 8174) sont
distinguées des choix propres à kartenn, regroupés à la fin.

## Sources

| Document | Statut | Consulté |
|---|---|---|
| [RFC 9700](https://www.rfc-editor.org/rfc/rfc9700.html) — Best Current Practice for OAuth 2.0 Security | BCP (2025) | 2026-10-17 (§4.14 **non vérifié**) |
| [RFC 10017](https://www.rfc-editor.org/info/rfc10017) — OAuth 2.0 for Browser-Based Applications (ex-draft-ietf-oauth-browser-based-apps) | BCP (2026 ; le numéro de BCP est incohérent sur la page) | 2026-10-17 |
| [RFC 9207](https://www.rfc-editor.org/rfc/rfc9207.html) — Authorization Server Issuer Identification | Proposed Standard (2022) | 2026-10-17 |
| [RFC 8725](https://www.rfc-editor.org/rfc/rfc8725.html) — JSON Web Token Best Current Practices | BCP (2020) | 2026-10-17 |
| [RFC 9068](https://www.rfc-editor.org/rfc/rfc9068.html) — JWT Profile for OAuth 2.0 Access Tokens | Proposed Standard (2021) | 2026-10-17 |
| [OpenID Connect Core 1.0](https://openid.net/specs/openid-connect-core-1_0.html) | spécification finale (errata 2) | 2026-10-17 |
| [OpenID Connect RP-Initiated Logout 1.0](https://openid.net/specs/openid-connect-rpinitiated-1_0.html) | spécification finale | 2026-10-17 |
| [RFC 7636](https://www.rfc-editor.org/rfc/rfc7636.html) (PKCE), [RFC 7617](https://www.rfc-editor.org/rfc/rfc7617.html) (Basic) | Proposed Standard | cités, **non relus** cette fois |
| [RFC 9106](https://www.rfc-editor.org/rfc/rfc9106.html) — Argon2 | Informational (2021) | 2026-10-17 |
| [RFC 6238](https://www.rfc-editor.org/rfc/rfc6238.html) — TOTP | Informational (2011) | 2026-10-17 |
| [NIST SP 800-63B-4](https://pages.nist.gov/800-63-4/sp800-63b.html) — Authentication and Authenticator Management | final (2025) | 2026-10-17 |

## Ce que disent les sources

### Flux OAuth et OIDC du serveur (RFC 9700, RFC 9207)

- Flux par code : utiliser `response_type=code` (SHOULD, §2.1.2) ; pas de flux implicite
  (SHOULD NOT).
- PKCE : RECOMMENDED pour un client confidentiel (§2.1.1), qui protège aussi contre la
  CSRF ; un serveur d'autorisation doit refuser un `code_verifier` sans `code_challenge`
  préalable (MUST, §2.1.1, §4.8.2).
- URI de redirection : comparaison **exacte** (MUST, §2.1, §4.1.3).
- CSRF : obligatoire (MUST, §2.1) ; PKCE ou `nonce` OIDC suffisent si le support PKCE du
  serveur est confirmé (§4.7.1), sinon `state` à usage unique lié au navigateur ; un
  `state` qui porte des données doit être protégé contre la falsification (MUST, §4.7.1).
- **Plusieurs émetteurs** : défense contre la confusion d'émetteur requise (§2.1) ;
  mémoriser l'émetteur visé par requête, lié au navigateur, et l'égaler à l'émetteur reçu
  (MUST, §4.4.2) ; paramètre `iss` de la réponse d'autorisation (RFC 9207 §2.4 : rejet en
  cas d'écart, MUST ; métadonnée `authorization_response_iss_parameter_supported`, §3).
- Jetons d'accès : restreints à un serveur de ressources (SHOULD, §2.3) ; le serveur de
  ressources vérifie à chaque requête que le jeton lui est destiné (MUST, §2.3).
- Jetons de rafraîchissement d'un client confidentiel : utilisables par ce seul client
  (RFC 6749, rappelé §2.2.2).

### Applications navigateur : le BFF (RFC 10017)

- Le BFF est un client confidentiel côté serveur qui garde les jetons dans une session à
  cookie et relaie les appels ; le navigateur ne détient aucun jeton (§6.1). Il neutralise
  le vol de jetons par script (§5.1.1, §5.1.2) et l'obtention silencieuse de nouveaux
  jetons (§5.1.3) ; reste le détournement du client dans le navigateur (§5.1.4), propre à
  toute application Web. Fortement recommandé pour les applications métier et sensibles.
- Cookie (§6.1.3.2) : `Secure` et `HttpOnly` (MUST) ; `SameSite=Strict`, `Path=/`, pas
  de `Domain`, préfixe `__Host-` (SHOULD).
- CSRF (§6.1.3.3) : protection obligatoire (MUST) : `SameSite` seulement si le BFF n'est
  pas « same-site » avec d'autres applications ; sinon en-tête personnalisé imposant une
  requête préalable CORS (SHOULD), ou jeton anti-falsification.

### Validation des jetons (OIDC Core, RFC 9068, RFC 8725)

- Jeton d'identité (OIDC Core §3.1.3.7) : `iss` égal à l'émetteur (MUST) ; `aud` contient
  le `client_id` et aucune audience non fiable (MUST) ; signature selon `alg` avec les
  clés de l'émetteur (MUST ; MAY se fier à TLS s'il vient directement du point de jeton) ;
  `exp` (MUST) ; `nonce` présent et égal s'il a été envoyé (MUST), rejeu à contrôler
  (SHOULD) ; `auth_time` si `max_age` est utilisé (SHOULD).
- Jeton d'accès JWT (RFC 9068 §4) : `typ` = `at+jwt` (MUST) ; `iss` exact (MUST) ; `aud`
  contient l'identifiant du serveur de ressources (MUST) ; signature, `alg` jamais `none`,
  clés de l'émetteur (MUST) ; `exp` (MUST, petite tolérance d'horloge permise) ; erreur
  `invalid_token` (RFC 6750 §3.1). Claims requis (§2.2) : `iss`, `exp`, `aud`, `sub`,
  `client_id`, `iat`, `jti`.
- RFC 8725 : liste fermée d'algorithmes, une clé pour un algorithme (§3.1) ; algorithmes
  actuels, pas de `none` (§3.2) ; toute opération invalide rejette le jeton (§3.3) ; clés
  appartenant à l'émetteur (§3.8) ; audience obligatoire quand un émetteur sert plusieurs
  parties (§3.9) ; typage explicite recommandé (§3.11) ; règles de validation mutuellement
  exclusives entre types de jetons d'un même émetteur (MUST, §3.12).
- Identifiant d'une personne (OIDC Core §5.7) : seul le couple (`iss`, `sub`) est garanti
  unique ; `email`, `phone_number`, `preferred_username` et `name` MUST NOT servir
  d'identifiant.

### Déconnexion (RP-Initiated Logout 1.0)

- `end_session_endpoint` dans les métadonnées (§2.1) ; `id_token_hint` RECOMMENDED,
  `post_logout_redirect_uri` optionnel mais préenregistré et comparé exactement (§3).

### Mots de passe, second facteur, sessions (NIST SP 800-63B-4, RFC 9106, RFC 6238)

- Mot de passe : 15 caractères au moins en facteur unique, 8 dans une authentification
  multifacteur (§3.1.1.2) ; pas de règles de composition ni de changement périodique
  (SHALL NOT, §3.1.1.2).
- Limitation : au plus 100 échecs consécutifs sur un authentificateur, puis
  désactivation (§3.2.2).
- OTP : secret d'au moins 112 bits ; un OTP accepté une seule fois (§3.1.4, §3.1.5).
- TOTP (RFC 6238) : pas de 30 s (§4.1, §5.2) ; au plus un pas de retard toléré
  (recommandé, §5.2) ; un OTP déjà validé ne doit pas être réaccepté (MUST, §5.2) ; clé de
  la taille de la sortie HMAC (SHOULD, §5.1).
- Argon2id (RFC 9106 §4) : option 1 : 2 Gio, t = 1, p = 4 ; option 2 (mémoire
  contrainte) : 64 Mio, t = 3, p = 4 ; sel 128 bits, sortie 256 bits.
- Réauthentification AAL2 : inactivité au plus 1 h, durée totale au plus 24 h (SHOULD,
  §2.2, numérotation **non vérifiée**).

## Choix de kartenn (spec 005)

- BFF à cookie chiffré sans état, `__Host-`, `Secure`, `HttpOnly`, `SameSite=Strict`,
  protection CSRF avec la 006 (K1) ; inactivité 30 min et durée absolue 8 h, plus strict
  que les plafonds AAL2 (K10).
- Identité = (`iss`, `sub`), jamais le courriel (K2, OIDC Core §5.7).
- Plusieurs émetteurs : émetteur mémorisé par requête et comparé (`iss` de RFC 9207
  quand l'émetteur le publie) ; URI de redirection exacte ; PKCE même en client
  confidentiel, `state` et `nonce`.
- Jetons d'accès des clients non navigateur : profil RFC 9068 exigé (`typ` `at+jwt`),
  audience de kartenn, liste fermée d'algorithmes par émetteur.
- Comptes de service : secrets générés (256 bits), HMAC-SHA-256 (K8) ; comptes de secours :
  Argon2id option 2, 15 caractères, TOTP (K9).
