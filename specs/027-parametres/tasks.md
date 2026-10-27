# 027 — Paramètres par déclarant et flux vus depuis une extrémité : tâches

Statut : spec **validée le 2026-10-28**.

- [ ] 1. Maquettes : E13 « Paramètres » (vues `coeur`, `machines`, `politique`,
  `politique-lecture`, `cloture`) et extrémité hors périmètre dans E11 ; ligne E13 du
  tableau des maquettes de la 020 — REQ-027-8, 9 — verify : `k-web2-check`,
  `k-web2-e2e ecrans clavier touch layout`, captures
  - où : `web/v2/shell/src/_ui/ecrans/E7Administration.vue` (espace Administration,
    permissions nommées), `E3Adresses.vue` (tableau-formulaire), `E11Flux.vue` (vue
    `machine`), `registry.ts`, `sidebar.ts` ; `specs/020-systeme-de-design/design.md`
    (tableau des maquettes)
- [ ] 2. Amendements : REQ-011-17 (lecteurs), REQ-011-15, 25, 26 (renvoi à la 027),
  F12 ; REQ-003-1 et 005 (catalogue des permissions) ; REQ-010-1, 2, 4, 5, 29
  (permissions) ; `specs/README.md`
  - fait (partie 011, avec la tâche 3) : REQ-011-17 (lecteurs), REQ-011-15, 25, 26
    (renvoi à la 027), F12 et sa ligne de synthèse ; reste la 003, la 005, la 010 et
    `specs/README.md`.
- [x] 3. Lecture d'un flux par une extrémité et extrémité hors périmètre — REQ-027-1 à
  4 — verify : tests d'acceptation ; `k-check` ; banc `bench-flow` complet (agent
  principal)
  - fait : `flows_seen` (pages de flux et d'objet, test d'une connexion),
    `through_endpoints` (non-conformités) et `access` (lecture d'un flux, évaluations)
    dans `crates/kartenn-flow/src/rest.rs` ; lisibilité des extrémités d'une page en
    une requête (`endpoint_reads`, prédicat du cœur pour adresse, préfixe, plage, type
    Machines pour interface et machine), seulement pour les lignes que ni l'entité ni le
    demandeur ne rendent lisibles ; filtre `endpoint=<adresse>[/<longueur>]`
    (`FlowFilter::within_value`) ; membres `source`, `destination` (`FlowEndpointValue`)
    au contrat, client TS régénéré ; tests `acceptance_of_req_027_1` à `4`
    (`tests/it/by_endpoint.rs`) ; deux mesures « reader by endpoints » dans
    `bench-flow` (lecteur `flow.read` sur une entité sans flux, lecture du conteneur
    chaud par un périmètre de préfixe), non lancées.
  - choix : l'extrémité par valeur n'apparaît que pour un flux lu par une extrémité ;
    un lecteur de l'entité ou le demandeur garde `fields` inchangé (sans membre
    `source`/`destination`), pour ne pas changer leur lecture ni payer la requête.
  - choix : un filtre par objet non lu (`endpoint=<sorte>:<id>`, `prefix`, `machine`,
    flux d'un objet) écarte seulement les flux lus par une extrémité ; un lecteur de
    l'entité garde ses filtres.
  - choix : le filtre par valeur couvre adresses et préfixes contenus (`<<=`), tous
    domaines ; une demande en attente n'y répond pas (comme `prefix`).
  - choix : une interface (ou une machine) hors périmètre montre ses adresses
    attachées à `valid_at`, triées ; valeurs lues dans l'état courant (comme le
    prédicat de droits), pas à `as_of`.
  - choix : les demandes en attente (F19) ne se lisent pas par une extrémité.
  - choix : libellé de `flow.read` du catalogue inchangé (fichier généré, touché par
    la tâche 4).
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
