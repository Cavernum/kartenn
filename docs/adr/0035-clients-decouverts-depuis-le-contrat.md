---
status: proposed
date: 2026-10-23
---

# 0035 — Clients découverts depuis le contrat

## Context

La spec 023 livre une bibliothèque Python (`kartenn`) et une CLI (`kartenn-cli`) qui
n'ont d'autre accès au serveur que l'API REST (D7). Les modules d'une build (Politique,
Flux, VLAN, Machines, et ceux à venir) ajoutent leurs collections au contrat
`/v1/openapi.json` selon les conventions REST de l'ADR 0026 (clause 9) ; une build
peut en omettre. Un client figé par build (générateur de code) devrait suivre chaque
version du serveur et chaque jeu de modules ; une liste de modules connue du client
casserait la promesse d'un module livré sans nouvelle version du client (T2, T3).

D7 amendée (« SDK client dérivé de l'OpenAPI ») et les arbitrages A1, A5 de la 023
fixent l'approche ; cet ADR en consigne la mécanique.

## Decision

1. **Cœur écrit à la main, ressources découvertes.** La bibliothèque code à la main le
   seul socle transverse : capacités, contrat, `me`, changesets, imports,
   authentification. Toute autre ressource est **dérivée à l'exécution** du contrat servi
   et des capacités, en une passe (`kartenn.catalog.derive`) :
   - `/v1/<coll>` : `list`, `create`, `query` (extension `x-query`) ;
   - `/v1/<coll>/{id}` : `get`, `update` (Merge Patch sous `If-Match`), `close`, `set` ;
   - `/v1/<coll>/{id}/<sous>` : lecture en `GET`, action sinon ;
     `properties/{key}/explanation|suggestion` → `properties.explain|suggest` ;
   - module d'une opération = son premier tag ; module d'une ressource = celui de son
     opération de collection ; une sous-ressource servie par un autre module garde le
     sien (`/v1/addresses/{id}/flows`, tag `flow`) ;
   - extension facultative `x-kartenn` (A5) : clé naturelle et colonnes sur le
     `PathItem` de collection, nom de verbe (`command`) sur une opération. Sans elle, les
     conventions suffisent.
   Aucun nom de module ni de ressource n'est codé : un module inconnu de la bibliothèque
   (test `demo`) est listable, lisible et modifiable.

2. **Compatibilité par présence.** `api_versions` doit contenir `v1`
   (`client.api-unsupported` sinon). Une fonction du socle est détectée par la présence
   de son `operationId`, jamais par la version du serveur ; une absence lève
   `client.operation-unavailable` à l'usage.

3. **Cache par révision (T20).** Révision = `sha256(version, modules et leurs versions,
   ETag du contrat)`. Catalogue en JSON compact dans
   `$XDG_CACHE_HOME/kartenn/catalog/<sha256(url)>-<révision>.json`, pointeur
   `<sha256(url)>.json` (révision, capacités, date de lecture) ; fichiers 0600 écrits par
   renommage atomique, répertoires 0700. Les capacités sont relues au plus une fois par
   minute ; même version et mêmes modules : le contrat est redemandé sous
   `If-None-Match` s'il porte un ETag (304, rien de téléchargé), pas du tout sinon.
   Un fichier abîmé vaut un fichier absent.

4. **Port de transport (T5).** Trois opérations, synchrones et asynchrones : `send`,
   `stream`, `sse`, sur des messages propres au cœur. L'adaptateur par défaut repose sur
   httpx2 ; c'est le seul module qui l'importe. Un adaptateur HTTP/3 futur
   (`kartenn[http3]`) implémenterait le même port, sous un ADR à ce moment.

5. **Logique écrite une fois.** Les appels sont des *plans* : des générateurs qui
   émettent des requêtes et reçoivent des réponses, exécutés par le client synchrone ou
   asynchrone. Le cœur ne fait aucune entrée-sortie.

## Alternatives

- Client généré au build (openapi-python-client) : figé par build, ni `QUERY`, ni
  changesets, ni imports ; épingle une version ancienne de httpx (T2, a).
- Bindings PyO3 sur une crate Rust (T2, c) : matrice de wheels, aucun gain pour un client
  limité par le réseau.
- Point d'accès de description propre aux clients : une seconde surface à tenir à côté du
  contrat (T3, rejeté).
- Liste des modules dans le client : chaque module exigerait une nouvelle version.

## Consequences

- Un module livré côté serveur apparaît dans la bibliothèque et dans la CLI sans
  nouvelle version. Un module qui s'écarte des conventions est mal nommé ou absent : le
  test de couverture de la CLI le voit sur le contrat de référence, et `x-kartenn`
  corrige sans code client.
- Le premier contact télécharge le contrat (environ 830 Kio pour la build complète au
  2026-10-23) ; les suivants ne lisent que les capacités, au plus une fois par minute.
- Les stubs de types (`python -m kartenn stubs generate`) sont produits pour un serveur
  donné et restent facultatifs (T4).
- L'ETag de `/v1/openapi.json` (tâche 4 de la 023) rend la détection d'un contrat changé
  à version égale ; sans lui, la clé repose sur la version et les modules.
