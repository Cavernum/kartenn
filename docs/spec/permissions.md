<!-- Généré par kartenn-core (authz::catalogue_markdown) : ne pas modifier à la main. -->

# Catalogue des permissions

Permissions du noyau (spec 005, REQ-005-16). S'y ajoutent, par propriété déclarée,
`prop.<clé>.edit`, `prop.<clé>.apply` et `prop.<clé>.apply_own`, et `prop.<clé>.read`
pour une propriété sensible. Un rôle peut contenir `prop.*.<action>` (toute propriété
non sensible) et `*` (tout). « Global » : ne s'accorde que sur le périmètre global.

Toute modification passe par un changeset (spec 024, M7) : `edit` le crée et le
soumet, en attente ; `apply` applique ou rejette le changeset d'un autre ;
`apply_own` applique le sien, sauf quand une politique d'approbation le retient.
Une permission d'une autre action (`core.role.admin`, `flow.request`) vaut
édition et application.

| Permission | Global | Description |
|---|---|---|
| `core.entity.read` | — | Lire les entités. |
| `core.entity.edit` | — | Créer, modifier ou clore une entité dans un changeset et le soumettre. |
| `core.entity.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche une entité. |
| `core.entity.apply_own` | — | Appliquer son propre changeset qui touche une entité. |
| `core.site.read` | — | Lire les sites. |
| `core.site.edit` | — | Créer, modifier ou clore un site dans un changeset et le soumettre. |
| `core.site.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un site. |
| `core.site.apply_own` | — | Appliquer son propre changeset qui touche un site. |
| `core.domain.read` | — | Lire les domaines d'adressage. |
| `core.domain.edit` | — | Créer, modifier ou clore un domaine d'adressage dans un changeset et le soumettre. |
| `core.domain.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un domaine d'adressage. |
| `core.domain.apply_own` | — | Appliquer son propre changeset qui touche un domaine d'adressage. |
| `core.prefix.read` | — | Lire les préfixes. |
| `core.prefix.edit` | — | Créer, modifier ou clore un préfixe dans un changeset et le soumettre. |
| `core.prefix.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un préfixe. |
| `core.prefix.apply_own` | — | Appliquer son propre changeset qui touche un préfixe. |
| `core.range.read` | — | Lire les plages. |
| `core.range.edit` | — | Créer, modifier ou clore une plage dans un changeset et le soumettre. |
| `core.range.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche une plage. |
| `core.range.apply_own` | — | Appliquer son propre changeset qui touche une plage. |
| `core.address.read` | — | Lire les adresses. |
| `core.address.edit` | — | Créer, modifier ou clore une adresse dans un changeset et le soumettre. |
| `core.address.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche une adresse. |
| `core.address.apply_own` | — | Appliquer son propre changeset qui touche une adresse. |
| `core.changeset.read` | oui | Lire les changesets, leurs opérations et leur différentiel. |
| `core.anomaly.acknowledge` | — | Acquitter, avec un motif, une anomalie d'un objet du périmètre. |
| `core.property.admin` | oui | Déclarer une propriété, changer sa sensibilité. |
| `core.approval_policy.admin` | — | Créer, modifier ou désactiver une politique d'approbation. |
| `core.rbac.read` | oui | Lire les rôles, groupes, membres, liaisons, rattachements, comptes de service et l'état des identités. |
| `core.rbac.admin` | — | Créer des groupes, poser ou clore leurs liaisons et leurs membres dans le périmètre, pour des rôles dont on détient les permissions. |
| `core.role.admin` | oui | Définir, modifier ou clore un rôle. |
| `core.issuer.admin` | oui | Rattacher un émetteur OIDC à des entités ou au périmètre global. |
| `core.account.admin` | oui | Gérer les comptes de service, désactiver une identité, révoquer ses sessions. |
| `core.audit.read` | — | Consulter ou exporter le journal d'audit. |
| `core.identity.pseudonymise` | oui | Pseudonymiser sans délai les données d'identité d'une personne. |
| `core.identity.act_on_behalf` | oui | Pour un compte de service : écrire pour le compte d'un demandeur nommé par l'en-tête Kartenn-On-Behalf-Of, sous ses propres droits. |
| `core.observability.admin` | oui | Changer à chaud le filtre d'une sortie technique. |
| `core.draft.admin` | oui | Lire, supprimer ou attribuer à un autre le brouillon d'une personne. |
| `vlan.group.read` | — | Lire les groupes de VLAN. |
| `vlan.group.edit` | — | Créer, modifier ou clore un groupe de VLAN dans un changeset et le soumettre. |
| `vlan.group.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un groupe de VLAN. |
| `vlan.group.apply_own` | — | Appliquer son propre changeset qui touche un groupe de VLAN. |
| `vlan.read` | — | Lire les VLAN. |
| `vlan.edit` | — | Créer, modifier ou clore un VLAN dans un changeset et le soumettre. |
| `vlan.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un VLAN. |
| `vlan.apply_own` | — | Appliquer son propre changeset qui touche un VLAN. |
| `machines.cluster.read` | — | Lire les clusters. |
| `machines.cluster.edit` | — | Créer, modifier ou clore un cluster dans un changeset et le soumettre. |
| `machines.cluster.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un cluster. |
| `machines.cluster.apply_own` | — | Appliquer son propre changeset qui touche un cluster. |
| `machines.read` | — | Lire les machines. |
| `machines.edit` | — | Créer, modifier ou clore une machine dans un changeset et le soumettre. |
| `machines.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche une machine. |
| `machines.apply_own` | — | Appliquer son propre changeset qui touche une machine. |
| `machines.interface.read` | — | Lire les interfaces. |
| `machines.interface.edit` | — | Créer, modifier ou clore une interface dans un changeset et le soumettre. |
| `machines.interface.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche une interface. |
| `machines.interface.apply_own` | — | Appliquer son propre changeset qui touche une interface. |
| `policy.catalogue.read` | — | Lire la politique : axes, valeurs, types de flux, services, règles, motifs, conditions et signaux. |
| `policy.verdict.read` | — | Demander un verdict de conformité entre deux extrémités dont on détient la lecture. |
| `policy.flow.request` | — | Demander un flux pour une entité : verdict de toute paire, explication réduite aux valeurs, règles, motifs et conditions. |
| `policy.axis.admin` | oui | Désigner, modifier ou retirer un axe, décrire ses valeurs. |
| `policy.environment.admin` | oui | Créer, décrire ou clore un environnement. |
| `policy.flow_type.admin` | oui | Créer, modifier ou clore un type de flux. |
| `policy.service.admin` | oui | Créer, modifier ou clore un service. |
| `policy.rule.admin` | oui | Créer, modifier ou clore une règle, un motif, une condition d'admissibilité, un signal de risque ou un rôle exempté. |
| `policy.inference.admin` | — | Écrire les règles d'inférence qui citent un domaine du périmètre. |
| `flow.read` | — | Lire les flux. |
| `flow.request` | — | Demander un flux sans lire ses extrémités, et lire ses propres demandes. |
| `flow.review` | — | Instruire une demande de flux. |
| `flow.waiver.grant` | — | Accorder une dérogation à un flux refusé. |
| `flow.edit` | — | Écrire directement un flux (maintenance) dans un changeset et le soumettre, avec la lecture de ses deux extrémités. |
| `flow.apply` | — | Appliquer ou rejeter le changeset d'un autre qui écrit directement un flux. |
| `flow.apply_own` | — | Appliquer son propre changeset qui écrit directement un flux. |

## Rôles livrés

| Rôle | Clé | Contenu |
|---|---|---|
| Lecteur | `reader` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `vlan.group.read`, `vlan.read`, `machines.cluster.read`, `machines.read`, `machines.interface.read`, `policy.catalogue.read`, `policy.verdict.read`, `flow.read` |
| Éditeur | `editor` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.entity.edit`, `core.site.edit`, `core.domain.edit`, `core.prefix.edit`, `core.range.edit`, `core.address.edit`, `prop.*.edit`, `core.entity.apply_own`, `core.site.apply_own`, `core.domain.apply_own`, `core.prefix.apply_own`, `core.range.apply_own`, `core.address.apply_own`, `prop.*.apply_own`, `vlan.group.read`, `vlan.read`, `machines.cluster.read`, `machines.read`, `machines.interface.read`, `policy.catalogue.read`, `policy.verdict.read`, `flow.read` |
| Approbateur | `approver` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.entity.apply`, `core.site.apply`, `core.domain.apply`, `core.prefix.apply`, `core.range.apply`, `core.address.apply`, `prop.*.apply`, `vlan.group.read`, `vlan.read`, `machines.cluster.read`, `machines.read`, `machines.interface.read`, `policy.catalogue.read`, `policy.verdict.read`, `flow.read`, `vlan.group.apply`, `vlan.apply`, `machines.cluster.apply`, `machines.apply`, `machines.interface.apply`, `flow.apply` |
| Administrateur | `administrator` | `*` |

## Anciens noms

Renommées par la spec 018 (R2, R3) ou remplacées par la spec 024 (REQ-024-9). Un
ancien nom est refusé dans un rôle (`core.role.invalid`) ; il ne reste que dans
l'historique et le journal d'audit.

| Ancien nom | Nouveaux noms |
|---|---|
| `core.entity.write` | `core.entity.edit`, `core.entity.apply_own` |
| `core.entity.propose` | `core.entity.edit` |
| `core.site.write` | `core.site.edit`, `core.site.apply_own` |
| `core.site.propose` | `core.site.edit` |
| `core.domain.write` | `core.domain.edit`, `core.domain.apply_own` |
| `core.domain.propose` | `core.domain.edit` |
| `core.prefix.write` | `core.prefix.edit`, `core.prefix.apply_own` |
| `core.prefix.propose` | `core.prefix.edit` |
| `core.range.write` | `core.range.edit`, `core.range.apply_own` |
| `core.range.propose` | `core.range.edit` |
| `core.address.write` | `core.address.edit`, `core.address.apply_own` |
| `core.address.propose` | `core.address.edit` |
| `vlan.group.write` | `vlan.group.edit`, `vlan.group.apply_own` |
| `vlan.group.propose` | `vlan.group.edit` |
| `vlan.write` | `vlan.edit`, `vlan.apply_own` |
| `vlan.propose` | `vlan.edit` |
| `machines.cluster.write` | `machines.cluster.edit`, `machines.cluster.apply_own` |
| `machines.cluster.propose` | `machines.cluster.edit` |
| `machines.write` | `machines.edit`, `machines.apply_own` |
| `machines.propose` | `machines.edit` |
| `machines.interface.write` | `machines.interface.edit`, `machines.interface.apply_own` |
| `machines.interface.propose` | `machines.interface.edit` |
| `flow.write` | `flow.edit`, `flow.apply_own` |
| `flow.propose` | `flow.edit` |
| `prop.*.write` | `prop.*.edit`, `prop.*.apply_own` |
| `prop.*.propose` | `prop.*.edit` |
| `vlan.vlan.read` | `vlan.read` |
| `vlan.vlan.write` | `vlan.edit`, `vlan.apply_own` |
| `vlan.vlan.propose` | `vlan.edit` |
| `machines.machine.read` | `machines.read` |
| `machines.machine.write` | `machines.edit`, `machines.apply_own` |
| `machines.machine.propose` | `machines.edit` |
| `flow.flow.read` | `flow.read` |
| `flow.flow.request` | `flow.request` |
| `flow.flow.review` | `flow.review` |
| `flow.flow.write` | `flow.edit`, `flow.apply_own` |
| `flow.flow.waive` | `flow.waiver.grant` |
| `core.changeset.approve` | `core.entity.apply`, `core.site.apply`, `core.domain.apply`, `core.prefix.apply`, `core.range.apply`, `core.address.apply`, `vlan.group.apply`, `vlan.apply`, `machines.cluster.apply`, `machines.apply`, `machines.interface.apply`, `flow.apply`, `prop.*.apply` |
