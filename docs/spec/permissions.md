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
| `policy.axis.read` | — | Lire les axes de la politique. |
| `policy.axis.write` | oui | Désigner, modifier ou retirer un axe de la politique. |
| `policy.axis.propose` | oui | Proposer un changement d'un axe de la politique. |
| `policy.value.read` | — | Lire les attributs des valeurs des axes. |
| `policy.value.write` | oui | Décrire une valeur d'axe, créer ou clore un environnement. |
| `policy.value.propose` | oui | Proposer un changement des attributs d'une valeur d'axe. |
| `policy.flow_type.read` | — | Lire les types de flux. |
| `policy.flow_type.write` | oui | Créer, modifier ou clore un type de flux. |
| `policy.flow_type.propose` | oui | Proposer un changement d'un type de flux. |
| `policy.service.read` | — | Lire le catalogue de services. |
| `policy.service.write` | oui | Créer, modifier ou clore un service. |
| `policy.service.propose` | oui | Proposer un changement d'un service. |
| `policy.pattern.read` | — | Lire les motifs d'architecture. |
| `policy.pattern.write` | oui | Créer, modifier ou clore un motif d'architecture. |
| `policy.pattern.propose` | oui | Proposer un changement d'un motif d'architecture. |
| `policy.rule.read` | — | Lire les règles de la politique. |
| `policy.rule.write` | oui | Créer, modifier ou clore une règle de la politique. |
| `policy.rule.propose` | oui | Proposer un changement d'une règle de la politique. |
| `policy.condition.read` | — | Lire les conditions d'admissibilité. |
| `policy.condition.write` | oui | Créer, modifier ou clore une condition d'admissibilité. |
| `policy.condition.propose` | oui | Proposer un changement d'une condition d'admissibilité. |
| `policy.signal.read` | — | Lire les signaux de risque. |
| `policy.signal.write` | oui | Créer, modifier ou clore un signal de risque. |
| `policy.signal.propose` | oui | Proposer un changement d'un signal de risque. |
| `policy.exempt_role.read` | — | Lire les rôles d'interface exemptés. |
| `policy.exempt_role.write` | oui | Exempter un rôle d'interface ou retirer l'exemption. |
| `policy.exempt_role.propose` | oui | Proposer une exemption d'un rôle d'interface. |
| `flow.flow.read` | — | Lire les flux. |
| `flow.flow.request` | — | Demander un flux sans lire ses extrémités, et lire ses propres demandes. |
| `flow.flow.review` | — | Instruire une demande de flux. |
| `flow.flow.waive` | — | Accorder une dérogation à un flux refusé. |
| `flow.flow.write` | — | Écrire directement un flux (maintenance), avec la lecture de ses deux extrémités. |

## Rôles livrés

| Rôle | Clé | Contenu |
|---|---|---|
| Lecteur | `reader` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `vlan.group.read`, `vlan.vlan.read`, `machines.cluster.read`, `machines.machine.read`, `machines.interface.read`, `policy.axis.read`, `policy.value.read`, `policy.flow_type.read`, `policy.service.read`, `policy.pattern.read`, `policy.rule.read`, `policy.condition.read`, `policy.signal.read`, `policy.exempt_role.read`, `flow.flow.read` |
| Éditeur | `editor` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.entity.write`, `core.site.write`, `core.domain.write`, `core.prefix.write`, `core.range.write`, `core.address.write`, `prop.*.write`, `vlan.group.read`, `vlan.vlan.read`, `machines.cluster.read`, `machines.machine.read`, `machines.interface.read`, `policy.axis.read`, `policy.value.read`, `policy.flow_type.read`, `policy.service.read`, `policy.pattern.read`, `policy.rule.read`, `policy.condition.read`, `policy.signal.read`, `policy.exempt_role.read`, `flow.flow.read` |
| Approbateur | `approver` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.changeset.approve`, `vlan.group.read`, `vlan.vlan.read`, `machines.cluster.read`, `machines.machine.read`, `machines.interface.read`, `policy.axis.read`, `policy.value.read`, `policy.flow_type.read`, `policy.service.read`, `policy.pattern.read`, `policy.rule.read`, `policy.condition.read`, `policy.signal.read`, `policy.exempt_role.read`, `flow.flow.read` |
| Administrateur | `administrator` | `*` |
