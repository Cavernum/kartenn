---
status: accepted
date: 2026-10-13
---

# 0007 — Domaines d'adressage et arbre de préfixes déduit

## Context

Les SI agglomérés réutilisent les mêmes plages privées ; la validation doit rester
stricte. Décisions D4, D34 ([journal](../../specs/001-cadrage/decisions.md)) ; 002/A2, A3
([journal 002](../../specs/002-noyau-adressage/decisions.md)).

## Decision

- Toute donnée d'adressage appartient à un **domaine d'adressage** (VRF, espace de
  routage), immuable après création.
- L'unicité est **par domaine** pour l'espace non global, **globale** pour l'espace
  public (hors anycast).
- Un préfixe est **conteneur** ou **réseau** ; un réseau est une feuille ; plages et
  adresses appartiennent à exactement un réseau.
- Le parent d'un objet est **déduit** : plus petit préfixe englobant du domaine, à la
  date considérée. Rien n'est stocké comme vérité.

## Alternatives

- Espace unique : faux au premier rachat qui réutilise 10.0.0.0/8.
- VRF optionnelle façon NetBox : réintroduit des doublons tolérés.
- Parent explicite : peut diverger de la réalité CIDR.
- Préfixes indifférenciés : sous-réseau à deviner pour la capacité et l'héritage.

## Consequences

- Insérer ou retirer un préfixe intermédiaire change l'héritage : différentiel et droit
  sur l'effet obligatoires (ADR 0008, 0010).
- Les requêtes d'inclusion doivent être indexées (`inet_ops`).
