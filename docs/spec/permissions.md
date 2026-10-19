<!-- Généré par kartenn-core (authz::catalogue_markdown) : ne pas modifier à la main. -->

# Catalogue des permissions

Permissions du noyau (spec 005, REQ-005-16). S'y ajoutent, par propriété déclarée,
`prop.<clé>.write` et `prop.<clé>.propose`, et `prop.<clé>.read` pour une propriété
sensible. Un rôle peut contenir `prop.*.<action>` (toute propriété non sensible) et
`*` (tout). « Global » : ne s'accorde que sur le périmètre global.

| Permission | Global | Description |
|---|---|---|
| `core.entity.read` | — | Lire les entités. |
| `core.entity.write` | — | Créer, modifier ou clore une entité. |
| `core.entity.propose` | — | Proposer un changement d'entité. |
| `core.site.read` | — | Lire les sites. |
| `core.site.write` | — | Créer, modifier ou clore un site. |
| `core.site.propose` | — | Proposer un changement de site. |
| `core.domain.read` | — | Lire les domaines d'adressage. |
| `core.domain.write` | — | Créer, modifier ou clore un domaine d'adressage. |
| `core.domain.propose` | — | Proposer un changement de domaine d'adressage. |
| `core.prefix.read` | — | Lire les préfixes. |
| `core.prefix.write` | — | Créer, modifier ou clore un préfixe. |
| `core.prefix.propose` | — | Proposer un changement de préfixe. |
| `core.range.read` | — | Lire les plages. |
| `core.range.write` | — | Créer, modifier ou clore une plage. |
| `core.range.propose` | — | Proposer un changement de plage. |
| `core.address.read` | — | Lire les adresses. |
| `core.address.write` | — | Créer, modifier ou clore une adresse. |
| `core.address.propose` | — | Proposer un changement d'adresse. |
| `core.changeset.approve` | — | Valider un changeset en attente qui touche le périmètre. |
| `core.changeset.read` | oui | Lire les changesets, leurs opérations et leur différentiel. |
| `core.anomaly.acknowledge` | — | Acquitter, avec un motif, une anomalie d'un objet du périmètre. |
| `core.property.admin` | oui | Déclarer une propriété, changer sa sensibilité. |
| `core.approval_policy.admin` | — | Créer, modifier ou désactiver une politique d'approbation. |
| `core.rbac.read` | oui | Lire les rôles, liaisons, rattachements, comptes de service et l'état des identités. |
| `core.rbac.admin` | — | Poser ou clore des liaisons dans le périmètre, pour des rôles dont on détient les permissions. |
| `core.role.admin` | oui | Définir, modifier ou clore un rôle. |
| `core.issuer.admin` | oui | Rattacher un émetteur OIDC à des entités ou au périmètre global. |
| `core.account.admin` | oui | Gérer les comptes de service, désactiver une identité, révoquer ses sessions. |
| `core.audit.read` | — | Consulter ou exporter le journal d'audit. |
| `core.observability.admin` | oui | Changer à chaud le filtre d'une sortie technique. |

## Rôles livrés

| Rôle | Clé | Contenu |
|---|---|---|
| Lecteur | `reader` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read` |
| Éditeur | `editor` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.entity.write`, `core.site.write`, `core.domain.write`, `core.prefix.write`, `core.range.write`, `core.address.write`, `prop.*.write` |
| Approbateur | `approver` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.changeset.approve` |
| Administrateur | `administrator` | `*` |
