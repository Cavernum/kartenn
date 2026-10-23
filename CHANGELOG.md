# Journal des changements

Toutes les évolutions notables de kartenn. Format inspiré de
[Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) ; versions selon
[SemVer 2.0.0](https://semver.org/lang/fr/). Chaque version publiée porte une étiquette
annotée et signée `vX.Y.Z` dont le message reprend sa section (ADR 0033).

Les sections des versions sont écrites par `k-release` : les faits marquants rédigés à la
main sous « Non publié », puis la liste des Conventional Commits rendue par git-cliff
(`cliff.toml`). Une section publiée ne se modifie plus.

## [Non publié]

## [0.1.0] — à publier

Première version : le référentiel réseau déclaratif, son API et son interface (D31).

### Ajouté

- **Noyau d'adressage** (002) : familles IPv4 et IPv6 extensibles, entités, sites,
  domaines d'adressage, arbre de préfixes déduit de l'inclusion, plages, adresses,
  occupation et prochain libre, anomalies, identifiants externes.
- **Propriétés héritables** (003) : types de valeurs, listes fermées, héritage le long des
  arbres, résolution matérialisée et expliquée, obligations, sources de valeurs attendues.
- **Changesets et bitemporel** (004) : écritures atomiques simulées puis appliquées,
  différentiel, propositions et approbations, annulation, lectures à une date de validité
  et à une date système, outbox ordonnée par domaine.
- **Authentification et RBAC** (005) : session BFF OIDC à cookie chiffré, jetons
  d'accès, comptes de service, comptes de secours à phrase de passe et TOTP, rôles et
  liaisons par périmètre évalués en SQL.
- **API REST** (006) : contrat `/v1` OpenAPI 3.1, lectures datées, curseurs signés,
  filtres et `QUERY`, Merge Patch et `If-Match`, `Idempotency-Key`, imports par
  morceaux, Problem Details, quotas et délestage.
- **Shell Web et plugins** (007) : interface Vue 3 servie par le serveur, plugins
  chargés à chaud par import map avec SRI et CSP stricte, SDK MPL-2.0, contexte temporel,
  brouillon local simulé, écrans du noyau, des changesets et de l'administration, FR et
  EN, WCAG 2.2 AA.
- **Contrat de module et VLAN** (008) : modules métier compilés (ADR 0026), groupes de
  VLAN et VLAN 802.1Q, lien VLAN des réseaux, plugin VLAN.
- **Machines et interfaces** (009) : clusters, machines, interfaces, modes 802.1Q,
  rattachements d'adresses, port de lecture pour la politique de flux, plugin machines.
- **Journalisation et audit** (012) : journal d'audit scellé en base et exporté, logs
  techniques sur plusieurs sorties, OpenTelemetry.
- **Outillage** : émetteur OIDC de développement, un binaire de tests par crate
  (ADR 0027), bases de test clonées d'un gabarit (ADR 0028), bancs de budgets du serveur
  et du navigateur.
