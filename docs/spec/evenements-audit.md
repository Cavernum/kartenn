<!-- Généré par kartenn-core (audit::catalogue_markdown) : ne pas modifier à la main. -->

# Catalogue des événements d'audit

Noms stables (spec 012, REQ-012-5, 30). La sévérité suit la table de REQ-012-4 ; la
criticité fixe la durée de conservation (L22). Les correspondances OCSF (1.9) et ECS
(9) sont indicatives et s'appliquent à l'export.

| Événement | Sévérité | Criticité | Description | Détails | Exigences | OCSF | ECS |
|---|---|---|---|---|---|---|---|
| `authn.login.succeeded` | INFO | normal | Une identité s'est authentifiée. | `method` | REQ-012-8 | Authentication 3002 / Logon | `authentication` |
| `authn.login.failed` | WARN | high | Une authentification a échoué ; l'identifiant revendiqué n'est conservé qu'en empreinte. | `method` | REQ-012-8, REQ-012-9 | Authentication 3002 / Logon | `authentication` |
| `authn.login.failures_aggregated` | WARN | high | Échecs répétés d'une même source agrégés sur une minute, avec leur nombre exact. | `count`, `window_seconds` | REQ-012-19 | Authentication 3002 / Logon | `authentication` |
| `authn.account.locked` | WARN | high | Un compte a été verrouillé après des échecs répétés. | — | REQ-012-8 | Account Change 3001 / Lock | `iam` |
| `authn.token.reused` | WARN | high | Un jeton déjà utilisé ou révoqué a été présenté. | — | REQ-012-8 | Authentication 3002 / Logon | `authentication` |
| `authn.session.closed` | INFO | low | Une session s'est terminée (déconnexion ou expiration). | `cause` | REQ-012-8 | Authentication 3002 / Logoff | `authentication` |
| `authn.breakglass.used` | WARN | high | Un compte de secours a ouvert une session. | `method` | REQ-005-15 | Authentication 3002 / Logon | `authentication` |
| `authz.access.denied` | WARN | high | Un droit a été refusé à l'acteur sur les objets cibles. | `permission` | REQ-012-8 | Authorize Session 3003 / Assign Privileges | `iam` |
| `changeset.state.pending` | INFO | normal | Un changeset attend une approbation ou une validation de proposition. | `kind` | REQ-012-8 | API Activity 6003 / Create | `configuration` |
| `changeset.state.applied` | INFO | normal | Un changeset a été appliqué. | `operations` | REQ-012-8 | API Activity 6003 / Update | `configuration` |
| `changeset.state.refused` | INFO | normal | Un changeset a été refusé par la validation. | `rules` | REQ-012-8 | API Activity 6003 / Update | `configuration` |
| `changeset.state.withdrawn` | INFO | normal | Son auteur a retiré un changeset en attente. | — | REQ-012-8 | API Activity 6003 / Delete | `configuration` |
| `changeset.state.rejected` | INFO | normal | Un valideur a rejeté un changeset en attente. | — | REQ-012-8 | API Activity 6003 / Update | `configuration` |
| `changeset.state.expired` | INFO | normal | Un changeset en attente a expiré. | — | REQ-012-8 | API Activity 6003 / Update | `configuration` |
| `changeset.state.stale` | WARN | normal | Un changeset en attente n'a pas passé sa revalidation au moment d'être validé. | `rules` | REQ-012-8 | API Activity 6003 / Update | `configuration` |
| `anomaly.state.acknowledged` | INFO | normal | Une anomalie a été acquittée avec un motif, pour la version courante de son objet. | `rule`, `object` | REQ-002-36 | API Activity 6003 / Update | `configuration` |
| `rbac.role.changed` | INFO | high | Un rôle ou ses permissions ont changé. | `role` | REQ-012-8 | Entity Management 3004 / Update | `iam` |
| `rbac.binding.changed` | INFO | high | Une liaison sujet × rôle × périmètre a été créée ou retirée. | `role`, `scope` | REQ-012-8 | Entity Management 3004 / Update | `iam` |
| `account.service.changed` | INFO | high | Un compte de service a été créé, renouvelé ou révoqué. | `change` | REQ-012-8 | Account Change 3001 / Create | `iam` |
| `account.breakglass.created` | WARN | high | Un compte de secours a été créé, hors de l'API, par le propriétaire du schéma, avec le rôle d'administration global jusqu'à sa fin de validité (K17). | `valid_until`, `role`, `scope` | REQ-005-14 | Account Change 3001 / Create | `iam` |
| `policy.approval.changed` | INFO | high | Une politique d'approbation a été créée, modifiée ou désactivée. | `enabled` | REQ-012-8 | Entity Management 3004 / Update | `configuration` |
| `property.declaration.changed` | INFO | normal | Une déclaration de propriété a été créée ou modifiée. | `key`, `sensitive` | REQ-012-8, REQ-003-27 | Entity Management 3004 / Update | `configuration` |
| `person.identity.pseudonymised` | INFO | high | Les données d'identité d'une personne ont été pseudonymisées. | `motive` | REQ-012-8 | Account Change 3001 / Other | `iam` |
| `data.objects.read` | INFO | low | Une requête de lecture a renvoyé ces objets (jamais leurs valeurs). | `criteria`, `targets_omitted` | REQ-012-18 | API Activity 6003 / Read | `database` |
| `data.sensitive.read` | INFO | high | La valeur d'une propriété sensible a été lue. | `property` | REQ-012-18 | API Activity 6003 / Read | `database` |
| `data.objects.exported` | INFO | high | Un export en masse a été produit. | `criteria`, `count` | REQ-012-8 | API Activity 6003 / Read | `database` |
| `audit.events.read` | INFO | high | Le journal d'audit a été consulté ou exporté. | `criteria`, `count` | REQ-012-17 | API Activity 6003 / Read | `database` |
| `sys.instance.started` | INFO | normal | Une instance a démarré. | `version` | REQ-012-8 | Application Lifecycle 6002 / Start | `process` |
| `sys.instance.stopped` | INFO | normal | Une instance s'est arrêtée. | `lost_technical_events` | REQ-012-8, REQ-012-34 | Application Lifecycle 6002 / Stop | `process` |
| `sys.schema.migrated` | INFO | normal | Le job de migration a été exécuté. | `version` | REQ-012-8 | Application Lifecycle 6002 / Update | `configuration` |
| `sys.config.changed` | INFO | high | Un réglage a changé. | `setting` | REQ-012-8 | Application Lifecycle 6002 / Update | `configuration` |
| `sys.log_filter.changed` | INFO | high | Le filtre d'une sortie technique a été modifié à chaud. | `output`, `filter`, `expires_at` | REQ-012-27 | Application Lifecycle 6002 / Update | `configuration` |
| `sys.output.unavailable` | WARN | high | Une sortie est inaccessible depuis plus d'une minute. | `output` | REQ-012-26 | Application Lifecycle 6002 / Other | `process` |
| `sys.output.available` | INFO | normal | Une sortie inaccessible est revenue. | `output` | REQ-012-26 | Application Lifecycle 6002 / Other | `process` |
| `sys.output.disabled` | WARN | high | Une sortie a été désactivée. | `output` | REQ-012-26 | Application Lifecycle 6002 / Other | `process` |
| `audit.seal.created` | INFO | normal | Un lot d'événements d'audit a été scellé. | `seal`, `count`, `hash`, `key_id` | REQ-012-13 | Application Lifecycle 6002 / Other | `configuration` |
| `audit.chain.broken` | ERROR | high | La vérification de la chaîne des sceaux a trouvé un lot altéré. | `seal` | REQ-012-14 | Detection Finding 2004 / Create | `intrusion_detection` |
| `abuse.rate.exceeded` | WARN | normal | Une limite de débit a été dépassée, refus agrégés par minute et par identité ou adresse. | `limit`, `count`, `window_seconds` | REQ-012-8, REQ-006-29 | API Activity 6003 / Other | `web` |
| `abuse.input.rejected` | WARN | normal | Une entrée a été refusée par la validation à la frontière. | `reason` | REQ-012-8 | API Activity 6003 / Other | `web` |
