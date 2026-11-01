# Liste de parité : ancien shell → nouveau shell

Spec 021, REQ-021-31, tâche 26 (design § « Liste de parité ») : chaque route, écran,
action, point d'extension et parcours de l'ancien shell (`web/`) et de ses plugins, avec
son équivalent dans `web/v2/`, son parcours porté et son état. La bascule (tâche 27) exige
une liste entièrement cochée. Tenue à jour par chaque tâche de P2 et P3 et par chaque
correctif de l'ancien shell qui touche un comportement.

Cette première version ne tient que l'administration (tâche 15a) ; la tâche 26 la complète
pour les autres zones, d'après `web/shell/src/router.ts`, `web/e2e/journeys/*.spec.ts` et
`web/plugins/*/public/manifest.json`.

États : `[x]` porté, parcours vert ; `[ ]` à porter (tâche entre parenthèses) ; `[-]`
retiré par une spec (référence).

## Administration

| État | Ancien                                                                           | Nouveau                                                                                                                                                                                          | Parcours porté                                 |
| ---- | -------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------- |
| [x]  | `/admin` (sections selon les droits)                                             | `/admin` : espace `admin` de la barre latérale, entrées selon les droits ; la route mène à la première visible                                                                                   | `admin` (« a reader reaches no screen »)       |
| [x]  | `/admin/roles` : liste, rôle défini (`PUT /v1/roles/{key}`), rôle clos           | `/admin/roles`, `/admin/roles/:key` (panneau droit) : créer, modifier, dupliquer, supprimer, permissions ajoutées et retirées, au brouillon ; permissions en nom servi et libellé ; groupes liés | `admin` (« a role »)                           |
| [-]  | `/admin/bindings` : liaisons posées et closes                                    | retiré par la 025 (REQ-025-12, G4) : les liaisons se gèrent depuis leur groupe, `/admin/groups/:id`                                                                                              | `admin` (« a group »)                          |
| [x]  | — (025)                                                                          | `/admin/groups`, `/admin/groups/:id` : groupes, membres (identités, groupes de l'IdP), rôles sur périmètres, au brouillon ; refus par permission, passé, groupe livré, périmètre plus large      | `admin` (« a group », « the delivered group ») |
| [ ]  | `/admin/issuers`                                                                 | `/admin/issuers` (15b)                                                                                                                                                                           | `admin` (15b)                                  |
| [ ]  | `/admin/service-accounts`, `/admin/service-accounts/:id`, secret montré une fois | `/admin/accounts` (15b)                                                                                                                                                                          | `admin` (15b)                                  |
| [ ]  | `/admin/identities`, `/admin/identities/:id`, pseudonymisation                   | `/admin/identities` (15b)                                                                                                                                                                        | `admin` (15b)                                  |
| [ ]  | `/admin/audit` : recherche, export NDJSON                                        | `/admin/audit` (15c)                                                                                                                                                                             | `admin` (15c)                                  |
| [ ]  | `/imports`, `/imports/:id` : suivi, reprise, annulation, tri en échec            | `/imports` (15c)                                                                                                                                                                                 | `admin` (15c)                                  |
| [ ]  | parcours `web/e2e/journeys/admin.spec.ts`                                        | `web/v2/e2e/journeys/admin.spec.ts` : groupes et rôles (15a) ; comptes, secret, identités (15b) ; audit, imports (15c)                                                                           | —                                              |
