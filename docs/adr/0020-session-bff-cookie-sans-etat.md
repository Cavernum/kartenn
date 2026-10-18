---
status: accepted
date: 2026-10-18
---

# 0020 — Session du navigateur : BFF à cookie chiffré sans état

## Context

Le shell Web (D17) charge des plugins UI tiers ; tout jeton lisible par le JavaScript
leur serait accessible. Les instances sont sans état (D9, ADR 0012). Les bonnes pratiques
OAuth pour les applications navigateur (RFC 10017 §6.1) recommandent fortement un
« backend for frontend » pour les applications métier. Décisions 005/K1, K10.

## Decision

Le serveur kartenn mène le flux OIDC par code (PKCE, `state`, `nonce`, `iss` de
RFC 9207) en client confidentiel. Le navigateur ne reçoit qu'un cookie `__Host-`,
`Secure`, `HttpOnly`, `SameSite=Strict`, chiffré et authentifié (AES-256-GCM, clé dérivée
d'un trousseau commun aux instances), qui porte l'identité, les groupes et le jeton de
rafraîchissement. Inactivité 30 min, durée absolue 8 h ; l'état de l'identité est relu à
chaque requête (cache de 5 s) pour permettre la révocation immédiate.

## Alternatives

- Client public dans le navigateur : jetons exposés à tout script, plugins compris.
- Session stockée côté serveur : contredit D9 ou exige un stockage partagé de sessions.
- Cookie sans contrôle d'état : pas de révocation avant expiration.

## Consequences

- Un trousseau de clés à distribuer à toutes les instances et à faire tourner.
- Taille du cookie bornée (4 Kio, découpage en trois au plus).
- Protection CSRF à la charge de la couche HTTP (006).
