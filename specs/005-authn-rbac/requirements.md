---
status: implemented
updated: 2026-10-18
---

# 005 — Authentification et RBAC : exigences

## Résumé

kartenn décrit le réseau d'une organisation et de ses prestataires : savoir **qui** agit
et **ce qu'il a le droit de voir et de faire, où**, est la condition de tout le reste.
Cette spec fixe l'identité des personnes (plusieurs fournisseurs OIDC), des comptes de
service et des comptes de secours ; la vérification de leurs preuves ; et le contrôle
d'accès par rôles, par périmètres hérités et par confinement des fournisseurs
d'identité. Tout refus est par défaut, toute décision est journalisée (012), toute
modification des droits passe par un changeset (004). Les routes HTTP, cookies et
formats d'erreur sont la spec 006, qui appelle ce que la 005 fournit.

Décisions : [`decisions.md`](decisions.md) (K1–K15) ; D9, D11, D13 (amendé par K8), D14,
D36 ([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) ; ADR 0009, 0010,
0014 ; 003/P6, P7, P17 ; 004 (politiques d'approbation, propositions) ; 012 (événements
d'audit). Sources :
[`docs/reference/authentification-autorisation.md`](../../docs/reference/authentification-autorisation.md).

### Vocabulaire

- **Émetteur** : fournisseur d'identité OIDC déclaré dans la configuration (identifiant
  `iss`).
- **Identité** : sujet authentifié : personne (émetteur, `sub`), compte de service ou
  compte de secours.
- **Groupe** : valeur du claim de groupes d'un émetteur, lue dans le jeton, jamais
  stockée.
- **Permission** : droit élémentaire `<module>.<objet>.<action>` ou `prop.<clé>.<action>`.
- **Rôle** : ensemble nommé de permissions.
- **Périmètre** : global, entité, site, domaine ou sous-arbre de préfixes ; hérité vers
  le bas.
- **Liaison** : sujet × rôle × périmètre, valable sur une période applicative ; le sujet
  est une identité, un groupe d'un émetteur ou un émetteur entier.
- **Confinement** : borne des droits obtenus par un émetteur : les objets dont l'entité
  propriétaire est dans les sous-arbres rattachés à cet émetteur, ou tout (rattachement
  global).
- **Session** : cookie chiffré sans état délivré par le BFF.

### Conventions

- Forme EARS en français, comme dans
  [`002/requirements.md`](../002-noyau-adressage/requirements.md#conventions).
- Les références entre crochets renvoient aux décisions (K*) et aux sources de la note de
  référence : [RFC 9700 §x], [RFC 10017 §x], [OIDC §x], [800-63B §x].

## Exigences

### A. Identités

#### REQ-005-1 — Clé d'une personne
kartenn doit identifier une personne par le couple (émetteur, `sub`) et ne jamais
utiliser `email`, `preferred_username`, `phone_number` ou `name` comme identifiant. Une
même personne venue de deux émetteurs doit être deux identités. [K2, OIDC Core §5.7]

Acceptation :
- Deux connexions avec le même courriel et deux `sub` différents créent deux identités.
- Deux émetteurs qui émettent le même `sub` créent deux identités.

#### REQ-005-2 — Création à la première connexion
Lors de la première connexion réussie d'une personne, kartenn doit créer son identité
avec pour seules données (émetteur, `sub`) et un nom affiché tiré du claim `name`, mis à
jour à chaque connexion ; aucun courriel. [K3]

Acceptation :
- Après une première connexion, la table des identités contient (émetteur, `sub`, nom),
  et un événement `authn.login.succeeded` attribué à la nouvelle identité.
- La pseudonymisation de la 004 efface le nom et conserve l'identifiant interne.

#### REQ-005-3 — État d'une identité
*Amendée par la 022 (A6, REQ-022-18, 2026-10-23) : un flux d'événements ouvert, qui n'a pas de « requête suivante », est fermé dans les 10 s.*
kartenn doit permettre de désactiver une identité et de révoquer toutes ses sessions
(horodatage « révoqué après »), par changeset ; une identité désactivée ou une session
antérieure à la révocation doit être refusée à la requête suivante. [K10]

Acceptation :
- Une session ouverte, la révocation appliquée, la requête suivante est refusée et
  produit `authz.access.denied` (raison `session.revoked`).

### B. Connexion des personnes (BFF)

#### REQ-005-4 — Flux OIDC du serveur
kartenn doit mener lui-même, en client confidentiel, le flux OIDC par code avec PKCE
(S256), `state` et `nonce` à usage unique liés au navigateur, URI de redirection
comparée exactement, et valider le jeton d'identité selon OIDC Core §3.1.3.7 (émetteur,
audience, signature avec les clés de l'émetteur, expiration, `nonce`). [K1, RFC 9700
§2.1, §2.1.1, §4.7.1, OIDC Core §3.1.3.7]

Acceptation :
- Un `state` rejoué, un `nonce` différent, une audience étrangère, une signature
  invalide ou un jeton expiré : connexion refusée, `authn.login.failed` avec la raison.

#### REQ-005-5 — Plusieurs émetteurs, confusion d'émetteur
Où plusieurs émetteurs sont configurés, kartenn doit mémoriser l'émetteur visé par
chaque connexion et refuser une réponse d'un autre émetteur, en vérifiant le paramètre
`iss` de la réponse quand l'émetteur le publie. [K11, RFC 9700 §4.4.2, RFC 9207 §2.4]

Acceptation :
- Une réponse d'autorisation portant un `iss` différent de l'émetteur visé est rejetée
  avant l'échange du code.

#### REQ-005-6 — Session du navigateur
kartenn doit remettre au navigateur une session sous forme d'un cookie chiffré et
authentifié, sans état côté serveur, préfixé `__Host-`, `Secure`, `HttpOnly`,
`SameSite=Strict`, `Path=/`, sans `Domain` ; aucun jeton OAuth ne doit être lisible par
le navigateur. [K1, RFC 10017 §6.1, §6.1.3.2]

Acceptation :
- Le cookie, déchiffré par une autre instance, donne la même session ; modifié d'un
  octet, il est refusé.
- Aucune réponse au navigateur ne contient de jeton d'accès, d'identité ou de
  rafraîchissement.

#### REQ-005-7 — Durées de session
*Amendée par la 022 (A6, REQ-022-18, 2026-10-23) : un flux d'événements n'est pas une activité de session ; il est fermé à l'échéance.*
kartenn doit fermer une session après 30 minutes d'inactivité et 8 heures au plus depuis
la connexion (réglables, sans dépasser 1 h et 24 h) ; il doit rafraîchir les groupes
auprès de l'émetteur à l'expiration du jeton d'accès. [K10, 800-63B §2.2]

Acceptation :
- À 30 min 1 s d'inactivité, ou 8 h après la connexion, la requête est refusée et
  `authn.session.closed` est enregistré.
- Un groupe retiré à l'IdP n'ouvre plus ses droits après le rafraîchissement suivant.

#### REQ-005-8 — Déconnexion
Lors d'une déconnexion, kartenn doit invalider le cookie et, si l'émetteur publie
`end_session_endpoint`, rediriger vers lui avec `id_token_hint`. [K10, RP-Initiated
Logout §2, §3]

### C. Jetons des clients non navigateur

#### REQ-005-9 — Jetons d'accès Bearer
kartenn doit accepter des jetons d'accès JWT d'un émetteur configuré selon RFC 9068 §4 :
`typ` `at+jwt`, `iss` exact, audience de kartenn, signature par une clé de l'émetteur et
un algorithme de la liste fermée de l'émetteur (jamais `none`), `exp` avec une tolérance
d'horloge d'au plus 60 s ; et refuser un jeton d'identité présenté comme jeton d'accès.
[K1, RFC 9068 §4, RFC 8725 §3.1, §3.9, §3.12]

Acceptation :
- Un jeton d'identité, un jeton `alg: none`, un jeton d'un autre émetteur signé par une
  clé valide de cet autre émetteur : refusés (`invalid_token`).

#### REQ-005-10 — Clés des émetteurs
kartenn doit lire les clés de chaque émetteur depuis ses métadonnées de découverte, les
garder en cache et les recharger quand un jeton présente un identifiant de clé inconnu,
au plus une fois par minute et par émetteur. [K1, RFC 8725 §3.8]

Acceptation :
- Après une rotation des clés de l'émetteur, un jeton signé par la nouvelle clé est
  accepté sans redémarrage ; mille jetons à `kid` inconnu ne provoquent qu'un
  rechargement.

### D. Comptes de service

#### REQ-005-11 — Secrets générés
kartenn doit générer chaque secret de compte de service (256 bits aléatoires), ne le
montrer qu'une fois et n'en stocker qu'une empreinte HMAC-SHA-256 sous une clé serveur ;
un compte peut avoir deux secrets actifs à la fois (rotation) et chaque secret une date
d'expiration. [K8, D13 amendé]

Acceptation :
- Le secret n'apparaît ni en base, ni dans l'audit, ni dans les logs (REQ-012-28).
- Après rotation, l'ancien et le nouveau secret fonctionnent jusqu'au retrait de
  l'ancien ; un secret expiré est refusé.

#### REQ-005-12 — Authentification Basic
kartenn doit authentifier un compte de service par Basic (RFC 7617) sur TLS seulement,
en comparant l'empreinte en temps constant, et lire l'état du compte (actif, secrets,
expiration) à chaque requête ; un compte désactivé est refusé à la requête suivante.
[K8, INV-20]

Acceptation :
- Un en-tête Basic reçu hors TLS est refusé sans vérification.
- La vérification d'un secret coûte moins de 1 ms au p95.

#### REQ-005-13 — Échecs répétés
Si les échecs d'authentification se répètent depuis une même source, alors kartenn doit
limiter leur débit sans verrouiller les comptes de service, et agréger les échecs dans
l'audit (REQ-012-19). [K14]

### E. Comptes de secours

#### REQ-005-14 — Création hors API
kartenn ne doit créer un compte humain de secours que par une sous-commande du binaire
exécutée avec le rôle propriétaire du schéma, pour une durée de validité bornée ; aucun
n'existe par défaut. La même sous-commande doit lier l'identité du compte au rôle livré
`administrator`, portée globale, jusqu'à la fin de validité du compte : c'est la seule
voie d'amorçage d'une instance neuve. [K9, K17]

Acceptation :
- Aucune opération de l'API ni du pipeline ne crée ni ne modifie un compte de secours.
- Sur une instance neuve, un compte créé par la sous-commande ouvre une session de secours
  qui peut lier un groupe OIDC à un rôle ; `account.breakglass.created` nomme le rôle et
  la portée ; après `valid_until`, la liaison n'a plus d'effet.

#### REQ-005-15 — Deux facteurs
kartenn doit exiger d'un compte de secours un mot de passe d'au moins 15 caractères,
sans règle de composition, haché en Argon2id (64 Mio, t = 3, p = 4, sel 128 bits), et un
code TOTP (RFC 6238, pas de 30 s, un pas de retard toléré, code jamais accepté deux
fois) dont le secret est stocké chiffré ; au-delà de 100 échecs consécutifs, le compte
est désactivé. [K9, 800-63B §3.1.1.2, §3.2.2, RFC 6238 §5.2, RFC 9106 §4]

Acceptation :
- Le bon mot de passe avec un code TOTP déjà utilisé : refus.
- Chaque utilisation produit un événement d'audit de criticité haute et un WARN.

### F. Modèle de contrôle d'accès

#### REQ-005-16 — Catalogue des permissions
kartenn doit tenir un catalogue des permissions déclarées par chaque module
(`<module>.<objet>.<action>`, actions `read`, `write`, `propose`, `approve` ; `prop.<clé>.
write`, `prop.<clé>.propose` ; `prop.<clé>.read` pour les propriétés sensibles), publié
dans `docs/spec/` et vérifié par un test. [K12, D36]

#### REQ-005-17 — Rôles
kartenn doit livrer les rôles lecteur, éditeur, approbateur et administrateur (K12) et
permettre des rôles personnalisés ; une permission inconnue du catalogue doit être
refusée. [K12, D36]

#### REQ-005-18 — Liaisons et périmètres
kartenn doit accorder une permission à un sujet sur un objet si et seulement si une
liaison valide à la date de la requête associe ce sujet (l'identité, l'un de ses groupes
ou son émetteur entier) à un rôle qui contient la permission, sur un périmètre qui
contient l'objet (héritage vers le bas : global, entité, site, domaine, sous-arbre de
préfixes), **et** si l'objet est dans le confinement de l'émetteur du sujet. [K4, K5,
K13, K14]

Acceptation :
- Lecteur sur l'entité E : lit les adresses dont le propriétaire résolu est dans le
  sous-arbre de E, pas les autres.
- Une liaison globale d'une identité d'un émetteur rattaché à l'entité F n'ouvre que le
  sous-arbre de F.
- Une liaison dont la période est échue n'accorde plus rien.

#### REQ-005-19 — Refus par défaut
Si aucune liaison n'accorde la permission, alors kartenn doit refuser, y compris en
lecture, et enregistrer `authz.access.denied`. [K5, INV-25]

Acceptation :
- Une identité sans liaison ne lit aucun objet ; une liste lui renvoie une page vide.

#### REQ-005-20 — Confinement des émetteurs
kartenn doit borner les droits obtenus via un émetteur aux objets dont l'entité
propriétaire résolue (`core.owner`) est dans les sous-arbres rattachés à cet émetteur,
sauf rattachement global ; un émetteur sans rattachement ne doit ouvrir aucun droit ; un
objet sans propriétaire n'est atteignable que par un émetteur à rattachement global, un
compte de service ou un compte de secours. [K4, K11, K13]

#### REQ-005-21 — Valeurs sensibles
Où une propriété est déclarée sensible, kartenn doit masquer sa valeur à qui n'a pas
`prop.<clé>.read` sur l'objet, sans masquer l'objet. [K12, 003/P17]

Acceptation :
- Un lecteur sans le droit voit l'objet et la clé, pas la valeur ; aucun événement
  `data.sensitive.read` n'est produit pour lui.

#### REQ-005-22 — Droits des écritures
kartenn doit exiger, pour chaque opération d'un changeset, la permission d'écriture (ou
de proposition) du type d'objet sur l'objet visé, en plus des droits de propriété déjà
vérifiés (003) ; l'approbation d'un changeset en attente exige `approve` sur le
périmètre des objets qu'il touche. [K12, REQ-003-17, REQ-004-41]

Acceptation :
- Un éditeur d'une entité ne crée pas d'adresse dans un réseau d'une autre entité ; avec
  `core.address.propose`, sa demande devient une proposition (004).

### G. Décision et filtrage

#### REQ-005-23 — Décision groupée
kartenn doit répondre à `authorize_many(sujet, permission, objets)` en une requête par
lot, quel que soit le nombre d'objets. [D36, 003/design § Droits]

#### REQ-005-24 — Filtrage des listes
kartenn doit filtrer toute liste et toute recherche par les mêmes règles que la décision
unitaire, par un prédicat SQL, sans renvoyer d'objet non autorisé ni de nombre total qui
en compterait. [D36, K5]

Acceptation :
- Pour un échantillon d'objets, la liste filtrée et la décision unitaire coïncident
  (test de propriété).

### H. Administration

#### REQ-005-25 — Par changesets
kartenn doit faire passer toute création ou modification de rôle, de liaison, de
rattachement d'émetteur et de compte de service par un changeset (validé, versionné en
bitemporel, audité, soumis aux politiques d'approbation, simulable et annulable). [K6,
D11]

Acceptation :
- « Qui avait le rôle administrateur sur E le 3 mars ? » se répond par une lecture datée.
- Une politique peut exiger un second administrateur pour toute liaison du rôle
  administrateur.
- L'annulation d'un changeset qui crée un rôle, lie ce rôle et rattache un émetteur rend
  l'état d'avant ; celle d'un changeset qui clôt un rôle rend le rôle et ses liaisons
  (K16). Les comptes de service, leurs secrets et l'état des identités sont listés comme
  non annulables.

#### REQ-005-26 — Délégation sans escalade
kartenn doit n'autoriser une liaison que si son auteur détient `core.rbac.admin` sur un
périmètre qui contient celui de la liaison, et toutes les permissions du rôle lié sur ce
périmètre ; définir ou modifier un rôle exige `core.role.admin`, rattacher un émetteur
`core.issuer.admin`, tous deux sur le périmètre global. [K7, K11]

Acceptation :
- Un administrateur d'entité ne peut ni lier le rôle administrateur au-dessus de son
  entité, ni lier un rôle qui contient une permission qu'il n'a pas, à lui ou à un autre.

#### REQ-005-27 — Périmètres des politiques d'approbation
kartenn doit permettre aux politiques d'approbation (004) de viser un périmètre (entité,
site, sous-arbre) et de désigner des auteurs par rôle ou par groupe. [K14, 004 tâche 13]

### I. Audit et performance

#### REQ-005-28 — Événements
kartenn doit enregistrer `authn.login.succeeded`, `authn.login.failed` (raison codée,
identifiant revendiqué en empreinte), `authn.session.closed`, `authz.access.denied`, les
utilisations des comptes de secours (criticité haute) et les changesets d'administration
des droits. [REQ-012-8, REQ-012-9]

#### REQ-005-29 — Budgets
kartenn doit tenir les budgets INV-30 à INV-36 avec le contrôle d'accès actif, pour 1 000
liaisons et 50 groupes par identité. [D14, K14, K15]

Acceptation :
- Mesuré par le banc `bench-rbac` (jeu de `bench-core` et un jeu de liaisons) : chaque
  p95 avec contrôle d'accès sous son budget ; le surcoût relatif est rapporté.
- ~~Surcoût de la décision et du filtrage sous 10 % du p95~~ : retiré (K15), hors d'atteinte
  sans cache pour des lectures sous la milliseconde, et sans effet sur les budgets.

## Non-objectifs

- Liaison d'identités entre émetteurs (K2) ; provisionnement SCIM.
- Authentification mutuelle TLS des comptes de service ; jetons opaques et introspection
  (RFC 7662) ; échange `client_credentials` (K8).
- Règles de refus explicites, contrôle par attributs (ABAC), moteur de politiques externe
  (OPA, Cedar : adaptateur possible plus tard, ADR 0010).
- Second facteur pour les personnes : à la charge de leur IdP.
- Routes HTTP, formats d'erreur, protection CSRF concrète : 006.

## Questions ouvertes

- [x] Clé serveur des empreintes HMAC et du chiffrement des cookies et des secrets TOTP :
  fichier, rotation, partage entre instances. (Claude, conception) — `auth.key_file`, un
  trousseau partagé, clé courante en tête (design « Trousseau »).
- [x] Cache des liaisons et de l'état des identités entre instances : durée, invalidation
  après un changeset de droits. (Claude, conception) — état des identités : 5 s, oublié
  par l'instance qui le change ; liaisons : pas de cache (K15).
- [x] REQ-005-25 « annulable » : l'annulation (004) n'avait pas d'inverse pour les
  opérations d'administration. (utilisateur) — inverses des rôles, liaisons et
  rattachements par des opérations ordinaires (K16) ; comptes de service, secrets et état
  des identités non annulables.
