# 026 — Le VLAN dans le cœur : décisions

## C1 — Le VLAN est un objet du cœur (amende D18)
- Constat (2026-10-27) : l'utilisateur estime que le support des VLAN doit faire partie
  du cœur.
- Options : (A) cœur complet : tables, routes, règles, écrans ; (B) module toujours
  compilé, écrans dans le shell ; (C) module optionnel mieux intégré à l'interface.
- Décision : **A**. D18 devient « noyau (dont le VLAN) + Machines et interfaces ».
- Pourquoi : notion de base d'un référentiel réseau ; Machines en dépendait déjà ;
  supprime les cas « VLAN absent » dans les modules et les écrans.
- Rejeté : B (le VLAN resterait hors des règles du cœur), C (D18 inchangée, mais deux
  vérités sur ce qui est « de base »).
- Décidé par : l'utilisateur (2026-10-27, arbitrage « Cœur complet »).

## C2 — Code à sa place dans le cœur
- Décision : le modèle et les règles du VLAN rejoignent `kartenn-core`, le stockage
  `kartenn-store-pg` (migrations du cœur), les opérations le pipeline du cœur
  (`kartenn-service`), les routes `kartenn-rest` ; la crate `kartenn-vlan` disparaît.
- Pourquoi : un objet du cœur ne passe pas par `module_object` ni par le registre ; les
  imiter (préfixe, adresse) garde un seul chemin.
- Décidé par : Claude, conséquence de C1 ; découpage réversible tant que rien n'est gelé.

## C3 — Noms (REQ-026-2)
- Décision : permissions `core.vlan_group.{read,edit,apply,approval.self}` et
  `core.vlan.{…}` ; propriété `core.vlan` ; codes `core.vlan.*` selon la grammaire des
  codes du cœur ; routes inchangées (`/v1/vlan-groups`, `/v1/vlans`, déjà sous `/v1`).
- Pourquoi : un seul espace de noms pour le cœur ; les routes ne changent pas, seule la
  provenance.
- Décidé par : Claude (choix peu coûteux, contrat non gelé, pas d'alias).

## C4 — Migration des données
- Décision : une migration du cœur déplace les tables du schéma `vlan` vers le schéma du
  cœur (mêmes colonnes, versions et périodes ; `ALTER TABLE … SET SCHEMA`, pas de
  copie), retire l'inscription des objets dans `module_object` et l'entrée du registre ;
  la propriété, les rôles et le journal des changesets sont renommés comme pour
  `approval.self` (migration de rôles par changeset système, réécriture des codes dans
  le journal, audit laissé historique).
- Pourquoi : garder l'historique bitemporel intact ; même méthode éprouvée que 024-1 et
  la migration 0071.
- Décidé par : Claude.

## C5 — Machines, Politique, Flux
- Décision : ils lisent le VLAN par le port de lecture du cœur ; Machines perd sa
  dépendance de module à `vlan`.
- Décidé par : Claude, conséquence de C1.

## C6 — Interface
- Décision : shell v2 : écrans du cœur (maquette retirée d'E10, nouvelle maquette dans
  la liste de la 020) ; shell v1 : le plugin `vlan` reste servi tel quel, toujours actif,
  tant que v1 n'est pas remplacé (pas de travail d'écran sur une interface en fin de
  vie).
- Pourquoi : v1 est remplacé par la 021 ; refaire ses écrans serait perdu.
- Décidé par : Claude (réversible).

## C7 — Spec 008
- Décision : la 008 garde la partie A (contrat de module) comme référence de ce contrat ;
  ses parties B à F (VLAN) sont marquées « reprises par la 026 » sans être réécrites.
- Décidé par : Claude.
