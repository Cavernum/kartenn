# 002 — Notes d'implémentation

Écarts et questions découverts en implémentant, à trancher avant de finir la spec.

## I1 — Contradiction : anycast entre domaines (REQ-002-25, 32, 46) — **décision requise**

- REQ-002-25 : toute adresse appartient à un réseau de son domaine.
- REQ-002-32 : les réseaux publics sont uniques entre domaines.
- REQ-002-46 : une adresse publique anycast peut figurer dans plusieurs domaines ; son
  critère d'acceptation précise qu'« un réseau public dupliqué reste refusé même s'il ne
  contient que des adresses anycast ».

Conséquence : une adresse anycast ne peut **jamais** exister dans deux domaines (il lui
faudrait un réseau dans chacun, et ce réseau est refusé). La logique de A11 au niveau des
adresses est implémentée (`kartenn-service/src/global.rs`), mais elle est inatteignable.

Proposition : étendre A11 aux **réseaux** : un réseau public peut être marqué anycast et
figurer dans plusieurs domaines si toutes ses occurrences le sont (avertissement listant les
domaines) ; ses adresses sont alors anycast. Alternative : renoncer à REQ-002-46.

## I2 — Plages en attente de l'amendement du prototype 001

Table `ip_range`, règles REQ-002-21 à 24 et 28, clé naturelle des plages : en attente de la
validation de l'amendement (type `inetrange` sans `subtype_diff`).

## I3 — Opérations pas encore exposées par le pipeline

Le pipeline (004-7) sait créer domaines, préfixes et adresses et clore un objet. Les
opérations de modification (changement de nature, redimensionnement, changement d'usage)
viendront avec le découpage en base (004-8) ; les règles REQ-002-17 et 18 seront branchées
à ce moment-là.

## I4 — Règles globales : faits fournis par le stockage

Les règles globales sont évaluées par le service (`global.rs`) à partir de faits demandés
au port de stockage dans la transaction, sur l'état final : réseaux englobants, préfixes
contenus, couverture temporelle d'une adresse (`range_agg` des périodes des réseaux),
réseaux et adresses publics des autres domaines. Une requête par objet touché : à mesurer
pour les imports (banc 004-23).
