---
status: accepted
date: 2026-10-13
---

# 0009 — OIDC multi-émetteurs et Basic Auth

## Context

Des opérateurs externes (prestataires, autres sociétés du groupe) doivent s'authentifier
sans être dans l'IdP de l'organisation ; l'automatisation a besoin d'un mécanisme simple.
Décision D13 ([journal](../../specs/001-cadrage/decisions.md)).

## Decision

Humains : OIDC (code + PKCE), **plusieurs émetteurs de confiance** configurés, claims
mappés sur le RBAC. Machines : comptes de service en Basic sur TLS, secrets Argon2id.
Comptes humains locaux en Basic pour le secours seulement. Les instances valident les
jetons localement (JWKS en cache).

## Alternatives

- Un seul IdP fédéré : dépendance forte à la capacité de l'IdP central à accueillir des
  externes.
- Comptes locaux pour les externes : cycle de vie des mots de passe, pas de MFA.

## Consequences

- Gestion par émetteur : JWKS, audiences, correspondance des claims.
- Une même personne via deux émetteurs = deux identités (question ouverte Q-ouv-5).

## Amendement — 2026-10-18 (spec 005)

- Secrets des comptes de service : générés par kartenn (256 bits) et stockés en
  empreinte HMAC-SHA-256 sous le trousseau, non en Argon2id : un secret aléatoire de
  cette taille ne se devine pas, et la vérification reste sous la milliseconde
  (005/K8, amendement de D13).
- Secours : phrase de passe (Argon2id) et code TOTP, connexion par une route dédiée et
  non en Basic (005/K9).
- Navigateur : BFF à cookie chiffré sans état (ADR 0020) ; la validation locale des
  jetons vaut pour les jetons d'accès des clients non navigateurs (RFC 9068).
