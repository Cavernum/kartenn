# 005 — Authentification et RBAC : journal des décisions

Entretien (grill-me) du 2026-10-17. Cadre : D11, D13, D36, ADR 0009, 0010 ; port
`Authorizer` (`kartenn-service/src/authz.rs`) ; différés de la 002, 003, 004 et 012
(permissions par type d'objet, droits de lecture des propriétés sensibles, périmètres
des politiques d'approbation, événements `authn.*` et `authz.*`).

Périmètre retenu pour l'entretien : modèle d'identité et vérification des preuves
derrière des ports ; RBAC réel (catalogue, rôles, liaisons, périmètres hérités) derrière
`Authorizer` ; filtrage des listes ; administration auditée. Les routes HTTP, cookies et
formats sont la 006, qui appelle ce que la 005 fournit.

## K1 — Jetons du navigateur
- Décision : **BFF**. Le serveur mène le flux OIDC code + PKCE en client confidentiel ;
  le navigateur ne détient qu'un cookie de session chiffré (`HttpOnly`, `Secure`,
  `SameSite`), sans état côté serveur (D9) ; protection CSRF avec la 006. Les jetons
  d'accès `Bearer` restent acceptés pour les clients non navigateur.
- Pourquoi : aucun jeton n'est lisible par le JavaScript, donc ni par une injection ni
  par un plugin UI tiers (D17, INV-23) ; recommandation la plus forte des bonnes pratiques
  OAuth pour les applications navigateur.
- Rejeté : client public dans le navigateur (jetons exposés à tout script) ; les deux
  (deux chemins à sécuriser).
- Décidé par : recommandation acceptée

## K2 — Une personne, deux émetteurs (Q-ouv-5)
- Décision : **deux identités** distinctes, clé (émetteur, `sub`) ; droits et audit par
  identité ; aucune liaison en v0.1 (non-objectif).
- Pourquoi : prévu par D13 ; un émetteur n'obtient des droits que sur ses entités ; une
  fusion par courriel laisserait un émetteur s'approprier le compte d'un autre.
- Rejeté : liaison manuelle (reportée, sans besoin établi) ; liaison par courriel
  (fusion de comptes).
- Décidé par : recommandation acceptée ; Q-ouv-5 close.

## K3 — Création des identités, données conservées
- Décision : identité humaine créée **à la première connexion réussie** (juste-à-temps),
  sans pré-enregistrement ; seuls (émetteur, `sub`) et un nom affiché (claim `name`, mis à
  jour à chaque connexion) sont conservés ; pas de courriel. La pseudonymisation de la
  004 s'applique.
- Pourquoi : minimisation des données personnelles ; le cycle de vie des personnes reste
  chez leur IdP ; une identité sans liaison n'obtient que les droits par défaut.
- Rejeté : courriel conservé (donnée de plus sans usage établi) ; pré-enregistrement
  (lourd pour les prestataires).
- Décidé par : recommandation acceptée

## K4 — Origine des droits, confinement des émetteurs
- Décision : les **liaisons** (rôle × périmètre) sont administrées dans kartenn et visent
  une identité, un compte de service ou un **groupe d'un émetteur** (valeur d'un claim
  configuré par émetteur, `groups` par défaut, lue dans le jeton à chaque requête, jamais
  stockée). Chaque émetteur est **confiné** aux sous-arbres de ses entités : une liaison
  d'une de ses identités ou de ses groupes n'agit que sur les objets dont l'entité
  propriétaire (`core.owner`, ADR 0014) est dans ces sous-arbres, même si elle a été
  posée plus large.
- Pourquoi : l'IdP dit qui appartient à quel groupe, kartenn dit ce que le groupe peut
  faire et où (D36) ; un IdP de prestataire ne peut pas s'accorder plus que ses entités.
  Le retrait d'un groupe à l'IdP prend effet au renouvellement du jeton.
- Rejeté : correspondance dans la configuration (ni auditée ni modifiable sans
  redéploiement) ; rôles fournis par l'IdP (délègue le contrôle à des IdP tiers).
- Décidé par : recommandation acceptée

## K5 — Droits sans liaison
- Décision : **refus par défaut** ; la lecture est un droit comme un autre
  (`core.<objet>.read`, rôle « lecteur »). Une liaison peut viser **toutes les identités
  d'un émetteur**, pour les droits par défaut d'une population, dans le confinement de
  K4. Les listes sont filtrées par les mêmes règles (prédicats SQL, D36).
- Pourquoi : une cartographie réseau est une information sensible ; refus par défaut
  (ASVS) ; le sujet « émetteur entier » évite de lier chaque personne.
- Rejeté : lecture ouverte à toute identité authentifiée.
- Décidé par : recommandation acceptée

## K6 — Administration des rôles et liaisons
- Décision : **par changesets** (D11) : opérations de rôle et de liaison dans le
  pipeline, versionnées en bitemporel, auditées, soumises aux politiques d'approbation,
  simulables et annulables ; une liaison temporaire est une période applicative bornée.
- Pourquoi : « qui avait quel droit à telle date » devient une lecture datée ; l'exigence
  d'un second administrateur s'exprime par une politique d'approbation existante ;
  l'expiration ne demande aucun mécanisme de plus.
- Rejeté : API d'administration hors pipeline (ni historique daté ni approbation).
- Décidé par : recommandation acceptée

## K7 — Délégation, anti-escalade
- Décision : `core.rbac.admin` sur un périmètre permet d'y poser des liaisons, **dans ce
  périmètre seulement**, et **pour des rôles dont l'administrateur détient toutes les
  permissions** sur ce périmètre. Définir ou modifier un rôle exige `core.role.admin` sans
  restriction de périmètre (un rôle vaut partout).
- Pourquoi : un responsable d'entité gère les accès de son sous-arbre sans pouvoir élever
  quiconque, lui compris, au-delà de ses propres droits.
- Rejeté : administrateurs globaux seuls (goulet pour les prestataires et filiales).
- Décidé par : recommandation acceptée

## K8 — Secrets des comptes de service (amende D13)
- Décision : kartenn **génère** les secrets des comptes de service (256 bits aléatoires,
  montrés une seule fois) et les stocke en **HMAC-SHA-256** avec une clé serveur ;
  rotation par deux secrets actifs à la fois, expiration, révocation immédiate (état lu
  en base à chaque requête). Argon2id (RFC 9106) reste pour les mots de passe humains
  (comptes de secours).
- Pourquoi : un secret aléatoire de 256 bits ne se devine pas ; un hachage lent n'ajoute
  rien et coûterait des dizaines de millisecondes à chaque requête d'un script ; pas de
  cache de secrets en mémoire.
- Rejeté : Argon2id avec cache par instance (état sensible en mémoire, plus de code) ;
  échange contre un jeton `client_credentials` (chaque script gère le renouvellement).
- **Contredit D13** (« secrets hachés en Argon2id ») pour les comptes de service :
  signalé, accepté par l'utilisateur ; D13 précisé.
- Décidé par : recommandation acceptée

## K9 — Comptes humains de secours
- Décision : aucun par défaut ; créés seulement par une sous-commande du binaire avec le
  rôle propriétaire de la base (jamais par l'API), pour une durée bornée ; mot de passe
  de 15 caractères au moins (NIST SP 800-63B) haché en Argon2id (RFC 9106) ; **second
  facteur TOTP obligatoire** (RFC 6238), secret stocké chiffré ; chaque utilisation :
  événement d'audit de criticité haute et WARN.
- Pourquoi : un compte de secours a des droits élevés et sert quand l'IdP est en panne ;
  un mot de passe volé ne doit pas suffire.
- Rejeté : sans second facteur ; pas de secours du tout (seule une intervention en base
  resterait).
- Décidé par : recommandation acceptée

## K10 — Sessions du navigateur, révocation
- Décision : cookie de session chiffré sans état, **inactivité 30 min, durée absolue
  8 h** (réglables) ; groupes rafraîchis auprès de l'IdP à l'expiration du jeton d'accès
  (jeton de rafraîchissement dans le cookie chiffré) ; à chaque requête, lecture de
  l'état de l'identité (indexée, cache de quelques secondes) : une identité désactivée,
  ou dont les sessions sont révoquées (horodatage « révoqué après »), est refusée
  aussitôt. Déconnexion locale et auprès de l'IdP (RP-Initiated Logout).
- Pourquoi : un cookie sans état ne se révoque pas avant son terme ; le contrôle d'état
  rend la coupure immédiate sans stocker de session.
- Rejeté : sessions longues (poste perdu) ; sans révocation.
- Décidé par : recommandation acceptée

## K11 — Déclaration des émetteurs
- Décision : la partie technique d'un émetteur (URL de découverte, client et secret du
  BFF, audience, claim des groupes, algorithmes acceptés) est dans la **configuration**,
  secrets par fichier ou variable ; son **rattachement à des entités** (K4) est dans
  kartenn, par changeset audité, sous `core.issuer.admin`. Un émetteur rattaché à aucune
  entité n'ouvre aucun droit (échec fermé).
- Pourquoi : pas de secret en base ; la décision de sécurité (ce que l'émetteur peut
  atteindre) est historisée, auditée et soumise aux approbations.
- Rejeté : tout dans la configuration (périmètre changé sans trace) ; tout en base
  (secrets du client en base).
- Décidé par : recommandation acceptée

## K12 — Permissions et rôles livrés
- Décision : permissions `<module>.<objet>.<action>`, actions `read`, `write` (créer,
  modifier, clore), `propose` (004) et `approve` (valider un changeset en attente du
  périmètre) ; par propriété : `prop.<clé>.write`, `prop.<clé>.propose` (003) et
  `prop.<clé>.read` pour les seules propriétés **sensibles** (sans ce droit, l'objet reste
  visible, la valeur est masquée). Rôles livrés : **lecteur** (lectures non sensibles),
  **éditeur** (lecteur + écriture des objets et des propriétés non sensibles),
  **approbateur** (lecteur + `approve`), **administrateur** (tout, dont
  `core.rbac.admin` ; `core.role.admin` et `core.issuer.admin` ne s'accordent que sur le
  périmètre global). Catalogue généré dans `docs/spec/`, comme ceux des règles et des
  événements.
- Pourquoi : assez fin pour limiter un prestataire aux adresses d'un réseau ; la lecture
  des valeurs sensibles (003/P17) a enfin son droit.
- Rejeté : permissions par module (trop grossières).
- Décidé par : recommandation acceptée

## K13 — Objets sans propriétaire, rattachement global
- Décision : le rattachement d'un émetteur peut être **global** (typiquement l'IdP
  interne), décision explicite et auditée sous `core.issuer.admin`. Un émetteur rattaché à
  des entités n'atteint que les objets qu'elles possèdent ; un objet sans propriétaire
  résolu reste hors de sa portée (échec fermé). Comptes de service et de secours ne sont
  pas confinés par un émetteur : leurs liaisons suffisent.
- Pourquoi : l'IdP interne n'a pas à attendre que chaque objet ait un propriétaire ; un
  prestataire ne voit jamais un objet non attribué à ses entités.
- Rejeté : propriétaire obligatoire sur tout objet (impose `core.owner` avant toute
  saisie).
- Décidé par : recommandation acceptée

## K14 — Points tranchés sans question (délégué)
- **Périmètres** : global ; entité (objets dont le propriétaire résolu est dans son
  sous-arbre, et le sous-arbre des entités lui-même) ; site (idem par `core.site`) ;
  domaine (préfixes, plages, adresses du domaine) ; sous-arbre de préfixes (le préfixe et
  ce qu'il contient dans son domaine). Héritage vers le bas ; aucune règle de refus
  explicite (l'union des liaisons fait les droits, le confinement les borne).
- **Politiques d'approbation** (différé de la 004) : périmètres par entité, site ou
  sous-arbre, auteurs désignés par rôle ou groupe, dans le même modèle.
- **Échecs d'authentification** : aucun verrouillage des comptes de service (secrets
  impossibles à deviner), mais limitation de débit par source ; comptes de secours :
  limitation par compte (NIST SP 800-63B : au plus 100 échecs consécutifs), agrégation
  des échecs par REQ-012-19.
- **Budgets** (D14) : les contrôles de droits et le filtrage des listes tiennent les
  budgets INV-30 à 36 avec 1 000 liaisons et 50 groupes par identité ; surcoût sur le p95
  mesuré au banc.
- **Non-objectifs** : liaison d'identités entre émetteurs, provisionnement SCIM,
  authentification mutuelle TLS des comptes de service, jetons opaques (introspection
  RFC 7662), règles de refus, ABAC.
- Décidé par : délégué (2026-10-17)

## K15 — Surcoût du contrôle d'accès (banc 005-16)
- Décision : le critère de REQ-005-29 est la tenue des budgets absolus INV-30 à 36 avec
  le contrôle d'accès actif ; le surcoût relatif est mesuré et rapporté par
  `bench-rbac`, sans seuil. Pas de cache des visibilités pour l'instant.
- Pourquoi : au jeu de référence, les budgets tiennent largement (lecture 12,7 ms pour
  50 ms, page 55 ms pour 150 ms, sous-arbre 10 ms pour 200 ms, changeset 11 ms pour
  200 ms). Un surcoût de 10 % sur une lecture d'1,2 ms laisse 120 µs, moins qu'un aller-
  retour en base : il imposerait un cache par instance, donc un délai de prise d'effet
  d'un retrait de droits entre instances.
- Rejeté : cache des visibilités par sujet avec durée de 5 s (retrait de droits différé
  sur les autres instances, code en plus pour un gain sans effet sur les budgets) ;
  dénormalisation du domaine et de la valeur du préfixe dans la liaison (copie à
  maintenir).
- Décidé par : utilisateur (2026-10-18)

## K16 — Annulation des changesets d'administration (REQ-005-25)
- Décision : l'annulation (004) inverse les rôles, leurs permissions, les liaisons et les
  rattachements d'émetteurs par des **opérations ordinaires** : un rôle créé est clos
  (`CloseRole`), un rôle modifié ou clos reprend son libellé et ses permissions d'avant
  (`SetRole`) ; une liaison créée est close (`Unbind`), une liaison close est recréée
  (`Bind`, nouvel identifiant) sur sa période d'avant ; de même `DetachIssuer` et
  `AttachIssuer`. Un rôle, une liaison ou un rattachement écrit depuis par un autre
  changeset est un conflit, laissé de côté ; un rôle créé que lie une liaison d'un autre
  changeset aussi. Les comptes de service, leurs secrets et l'état des identités restent
  non annulables (tables sans historique).
- Pourquoi : l'inverse passe par les mêmes contrôles que l'opération directe (droits
  d'administration, pas d'escalade, politiques d'approbation, audit) ; une restauration
  brute des lignes contournerait le contrôle d'escalade sur une liaison recréée.
- Rejeté : garder l'écart (amender REQ-005-25) ; restauration ligne à ligne comme
  `Restore` (contrôles des liaisons à réécrire pour elle, identifiants conservés au prix
  d'un chemin de droits distinct).
- Décidé par : utilisateur (inverses, 2026-10-18) ; forme : recommandation de Claude
