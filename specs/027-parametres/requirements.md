---
status: implemented
updated: 2026-10-28
---

# 027 — Paramètres par déclarant et flux vus depuis une extrémité : exigences

Statut : **implémentée le 2026-10-28**, validée le même jour (P1 à P8 ; P1, P2, P4 arbitrées, P3, P6, P7
confirmées par l'utilisateur).

## Résumé

Deux retours de l'utilisateur du 2026-10-28, réunis parce qu'ils touchent tous deux
aux droits :

1. **Flux vus depuis une extrémité.** Une personne doit voir les flux qui partent de ses
   machines ou y arrivent, même quand elle ne lit pas l'autre extrémité. Aujourd'hui un
   flux se lit par `flow.read` sur son entité ou comme demandeur (REQ-011-17) : le
   responsable d'une machine ne voit pas les flux d'une autre équipe qui la touchent.
   L'extrémité qu'il ne lit pas est montrée par sa seule valeur (adresse ou préfixe),
   sans objet, sans nom ni propriété.
2. **Paramètres.** Les champs disponibles et leurs valeurs possibles (environnements,
   médias d'interface, services avec leurs couples transport/port, types de flux,
   axes, champs personnalisés…) se configurent dans un écran « Paramètres », sous des
   droits **par déclarant** : on peut confier les paramètres du module Politique (la
   matrice de flux) à une équipe de sécurité sans lui donner ceux du cœur. Aujourd'hui
   `core.property.admin`, global, couvre toute déclaration et toute valeur de liste
   fermée, y compris celles des modules.

Décisions : [`decisions.md`](decisions.md) (P1 à P8) ; 011 (REQ-011-15, 17, 25, 26 ;
F12), 010 (REQ-010-25), 003 (REQ-003-1, 2, 5, 6), 018 (REQ-018-9), 024 (M7) ; D33, D36
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)). Pas de compatibilité à
garder avant le gel : anciens noms de permission repris par la table des renommages,
sans alias.

## Vocabulaire

- **Déclarant** : le cœur (clés `core.*` et `custom.*`, REQ-003-1) ou un module (clés
  `<module>.*`, objets de son catalogue).
- **Paramètres d'un déclarant** : ses déclarations de propriété (attributs modifiables),
  les valeurs et libellés de ses listes fermées, et, pour un module, les objets de son
  catalogue qui définissent des valeurs possibles (Politique : axes, environnements,
  types de flux, services).
- **Extrémité hors périmètre** : extrémité d'un flux que l'appelant ne lit pas à la date.

## Exigences

### A. Flux vus depuis une extrémité

#### REQ-027-1 — Lecture par une extrémité
kartenn doit rendre un flux lisible à l'appelant qui détient `flow.read` (sur
n'importe quel périmètre) et lit, à la date demandée, au moins une de ses extrémités
sous la permission de lecture de celle-ci, en plus des lecteurs de REQ-011-17 (`flow.read`
sur l'entité du flux, demandeur) ; cela vaut pour la liste des flux, les flux d'un objet
(REQ-011-15), les non-conformités (REQ-011-25) et le calcul de « ouvert » du test d'une
connexion (REQ-011-26) (P1, P3).

Acceptation :
- Alice lit `srv-app-01` (interface `eth0`, `192.0.2.10`) mais pas le réseau
  `198.51.100.0/24` ; un flux `eth0` → `198.51.100.0/24`, `https`, d'une entité qu'elle
  ne lit pas : listé sur la fiche de `srv-app-01` (Sortants) et dans `/v1/flows`.
- Même flux en sens inverse : listé dans les Entrants de `srv-app-01`.
- Bob détient `flow.read` mais ne lit aucune des deux extrémités ni l'entité : même 404
  qu'un flux absent.
- Une personne sans `flow.read` : aucun flux, même si elle lit les deux extrémités.

#### REQ-027-2 — Données du flux et accès aux objets
kartenn doit servir un flux lisible **tel quel**, quelle que soit la voie de lecture :
ses extrémités (sorte, identifiant, adresse ou préfixe, et pour une interface les
adresses qu'elle porte à la date), son service, son état, sa synthèse et ses évaluations
enregistrées sont des données du flux, sans masquage ni réponse réduite selon le
lecteur. Les données d'un objet du référentiel (nom d'une interface ou d'une machine,
attributs, propriétés, fiche) ne viennent que de ses propres routes, sous sa permission
de lecture : la réponse d'un flux n'en embarque aucune (P2, amendée le 2026-10-28).

Acceptation :
- Le flux d'Alice ci-dessus : destination `{kind: "prefix", id: …, value:
  "198.51.100.0/24"}`, identique à ce que voit un lecteur de l'entité du flux ; le
  service, l'état, la synthèse, l'échéance, les évaluations sont visibles.
- Destination `eth1` d'une machine qu'Alice ne lit pas : `{kind: "interface", id: …,
  values: ["198.51.100.20", "2001:db8::20"]}` ; `GET` de l'interface ou de sa machine
  par Alice : même 404 qu'un objet absent.
- La réponse d'un flux ne contient ni nom d'interface ni nom de machine, pour aucun
  lecteur ; l'interface web les obtient par la lecture de l'objet et, quand elle est
  refusée, montre la valeur sans lien, avec « fiche non accessible » et la permission
  manquante.

#### REQ-027-3 — Écritures et filtres
kartenn doit garder les écritures d'un flux sous REQ-011-17 (la maintenance directe exige
la lecture des deux extrémités) ; un filtre par objet d'extrémité ne s'applique qu'à un
objet lisible, un filtre par valeur (préfixe et son sous-arbre, REQ-011-14) s'applique
à toutes les extrémités, dont la valeur est une donnée du flux.

Acceptation :
- Alice tente de prolonger le flux ci-dessus avec `flow.edit` sur son entité : même 404
  qu'un flux absent (REQ-011-17).
- `?endpoint=198.51.100.0/24` rend le flux à Alice ; `?endpoint=<id du réseau>` rend
  une page vide.

#### REQ-027-4 — Budgets
kartenn doit tenir les budgets de REQ-011-19 avec cette lecture : page filtrée de flux ou
de non-conformités < 150 ms au banc complet, pour un lecteur qui ne lit les flux que par
leurs extrémités.

### B. Paramètres par déclarant

#### REQ-027-5 — Permissions des paramètres
kartenn doit déclarer, pour le cœur et pour chaque module qui a des paramètres, les
permissions globales `<déclarant>.settings.edit`, `<déclarant>.settings.apply` et
`<déclarant>.settings.approval.self` (grammaire de la 024, M7 : toute modification est
un changeset), et exiger celle du déclarant pour toute écriture de ses paramètres (P4,
P5).

Acceptation :
- Une équipe de sécurité avec `policy.settings.edit` et `policy.settings.approval.self` :
  crée l'environnement `preprod`, le service `ldaps` (`tcp/636`) et la valeur d'axe
  `pci` ; ajouter la valeur `sfp28` au média d'interface (`machines.interface_media`)
  ou un champ `custom.contrat` est refusé, permission `machines.settings.edit` ou
  `core.settings.edit` nommée.
- `core.settings.edit` seule sur un changeset qui ajoute un environnement : refusé,
  `policy.settings.edit` nommée.
- `core.settings.edit` accordée sur le périmètre d'une entité : refusée comme toute
  permission globale.

#### REQ-027-6 — Ce que couvre chaque déclarant
kartenn doit ranger :
- sous `core.settings.*` : déclarer, faire évoluer ou clore un champ `custom.*` ; les
  attributs modifiables des déclarations `core.*` ; la sensibilité de **toute**
  propriété (P6) ; les valeurs et libellés des listes fermées `core.*` et `custom.*`
  (dont `core.external_system`) ;
- sous `<module>.settings.*` : les valeurs et libellés des listes fermées du module ;
  les libellés de ses déclarations ; les objets de son catalogue qui définissent des
  valeurs possibles — pour Politique : axes, environnements, types de flux, services
  (REQ-010-1, 2, 4, 5, 29, 30).

Les déclarations d'un module restent synchronisées depuis son code par l'auteur système
(003/P12) ; les règles de la politique (`policy.rule.admin`) ne sont pas des paramètres
(P7).

Acceptation :
- Rendre `machines.fqdn` sensible avec `machines.settings.*` seule : refusé,
  `core.settings.edit` nommée.
- Relibeller la valeur `wifi` de `machines.interface_media` avec `machines.settings.edit`
  et `.approval.self` : appliqué, changeset `update_enum_value` audité (REQ-018-9).

#### REQ-027-7 — Reprise des droits
kartenn doit reprendre, à la migration, les rôles qui citent les anciennes permissions :
`core.property.admin` → `core.settings.{edit,apply,approval.self}` et
`<module>.settings.{edit,apply,approval.self}` de chaque module livré ;
`policy.axis.admin`, `policy.environment.admin`, `policy.flow_type.admin`,
`policy.service.admin` → `policy.settings.{edit,apply,approval.self}` ; par la table des
renommages (`RENAMED_PERMISSIONS`), sans alias.

Acceptation :
- Un rôle personnalisé avec `core.property.admin` : après migration, il détient les trois
  actions sur `core.settings`, `machines.settings` et `policy.settings` (modules livrés
  qui ont des paramètres ; Flux n'en a pas) ; seconde exécution sans effet.
- Le test qui traque les anciens noms de permission couvre ces noms.

### C. Écran des paramètres

#### REQ-027-8 — Maquette E13 « Paramètres »
Le shell v2 doit offrir, dans le catalogue des maquettes de la 020, un écran
« Paramètres » (entrée de l'espace Administration) avec un onglet par déclarant (Cœur,
Machines, Politique…) qui liste ses paramètres : champs (clé, libellé, type, porteurs,
héritage, sensibilité), valeurs d'une liste fermée en tableau-formulaire (clé figée
après création, libellés FR/EN, période de validité, clôture refusée tant qu'un objet ou
une règle cite la valeur, avec la liste — D33), environnements, services avec leurs
couples transport/port (REQ-010-5), types de flux, axes ; sans la permission du
déclarant, l'onglet est en lecture seule, la permission manquante nommée par son code ;
toute modification va au brouillon et au changeset (E5, E4) (P8).

Acceptation :
- Vues : `coeur` (champs et une liste fermée ouverte), `machines` (médias d'interface :
  `ethernet`, `sfp`, `sfp28`, `wifi`…), `politique` (environnements, services, types de
  flux, axes), `politique-lecture` (même onglet sans `policy.settings.edit`), `cloture`
  (clôture d'une valeur citée refusée, liste des objets) ; captures aux trois classes de
  largeur ; axe sans violation ; clavier.

#### REQ-027-9 — Maquette E11, extrémité hors périmètre
La maquette E11 doit montrer, dans les Entrants et Sortants d'une machine et dans le
registre, un flux dont l'autre extrémité est hors périmètre (REQ-027-2) : ses valeurs et
son service comme pour tout flux, sans nom d'objet, sans lien ni panneau de l'objet, avec
« fiche non accessible » et la permission manquante.
