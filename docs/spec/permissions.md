<!-- Généré par kartenn-core (authz::catalogue_markdown) : ne pas modifier à la main. -->

# Catalogue des permissions

Permissions du noyau (spec 005, REQ-005-16). S'y ajoutent, par propriété déclarée,
`prop.<clé>.edit`, `prop.<clé>.apply` et `prop.<clé>.approval.self`, et `prop.<clé>.read`
pour une propriété sensible. Un rôle peut contenir `prop.*.<action>` (toute propriété
non sensible) et `*` (tout). « Global » : ne s'accorde que sur le périmètre global.

Toute modification passe par un changeset (spec 024, M7) : `edit` le crée et le
soumet, en attente ; `apply` applique ou rejette le changeset d'un autre ;
`approval.self` applique le sien, sauf quand une politique d'approbation le
retient. Une action compte un segment, ou deux pour `approval.self`. Une
permission d'une autre action (`core.role.admin`, `flow.request`) vaut édition
et application. Les paramètres (déclarations, listes fermées, catalogue d'un
module) relèvent de `<déclarant>.settings.*`, le déclarant lu dans la clé
(`custom.*` : le cœur) ; la sensibilité de toute propriété, de `core.settings.*`
(spec 027).

| Permission | Global | Description |
|---|---|---|
| `core.entity.read` | — | Lire les entités. |
| `core.entity.edit` | — | Créer, modifier ou clore une entité dans un changeset et le soumettre. |
| `core.entity.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche une entité. |
| `core.entity.approval.self` | — | Appliquer son propre changeset qui touche une entité. |
| `core.site.read` | — | Lire les sites. |
| `core.site.edit` | — | Créer, modifier ou clore un site dans un changeset et le soumettre. |
| `core.site.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un site. |
| `core.site.approval.self` | — | Appliquer son propre changeset qui touche un site. |
| `core.domain.read` | — | Lire les domaines d'adressage. |
| `core.domain.edit` | — | Créer, modifier ou clore un domaine d'adressage dans un changeset et le soumettre. |
| `core.domain.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un domaine d'adressage. |
| `core.domain.approval.self` | — | Appliquer son propre changeset qui touche un domaine d'adressage. |
| `core.prefix.read` | — | Lire les préfixes. |
| `core.prefix.edit` | — | Créer, modifier ou clore un préfixe dans un changeset et le soumettre. |
| `core.prefix.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un préfixe. |
| `core.prefix.approval.self` | — | Appliquer son propre changeset qui touche un préfixe. |
| `core.range.read` | — | Lire les plages. |
| `core.range.edit` | — | Créer, modifier ou clore une plage dans un changeset et le soumettre. |
| `core.range.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche une plage. |
| `core.range.approval.self` | — | Appliquer son propre changeset qui touche une plage. |
| `core.address.read` | — | Lire les adresses. |
| `core.address.edit` | — | Créer, modifier ou clore une adresse dans un changeset et le soumettre. |
| `core.address.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche une adresse. |
| `core.address.approval.self` | — | Appliquer son propre changeset qui touche une adresse. |
| `core.vlan_group.read` | — | Lire les groupes de VLAN. |
| `core.vlan_group.edit` | — | Créer, modifier ou clore un groupe de VLAN dans un changeset et le soumettre. |
| `core.vlan_group.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un groupe de VLAN. |
| `core.vlan_group.approval.self` | — | Appliquer son propre changeset qui touche un groupe de VLAN. |
| `core.vlan.read` | — | Lire les VLAN. |
| `core.vlan.edit` | — | Créer, modifier ou clore un VLAN dans un changeset et le soumettre. |
| `core.vlan.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un VLAN. |
| `core.vlan.approval.self` | — | Appliquer son propre changeset qui touche un VLAN. |
| `core.changeset.read` | oui | Lire les changesets, leurs opérations et leur différentiel. |
| `core.anomaly.acknowledge` | — | Acquitter, avec un motif, une anomalie d'un objet du périmètre. |
| `core.settings.edit` | oui | Modifier dans un changeset et soumettre les paramètres du cœur : champs `custom.*`, déclarations `core.*`, listes fermées `core.*` et `custom.*`, sensibilité de toute propriété. |
| `core.settings.apply` | oui | Appliquer ou rejeter le changeset d'un autre qui touche les paramètres du cœur. |
| `core.settings.approval.self` | oui | Appliquer son propre changeset qui touche les paramètres du cœur. |
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
| `machines.cluster.read` | — | Lire les clusters. |
| `machines.cluster.edit` | — | Créer, modifier ou clore un cluster dans un changeset et le soumettre. |
| `machines.cluster.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche un cluster. |
| `machines.cluster.approval.self` | — | Appliquer son propre changeset qui touche un cluster. |
| `machines.read` | — | Lire les machines. |
| `machines.edit` | — | Créer, modifier ou clore une machine dans un changeset et le soumettre. |
| `machines.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche une machine. |
| `machines.approval.self` | — | Appliquer son propre changeset qui touche une machine. |
| `machines.interface.read` | — | Lire les interfaces. |
| `machines.interface.edit` | — | Créer, modifier ou clore une interface dans un changeset et le soumettre. |
| `machines.interface.apply` | — | Appliquer ou rejeter le changeset d'un autre qui touche une interface. |
| `machines.interface.approval.self` | — | Appliquer son propre changeset qui touche une interface. |
| `machines.settings.edit` | oui | Modifier dans un changeset et soumettre les paramètres des machines : valeurs et libellés de leurs listes fermées, libellés de leurs déclarations. |
| `machines.settings.apply` | oui | Appliquer ou rejeter le changeset d'un autre qui touche les paramètres des machines. |
| `machines.settings.approval.self` | oui | Appliquer son propre changeset qui touche les paramètres des machines. |
| `policy.catalogue.read` | — | Lire la politique : axes, valeurs, types de flux, services, règles, motifs, conditions et signaux. |
| `policy.verdict.read` | — | Demander un verdict de conformité entre deux extrémités dont on détient la lecture. |
| `policy.flow.request` | — | Demander un flux pour une entité : verdict de toute paire, explication réduite aux valeurs, règles, motifs et conditions. |
| `policy.settings.edit` | oui | Modifier dans un changeset et soumettre les paramètres de la politique : axes et leurs valeurs, environnements, types de flux, services, listes fermées et libellés des déclarations de la politique. |
| `policy.settings.apply` | oui | Appliquer ou rejeter le changeset d'un autre qui touche les paramètres de la politique. |
| `policy.settings.approval.self` | oui | Appliquer son propre changeset qui touche les paramètres de la politique. |
| `policy.rule.admin` | oui | Créer, modifier ou clore une règle, un motif, une condition d'admissibilité, un signal de risque ou un rôle exempté. |
| `policy.inference.admin` | — | Écrire les règles d'inférence qui citent un domaine du périmètre. |
| `flow.read` | — | Lire les flux. |
| `flow.request` | — | Demander un flux sans lire ses extrémités, et lire ses propres demandes. |
| `flow.review` | — | Instruire une demande de flux. |
| `flow.waiver.grant` | — | Accorder une dérogation à un flux refusé. |
| `flow.edit` | — | Écrire directement un flux (maintenance) dans un changeset et le soumettre, avec la lecture de ses deux extrémités. |
| `flow.apply` | — | Appliquer ou rejeter le changeset d'un autre qui écrit directement un flux. |
| `flow.approval.self` | — | Appliquer son propre changeset qui écrit directement un flux. |

## Rôles livrés

| Rôle | Clé | Contenu |
|---|---|---|
| Lecteur | `reader` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.vlan_group.read`, `core.vlan.read`, `machines.cluster.read`, `machines.read`, `machines.interface.read`, `policy.catalogue.read`, `policy.verdict.read`, `flow.read` |
| Éditeur | `editor` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.vlan_group.read`, `core.vlan.read`, `core.entity.edit`, `core.site.edit`, `core.domain.edit`, `core.prefix.edit`, `core.range.edit`, `core.address.edit`, `core.vlan_group.edit`, `core.vlan.edit`, `prop.*.edit`, `core.entity.approval.self`, `core.site.approval.self`, `core.domain.approval.self`, `core.prefix.approval.self`, `core.range.approval.self`, `core.address.approval.self`, `core.vlan_group.approval.self`, `core.vlan.approval.self`, `prop.*.approval.self`, `machines.cluster.read`, `machines.read`, `machines.interface.read`, `policy.catalogue.read`, `policy.verdict.read`, `flow.read` |
| Approbateur | `approver` | `core.entity.read`, `core.site.read`, `core.domain.read`, `core.prefix.read`, `core.range.read`, `core.address.read`, `core.vlan_group.read`, `core.vlan.read`, `core.entity.apply`, `core.site.apply`, `core.domain.apply`, `core.prefix.apply`, `core.range.apply`, `core.address.apply`, `core.vlan_group.apply`, `core.vlan.apply`, `prop.*.apply`, `machines.cluster.read`, `machines.read`, `machines.interface.read`, `policy.catalogue.read`, `policy.verdict.read`, `flow.read`, `machines.cluster.apply`, `machines.apply`, `machines.interface.apply`, `flow.apply` |
| Administrateur | `administrator` | `*` |

## Anciens noms

Renommées par la spec 018 (R2, R3), remplacées par la spec 024 (REQ-024-9) ou
réparties par déclarant par la spec 027 (REQ-027-7). Un
ancien nom est refusé dans un rôle (`core.role.invalid`) ; il ne reste que dans
l'historique et le journal d'audit.

| Ancien nom | Nouveaux noms |
|---|---|
| `core.entity.write` | `core.entity.edit`, `core.entity.approval.self` |
| `core.entity.propose` | `core.entity.edit` |
| `core.entity.apply_own` | `core.entity.approval.self` |
| `core.site.write` | `core.site.edit`, `core.site.approval.self` |
| `core.site.propose` | `core.site.edit` |
| `core.site.apply_own` | `core.site.approval.self` |
| `core.domain.write` | `core.domain.edit`, `core.domain.approval.self` |
| `core.domain.propose` | `core.domain.edit` |
| `core.domain.apply_own` | `core.domain.approval.self` |
| `core.prefix.write` | `core.prefix.edit`, `core.prefix.approval.self` |
| `core.prefix.propose` | `core.prefix.edit` |
| `core.prefix.apply_own` | `core.prefix.approval.self` |
| `core.range.write` | `core.range.edit`, `core.range.approval.self` |
| `core.range.propose` | `core.range.edit` |
| `core.range.apply_own` | `core.range.approval.self` |
| `core.address.write` | `core.address.edit`, `core.address.approval.self` |
| `core.address.propose` | `core.address.edit` |
| `core.address.apply_own` | `core.address.approval.self` |
| `core.vlan_group.write` | `core.vlan_group.edit`, `core.vlan_group.approval.self` |
| `core.vlan_group.propose` | `core.vlan_group.edit` |
| `core.vlan_group.apply_own` | `core.vlan_group.approval.self` |
| `core.vlan.write` | `core.vlan.edit`, `core.vlan.approval.self` |
| `core.vlan.propose` | `core.vlan.edit` |
| `core.vlan.apply_own` | `core.vlan.approval.self` |
| `core.settings.write` | `core.settings.edit`, `core.settings.approval.self` |
| `core.settings.propose` | `core.settings.edit` |
| `core.settings.apply_own` | `core.settings.approval.self` |
| `machines.cluster.write` | `machines.cluster.edit`, `machines.cluster.approval.self` |
| `machines.cluster.propose` | `machines.cluster.edit` |
| `machines.cluster.apply_own` | `machines.cluster.approval.self` |
| `machines.write` | `machines.edit`, `machines.approval.self` |
| `machines.propose` | `machines.edit` |
| `machines.apply_own` | `machines.approval.self` |
| `machines.interface.write` | `machines.interface.edit`, `machines.interface.approval.self` |
| `machines.interface.propose` | `machines.interface.edit` |
| `machines.interface.apply_own` | `machines.interface.approval.self` |
| `machines.settings.write` | `machines.settings.edit`, `machines.settings.approval.self` |
| `machines.settings.propose` | `machines.settings.edit` |
| `machines.settings.apply_own` | `machines.settings.approval.self` |
| `policy.settings.write` | `policy.settings.edit`, `policy.settings.approval.self` |
| `policy.settings.propose` | `policy.settings.edit` |
| `policy.settings.apply_own` | `policy.settings.approval.self` |
| `flow.write` | `flow.edit`, `flow.approval.self` |
| `flow.propose` | `flow.edit` |
| `flow.apply_own` | `flow.approval.self` |
| `prop.*.write` | `prop.*.edit`, `prop.*.approval.self` |
| `prop.*.propose` | `prop.*.edit` |
| `prop.*.apply_own` | `prop.*.approval.self` |
| `vlan.vlan.read` | `core.vlan.read` |
| `vlan.vlan.write` | `core.vlan.edit`, `core.vlan.approval.self` |
| `vlan.vlan.propose` | `core.vlan.edit` |
| `vlan.read` | `core.vlan.read` |
| `vlan.write` | `core.vlan.edit`, `core.vlan.approval.self` |
| `vlan.propose` | `core.vlan.edit` |
| `vlan.edit` | `core.vlan.edit` |
| `vlan.apply` | `core.vlan.apply` |
| `vlan.apply_own` | `core.vlan.approval.self` |
| `vlan.approval.self` | `core.vlan.approval.self` |
| `vlan.group.read` | `core.vlan_group.read` |
| `vlan.group.write` | `core.vlan_group.edit`, `core.vlan_group.approval.self` |
| `vlan.group.propose` | `core.vlan_group.edit` |
| `vlan.group.edit` | `core.vlan_group.edit` |
| `vlan.group.apply` | `core.vlan_group.apply` |
| `vlan.group.apply_own` | `core.vlan_group.approval.self` |
| `vlan.group.approval.self` | `core.vlan_group.approval.self` |
| `prop.vlan.vlan.read` | `prop.core.vlan.read` |
| `prop.vlan.vlan.write` | `prop.core.vlan.edit`, `prop.core.vlan.approval.self` |
| `prop.vlan.vlan.propose` | `prop.core.vlan.edit` |
| `prop.vlan.vlan.edit` | `prop.core.vlan.edit` |
| `prop.vlan.vlan.apply` | `prop.core.vlan.apply` |
| `prop.vlan.vlan.apply_own` | `prop.core.vlan.approval.self` |
| `prop.vlan.vlan.approval.self` | `prop.core.vlan.approval.self` |
| `machines.machine.read` | `machines.read` |
| `machines.machine.write` | `machines.edit`, `machines.approval.self` |
| `machines.machine.propose` | `machines.edit` |
| `flow.flow.read` | `flow.read` |
| `flow.flow.request` | `flow.request` |
| `flow.flow.review` | `flow.review` |
| `flow.flow.write` | `flow.edit`, `flow.approval.self` |
| `flow.flow.waive` | `flow.waiver.grant` |
| `core.property.admin` | `core.settings.edit`, `core.settings.apply`, `core.settings.approval.self`, `machines.settings.edit`, `machines.settings.apply`, `machines.settings.approval.self`, `policy.settings.edit`, `policy.settings.apply`, `policy.settings.approval.self` |
| `policy.axis.admin` | `policy.settings.edit`, `policy.settings.apply`, `policy.settings.approval.self` |
| `policy.environment.admin` | `policy.settings.edit`, `policy.settings.apply`, `policy.settings.approval.self` |
| `policy.flow_type.admin` | `policy.settings.edit`, `policy.settings.apply`, `policy.settings.approval.self` |
| `policy.service.admin` | `policy.settings.edit`, `policy.settings.apply`, `policy.settings.approval.self` |
| `core.changeset.approve` | `core.entity.apply`, `core.site.apply`, `core.domain.apply`, `core.prefix.apply`, `core.range.apply`, `core.address.apply`, `core.vlan_group.apply`, `core.vlan.apply`, `machines.cluster.apply`, `machines.apply`, `machines.interface.apply`, `flow.apply`, `prop.*.apply` |
