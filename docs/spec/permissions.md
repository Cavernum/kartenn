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
| `core.identity.pseudonymise` | oui | Pseudonymiser sans délai les données d'identité d'une personne. |
| `core.identity.act_on_behalf` | oui | Pour un compte de service : écrire pour le compte d'un demandeur nommé par l'en-tête Kartenn-On-Behalf-Of, sous ses propres droits. |
| `core.observability.admin` | oui | Changer à chaud le filtre d'une sortie technique. |
| `vlan.group.read` | — | Lire les groupes de VLAN. |
| `vlan.group.write` | — | Créer, modifier ou clore un groupe de VLAN. |
| `vlan.group.propose` | — | Proposer un changement d'un groupe de VLAN. |
| `vlan.vlan.read` | — | Lire les VLAN. |
| `vlan.vlan.write` | — | Créer, modifier ou clore un VLAN. |
| `vlan.vlan.propose` | — | Proposer un changement d'un VLAN. |
| `machines.cluster.read` | — | Lire les clusters. |
| `machines.cluster.write` | — | Créer, modifier ou clore un cluster. |
| `machines.cluster.propose` | — | Proposer un changement d'un cluster. |
| `machines.machine.read` | — | Lire les machines. |
| `machines.machine.write` | — | Créer, modifier ou clore une machine. |
| `machines.machine.propose` | — | Proposer un changement d'une machine. |
| `machines.interface.read` | — | Lire les interfaces. |
| `machines.interface.write` | — | Créer, modifier ou clore une interface. |
| `machines.interface.propose` | — | Proposer un changement d'une interface. |
| `policy.catalogue.read` | — | Lire la politique : axes, valeurs, types de flux, services, règles, motifs, conditions et signaux. |
| `policy.verdict.read` | — | Demander un verdict de conformité entre deux extrémités dont on détient la lecture. |
| `policy.flow.request` | — | Demander un flux pour une entité : verdict de toute paire, explication réduite aux valeurs, règles, motifs et conditions. |
| `policy.axis.admin` | oui | Désigner, modifier ou retirer un axe, décrire ses valeurs. |
| `policy.environment.admin` | oui | Créer, décrire ou clore un environnement. |
| `policy.flow_type.admin` | oui | Créer, modifier ou clore un type de flux. |
| `policy.service.admin` | oui | Créer, modifier ou clore un service. |
| `policy.rule.admin` | oui | Créer, modifier ou clore une règle, un motif, une condition d'admissibilité, un signal de risque ou un rôle exempté. |
| `policy.inference.admin` | — | Écrire les règles d'inférence qui citent un domaine du périmètre. |

## Rôles livrés

| Rôle | Clé | Contenu |
|---|---|---|
| Lecteur | `reader` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `vlan.group.read`, `vlan.vlan.read`, `machines.cluster.read`, `machines.machine.read`, `machines.interface.read`, `policy.catalogue.read`, `policy.verdict.read` |
| Éditeur | `editor` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.entity.write`, `core.site.write`, `core.domain.write`, `core.prefix.write`, `core.range.write`, `core.address.write`, `prop.*.write`, `vlan.group.read`, `vlan.vlan.read`, `machines.cluster.read`, `machines.machine.read`, `machines.interface.read`, `policy.catalogue.read`, `policy.verdict.read` |
| Approbateur | `approver` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.changeset.approve`, `vlan.group.read`, `vlan.vlan.read`, `machines.cluster.read`, `machines.machine.read`, `machines.interface.read`, `policy.catalogue.read`, `policy.verdict.read` |
| Administrateur | `administrator` | `*` |
