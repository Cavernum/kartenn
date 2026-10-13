# kartenn

*Kartenn* : « carte » en breton.

Référentiel réseau déclaratif pour les très grands systèmes d'information. Il sert à
cartographier, valider et maintenir les domaines d'adressage, les préfixes, les adresses,
les VLAN, les machines, les passerelles, les NAT…, puis, à terme, à vérifier la conformité
des flux réseau à une politique de sécurité (environnements et matrice de flux).

> **Statut : cadrage.** Il n'y a pas encore de code applicatif. Chaque brique est
> spécifiée et discutée avant d'être implémentée.

## Principes

- **Source de vérité déclarative et validation stricte** : aucune incohérence n'est acceptée en écriture.
- **Bitemporel** : l'état du réseau à une date donnée (temps applicatif) et ce que l'outil affirmait (temps système).
- **Changesets** : toute écriture est un ensemble d'opérations atomique, avec approbation possible.
- **Modulaire** : modules métier compilés (crates Rust), plugins UI chargés à chaud (Web Components), connecteurs hors processus.
- **Une seule frontière publique** : l'API REST. Web, CLI, TUI, MCP et connecteurs en sont des clients.
- **Accès** : OIDC (plusieurs émetteurs) et Basic Auth, RBAC par périmètre.

## Organisation du dépôt

| Chemin | Contenu |
|---|---|
| `crates/` | Workspace Rust : noyau, modules métier, serveur |
| `web/` | Shell Web (Vue 3 + TypeScript) et SDK de plugins |
| `docs/spec/` | Spécification système vivante |
| `docs/adr/` | Architecture Decision Records |
| `docs/reference/` | Notes de lecture des RFC et documentations officielles |
| `docs/practices/` | Bonnes pratiques (API, sécurité, exploitation) |
| `specs/` | Specs de fonctionnalités (`NNN-slug/`) |

Point d'entrée : [`specs/001-cadrage/decisions.md`](specs/001-cadrage/decisions.md).

## Environnement de développement

[devenv](https://devenv.sh) fournit Rust, Node.js 24, TypeScript et PostgreSQL 18 :

```sh
devenv shell   # outils
devenv up      # PostgreSQL local (base « kartenn »)
devenv test    # tests et hooks
```

## Licence

[AGPL-3.0](LICENSE).
