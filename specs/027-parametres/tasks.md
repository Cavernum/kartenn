# 027 — Paramètres par déclarant et flux vus depuis une extrémité : tâches

Statut : spec **validée le 2026-10-28**.

- [x] 1. Maquettes : E13 « Paramètres » (vues `coeur`, `machines`, `politique`,
  `politique-lecture`, `cloture`) et extrémité hors périmètre dans E11 ; ligne E13 du
  tableau des maquettes de la 020 — REQ-027-8, 9 — verify : `k-web2-check`,
  `k-web2-e2e ecrans clavier touch layout`, captures
  - où : `web/v2/shell/src/_ui/ecrans/E7Administration.vue` (espace Administration,
    permissions nommées), `E3Adresses.vue` (tableau-formulaire), `E11Flux.vue` (vue
    `machine`), `registry.ts`, `sidebar.ts` ; `specs/020-systeme-de-design/design.md`
    (tableau des maquettes)
  - fait : `E13Parametres.vue` (entrée « Paramètres » de l'espace Administration, un
    onglet `Tabs` par déclarant, cinq vues) ; E11 : deux flux dont l'autre extrémité
    n'est pas lue (préfixe `203.0.113.128/25`, interface aux adresses `198.51.100.20`,
    `2001:db8::20`), dans le registre et les Entrants/Sortants ; lignes E11 et E13 de la
    020 ; tests clavier E13 et E11 ; captures aux largeurs du banc.
  - choix : sans masquage (consigne du 2026-10-28, remplace REQ-027-2 tel qu'écrit) :
    sorte et valeurs de l'extrémité montrées, ni nom tiré de l'objet, ni lien ni
    panneau, « fiche non accessible » et la permission (`machines.read`,
    `core.prefix.read`) ; préfixe `203.0.113.128/25` plutôt que `198.51.100.0/24`, qui
    contient une adresse de `srv-app-01` dans les fixtures ; une personne par vue
    (droits `*.settings.edit` détenus), les onglets sans le droit en lecture seule ;
    refus de clôture codé `policy.reference.closed` ; couples `icmp type 8 code 0`,
    `ip proto 50` ; layout.spec ignore une barre non rendue (sélecteur de colonnes en
    `display: none`).
- [ ] 2. Amendements : REQ-011-17 (lecteurs), REQ-011-15, 25, 26 (renvoi à la 027),
  F12 ; REQ-003-1 et 005 (catalogue des permissions) ; REQ-010-1, 2, 4, 5, 29
  (permissions) ; `specs/README.md`
- [ ] 3. Lecture d'un flux par une extrémité et extrémité hors périmètre — REQ-027-1 à
  4 — verify : tests d'acceptation ; `k-check` ; banc `bench-flow` complet (agent
  principal)
  - où : `crates/kartenn-flow/src/rest.rs` (`visible`, `page_of`, flux d'un objet,
    non-conformités, `/v1/flow-checks`) ; `kartenn_store_pg::rbac::readable` ;
    `crates/kartenn-flow/tests/it/rest.rs` (tests de droits à imiter) ;
    `crates/kartenn-bench` (lecteur par extrémités)
- [ ] 4. Permissions `<déclarant>.settings.*`, contrôle par déclarant, migration des
  rôles — REQ-027-5 à 7 — verify : tests d'acceptation ; test des anciens noms ;
  `k-check`
  - où : `crates/kartenn-core/src/authz.rs` (`PROPERTY_ADMIN`, `RENAMED_PERMISSIONS`),
    `crates/kartenn-service/src/rights.rs:206` (`ObjectKind::Property | EnumValue`),
    `crates/kartenn-policy/src/module.rs` (permissions du catalogue) ;
    migration 0071 `approval_self` à imiter
- [ ] 5. Clients, SDK v1 et v2, CLI, `docs/spec/` (permissions, interfaces) — verify :
  `k-py-check`, `k-web-check`, `k-web2-check`
