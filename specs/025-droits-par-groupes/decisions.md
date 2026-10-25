# 025 — Droits par groupes : journal des décisions

Statut : **validée le 2026-10-25**. Écrit à partir de la revue des maquettes du
2026-10-25 (E7 Administration) et des arbitrages du même jour.

Cadre : D13 (authentification), D36 (RBAC composable, liaisons portées) ; 005/K4 à K7,
K13, K17 ; REQ-005-18, 26, 27 ; 004 (politiques d'approbation) ; 006 (routes des droits) ;
021 (E7) ; 024/M7 (`edit`, `apply`, `apply_own`).

Règle permanente de l'utilisateur : rien n'est gardé pour compatibilité avant le gel.

## Synthèse

- G1 plus de droits nominaux : une liaison lie un rôle à un **groupe kartenn** sur un
  périmètre ; jamais à une identité ni à un émetteur entier. Amende REQ-005-18 et K4.
- G2 les groupes sont tenus par kartenn ; leurs membres sont des identités (personnes,
  comptes de service) et des groupes de l'IdP (émetteur + nom du groupe du jeton).
- G3 les liaisons sur un émetteur entier disparaissent, sans remplaçant : pas de membre
  « émetteur entier ».
- G4 un groupe porte ses liaisons : l'administration des droits se fait groupe par groupe
  (membres, rôles sur périmètres) ; la notion de liaison reste dans le modèle et l'API,
  plus comme écran à part.
- G5 anti-escalade étendue à l'appartenance : changer les membres d'un groupe exige de
  pouvoir créer chacune de ses liaisons. Amende K7 et REQ-005-26.
- G6 pas de groupes imbriqués (un groupe kartenn n'est pas membre d'un autre).
- G7 compte de secours : membre du groupe livré `secours`, lié au rôle administrateur
  global ; plus de liaison nominale à l'amorçage. Amende K17.
- G8 permissions d'administration : `core.rbac.admin` (liaisons et membres, sur un
  périmètre) reste ; `core.group.admin` n'est pas créé.
- G9 migration sans compatibilité : liaisons de groupe IdP converties, liaisons
  nominales et d'émetteur converties ou retirées (voir G9).
- G10 auteurs des politiques d'approbation : rôles et groupes kartenn, plus de compte
  nommé.

## G1 — Plus de droits nominaux

Demande de l'utilisateur (2026-10-25) : « pas de permissions nominales, toutes les
permissions se font sur des groupes, ensuite on assigne les comptes dans des groupes ».

- Décision : `binding.subject` n'a plus qu'une sorte, le groupe kartenn. Les droits
  d'une identité sont l'union des liaisons des groupes dont elle est membre, directement
  ou par un groupe de son jeton.
- Effet : on retire un droit à une personne en la sortant d'un groupe, jamais en
  cherchant ses liaisons ; l'audit d'un droit se lit par groupe.

## G2 — Groupes tenus par kartenn

Arbitrage du 2026-10-25 (question posée en séance), retenu parmi trois options :
groupes kartenn ; groupes de l'IdP ou kartenn ; groupes de l'IdP seuls.

- Décision : `group (id, key, label, builtin)`, versionnée comme les rôles. Membres :
  identité (personne ou compte de service) ou groupe de l'IdP (`issuer`, `group_name`).
  L'appartenance est versionnée (période applicative : appartenance temporaire possible,
  comme une liaison temporaire de K6).
- Les comptes de service, sans jeton d'IdP, entrent dans un groupe comme membres directs.
- Un groupe de l'IdP n'est pas un sujet : il n'a de droits qu'en étant membre d'un
  groupe kartenn. Le nom du groupe du jeton reste la valeur servie par l'IdP.
- Écartés : groupes de l'IdP comme sujets à côté des groupes kartenn (deux sortes de
  sujets à afficher et à auditer) ; groupes de l'IdP seuls (rien pour les comptes de
  service sans liste déclarée à part).

## G3 — Plus de liaisons sur un émetteur entier

Arbitrage du 2026-10-25 : retirées, sans membre « émetteur entier ». Pour ouvrir un droit
à tous les comptes d'un IdP, l'IdP sert un groupe qui les contient, déclaré membre d'un
groupe kartenn. Le confinement par émetteur (K13, `issuer_attachment`) est inchangé : il
borne toujours ce qu'une identité d'émetteur peut atteindre, quels que soient ses groupes.

## G4 — L'administration se fait par groupe

Demande de l'utilisateur : « vois si tu peux simplifier la gestion des droits, car cela
ne concerne plus les utilisateurs ».

- Décision : l'écran Administration (021, E7) montre des **Groupes** (membres, rôles sur
  périmètres), des **Rôles** (permissions) et des **Identités** (état : désactiver,
  révoquer les sessions ; groupes en lecture). L'onglet « Liaisons » disparaît : une
  liaison s'ajoute, se modifie et se retire depuis son groupe.
- La fiche d'une identité affiche ses groupes et ses droits effectifs (calculés), sans
  rien d'éditable côté droits.
- L'API garde `bindings` (une liaison est un objet versionné, audité, annulable) avec
  `group` pour sujet ; elle ajoute `groups` et `groups/{id}/members`.

## G5 — Anti-escalade étendue à l'appartenance

Demande de l'utilisateur : « il ne faut pas pouvoir donner des droits sur un scope
supérieur à ce qu'on a accès. Si on peut gérer des permissions, alors on ne peut le faire
que sur au maximum son propre scope ».

- K7 reste pour une liaison : `core.rbac.admin` sur un périmètre qui contient le sien, et
  toutes les permissions du rôle sur ce périmètre.
- Ajout : ajouter ou retirer un membre d'un groupe exige, pour **chaque** liaison valide
  du groupe, ce que K7 exige pour la créer. Sinon entrer quelqu'un dans un groupe global
  donnerait un droit qu'on n'a pas. Refus : `core.group.escalation`.
- Retirer un membre est soumis à la même règle : un administrateur d'entité ne vide pas
  un groupe global.
- Créer un groupe vide demande `core.rbac.admin` sur un périmètre quelconque (un groupe
  sans liaison ne donne rien). Clore un groupe clôt ses liaisons et ses appartenances, et
  demande ce que G5 demande pour chacune.

## G6 — Pas de groupes imbriqués

Écarté : un groupe kartenn membre d'un autre. La résolution resterait récursive, l'audit
et la règle G5 deviendraient transitifs. Les groupes de l'IdP couvrent déjà la
hiérarchie de l'annuaire.

## G7 — Compte de secours

- Le groupe livré `secours` (`builtin`) est lié au rôle administrateur global par la
  synchronisation des rôles livrés (005, comme les rôles).
- La sous-commande `breakglass create` ajoute le compte à ce groupe sur
  `[création, valid_until)`, au lieu de la liaison nominale de K17 ; le reste de K17 ne
  change pas (écriture directe par l'adaptateur, sans identité appelante).
- Un groupe livré ne se modifie pas hors synchronisation (comme un rôle livré) ; ses
  membres, eux, se gèrent normalement (G5 : il faut être administrateur global).

## G8 — Permissions d'administration

- `core.rbac.admin` couvre liaisons et appartenances, sur un périmètre (G5).
- `core.role.admin` et `core.issuer.admin` restent globales.
- Pas de `core.group.admin` : la création d'un groupe vide ne donne rien (G5) et une
  permission de plus compliquerait le catalogue.

## G9 — Migration

Pas de compatibilité (règle de l'utilisateur), mais pas de perte silencieuse :

- liaison d'un groupe de l'IdP → groupe kartenn `idp-<émetteur>-<groupe>` qui a ce groupe
  pour seul membre, liaison reportée sur lui (un groupe par couple, réutilisé) ;
- liaison nominale → groupe kartenn `compte-<clé de l'identité>` qui a l'identité pour
  seul membre, liaison reportée ; ces groupes sont listés à la sortie de `migrate`
  (à regrouper à la main) ;
- liaison d'un émetteur entier → retirée, listée à la sortie de `migrate` et auditée.

La migration passe par un changeset système (comme la synchronisation des rôles livrés),
pour que l'historique montre le report.

## G10 — Auteurs des politiques d'approbation

REQ-005-27 et la 004 désignent des auteurs « par rôle ou par groupe » : le groupe y est
désormais un groupe kartenn ; un compte nommé n'est plus un auteur désignable (G1).

## Questions tranchées

- Q-025-1 (oui, 2026-10-25) : la fiche d'une identité affiche aussi les groupes de l'IdP
  de sa dernière session, en lecture, avec la date de la session, pour expliquer une
  appartenance indirecte. Ils sont gardés avec la session (005) ; une identité sans
  session n'en montre aucun.
  - Reprise le 2026-10-25 (arbitrage de l'utilisateur) : les sessions sont des cookies
    scellés (ADR 0020), le serveur n'en garde rien. La tâche 5 avait ajouté à `person` la
    date et les groupes de l'IdP de la dernière connexion, ce qui contredisait 005/K3
    (minimisation : émetteur, `sub` et nom affiché seuls). Décision : on garde **les
    groupes de l'IdP de la dernière connexion, sans date** (`person.last_login_groups`,
    écrasés à chaque connexion, effacés par la pseudonymisation) ; la date n'est pas
    gardée, parce qu'elle permettrait de suivre l'activité d'une personne. K3 est amendée
    en ce sens. La fiche dit « groupes de l'IdP lors de sa dernière connexion », sans
    horodatage ; une identité qui ne s'est jamais connectée n'en montre aucun. Points
    sensibles notés : ces noms de groupes révèlent l'organisation interne, ils ne sont
    servis qu'à un lecteur des droits (`core.rbac.admin` ou lecture des identités) et à
    la personne elle-même.
