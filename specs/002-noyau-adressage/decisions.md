# 002 — Noyau d'adressage : journal des décisions

Interrogatoire ciblé avant `requirements.md`. Contexte : décisions D1–D37
([`../001-cadrage/decisions.md`](../001-cadrage/decisions.md)) et notes de
[`docs/reference/`](../../docs/reference/README.md).

## A1 — Périmètre de la spec
- Décision : **ancres et adressage** : entités et sites (D12), domaines d'adressage,
  préfixes, plages, adresses, contrat de famille d'adresses (IPv4, IPv6) et
  classification par les registres IANA à usage spécial. Hors périmètre : changesets et
  bitemporel (004), propriétés héritables (003), RBAC (005), conventions d'API (006).
- Pourquoi : un domaine référence une entité propriétaire ; les specs 003 à 006
  s'appuient sur ces objets et leurs règles de validité.
- Rejeté : adressage seul (dépendance croisée avec les ancres), familles seules (aucun
  invariant entre objets).
- Décidé par : recommandation acceptée

## A2 — Conteneurs et réseaux
- Décision : un préfixe est soit un **conteneur** (agrégat, supernet : ne contient que des
  préfixes), soit un **réseau** (sous-réseau de niveau 3 : contient plages et adresses ;
  portera VLAN et passerelle dans des modules ultérieurs). Invariants : un réseau ne
  contient ni réseau ni conteneur ; toute adresse et toute plage appartiennent à
  **exactement un réseau** de leur domaine (pas d'adresse orpheline).
- Pourquoi : capacité (réseau, diffusion, /31, /127) et héritage par adresse (D22)
  non ambigus ; validation stricte (D1).
- Conséquence : changer la nature d'un préfixe est une opération validée comme les
  autres (un conteneur ne devient réseau que s'il ne contient aucun préfixe).
- Rejeté : préfixes indifférenciés (sous-réseau à deviner), statut libre (aucun invariant).
- Décidé par : recommandation acceptée

## A3 — Unicité de l'adressage public
- Décision : unicité **par domaine** pour l'espace non global (RFC 1918, ULA,
  100.64.0.0/10 et tout bloc du registre à usage spécial dont Globally Reachable est
  faux) ; unicité **tous domaines confondus** pour l'espace public (bloc absent des
  registres à usage spécial, ou marqué Globally Reachable) : deux réseaux ou adresses
  publics qui se chevauchent dans deux domaines sont refusés. Exception explicite et
  justifiée pour l'anycast.
- Pourquoi : une adresse publique est unique sur Internet ; un doublon est une erreur
  de saisie ou un NAT à modéliser comme tel (v0.3).
- Amende D4 : le domaine « global » n'est pas obligatoire pour l'adressage public ; une
  DMZ publique reste dans le domaine où elle est routée.
- Rejeté : unicité par domaine seulement (doublons publics silencieux), domaine Internet
  unique (DMZ dans deux domaines).
- Décidé par : recommandation acceptée

## A4 — Sévérités des règles de validation
- Décision : deux niveaux **fixés par la spec**, pas par configuration.
  - **Erreur** (changeset refusé) : adresse de réseau ou de diffusion IPv4 attribuée
    dans un réseau de longueur ≤ 30 ; IID nul (anycast Subnet-Router) attribué hors /127
    point à point ; adresse prise dans la boucle locale, 0.0.0.0/8, 240.0.0.0/4,
    255.255.255.255, `::`, `::1` ou un bloc multicast comme adresse d'hôte ; préfixe ou
    adresse IPv4-mapped.
  - **Avertissement** (changeset accepté, anomalie renvoyée et consultable) : bloc de
    documentation, 100.64.0.0/10, réseau IPv6 plus long qu'un /64 hors /127 et /128 point
    à point, ULA au Global ID manifestement non aléatoire.
- Chaque règle a un **identifiant stable** ; un avertissement peut être **acquitté**
  avec un motif (tracé, et il n'est plus remonté pour cet objet).
- Rejeté : tout bloquant (bloque des usages légitimes), sévérités configurables
  (validité dépendante de l'instance, imports non reproductibles).
- Décidé par : recommandation acceptée

## A5 — Usage plutôt que statut
- Décision : aucun statut temporel (planifiée, active, dépréciée…) : la période de
  validité (D10) est la seule source de vérité sur le temps. Une adresse porte un
  **usage** en liste fermée : hôte, passerelle, VIP ou anycast, réservée. Une plage porte
  un usage : DHCP, réservée, statique. Les usages alimentent des validations (une
  adresse réservée ne peut pas être attachée à une interface, module 009).
- Une adresse reste **unique dans son domaine** (A3 pour le public) : le partage d'une
  VIP entre machines se modélise par plusieurs attachements à une même adresse
  (module 009), pas par des doublons.
- Rejeté : statut classique (doublon contradictoire avec la période de validité),
  aucun attribut.
- Décidé par : recommandation acceptée

## A6 — Nom DNS des adresses
- Décision : champ optionnel `dns_name` sur l'adresse, validé strictement : nom d'hôte
  LDH (RFC 1123 §2.1), labels de 63 caractères au plus, 253 au total, forme canonique
  en minuscules sans point final, noms internationalisés stockés en A-label. Pas
  d'unicité imposée (round-robin), avertissement en cas de doublon dans un domaine.
  Le futur module DNS reprendra ce champ comme source.
- À faire : note de référence sur les noms (RFC 1123, RFC 1035, RFC 5890–5891 IDNA)
  avant la conception.
- Rejeté : libellé libre (valeurs invalides propagées), rien dans le noyau (règles
  d'inférence v0.2 sans nom).
- Décidé par : recommandation acceptée

## A7 — Propriété et rattachement aux sites
- Décision : les **entités** forment un arbre (groupe > filiales > départements…) ; les
  **sites** aussi (région > site > bâtiment…). Un domaine a une entité propriétaire
  obligatoire. Un préfixe a une entité propriétaire **optionnelle**, sinon héritée du
  préfixe parent puis du domaine ; une plage ou une adresse hérite de son réseau. Un
  **réseau** peut être rattaché à un site ; un conteneur non (il peut couvrir plusieurs
  sites). La résolution du propriétaire est expliquée (valeur, origine, chaîne).
- Pourquoi : un prestataire possède un sous-arbre routé dans notre domaine (D3) ; c'est
  l'ancre du RBAC (D36).
- À trancher en conception : le propriétaire hérité réutilise-t-il le mécanisme des
  propriétés héritables (spec 003) ou est-ce un cas particulier du noyau ?
- Rejeté : entité explicite partout (redondance, incohérences), entité au domaine
  seulement (un prestataire n'est pas un domaine).
- Décidé par : recommandation acceptée

## A8 — Plages et adresses
- Décision : les plages d'un réseau ne se chevauchent jamais (D34). Règles par usage :
  - **DHCP** : aucune adresse individuelle, sauf avec l'usage « réservée » (réservation DHCP) ;
  - **réservée** : aucune adresse individuelle ;
  - **statique** : seulement des adresses individuelles.
  Une plage ne contient jamais l'adresse de réseau ni de diffusion (IPv4 ≤ /30), ni
  l'IID nul (IPv6 hors /127). L'occupation d'un réseau compte une plage DHCP ou réservée
  comme entièrement occupée.
- Rejeté : plages informatives (statique au milieu d'un pool sans alerte), plages hors
  noyau (D34 sans objet).
- Décidé par : recommandation acceptée

## A9 — Identifiants
- Décision : chaque objet a un **UUID immuable** (v7). Entités, sites et domaines ont en
  plus un **slug** unique dans l'instance. Les objets d'adressage ont une **clé
  naturelle** : (domaine, préfixe), (domaine, début–fin de plage), (domaine, adresse).
  L'API accepte UUID ou clé naturelle. Les identifiants externes des connecteurs sont
  une correspondance (source, identifiant externe) → UUID.
- Rejeté : UUID seul (imports et CLI illisibles), clés naturelles seules (renommer ou
  corriger change l'identité, casse l'historique).
- Décidé par : recommandation acceptée
