---
status: accepted
date: 2026-10-23
---

# 0040 — Dictionnaire de libellés

## Context

Jusqu'à la v0.2, kartenn sert du texte dans une seule langue et sans règle commune : un
`label` nullable sur les rôles et les objets de la politique, une colonne
`policy_value.label` pour les valeurs d'axes, des descriptions de permissions et des
résumés de règles en français dans le code, des titres de Problem Details en anglais, et
rien pour les valeurs de listes fermées, les déclarations de propriétés, les sortes ou
les états (spec 018, « État des lieux »). Les clients (shell web, bibliothèque et CLI
Python) recopient leurs propres catalogues de codes. 007/W11 avait écarté toute
traduction côté serveur.

Les retours d'usage de la v0.2 (spec 018) demandent une clé stable toujours visible et un
libellé par langue (R7), pour tout ce qui en a un. Deux contraintes pèsent sur la forme :
un objet doit garder un seul corps et un seul ETag (006/H19, ADR 0022), et un libellé
renommé doit se lire à une date comme le reste du référentiel (D10, D11).

Arbitrages de l'utilisateur du 2026-10-23 : Q-018-4 (dictionnaire séparé, objets neutres
en langue), Q-018-5 (libellés versionnés avec leur objet), Q-018-6 (colonne `labels` sur
la table propriétaire), Q-018-7 (le noyau porte les libellés de toute valeur de liste
fermée). Décisions R19 à R24 de la 018. Sources : note
[`langues-negociation.md`](../reference/langues-negociation.md).

## Decision

1. **Corps neutres en langue (R20).** Un objet qui cite une valeur porte sa clé seule.
   Un objet qui possède des libellés les porte en table neutre `labels: {<langue>:
   <texte>}`, lue et réécrite avec lui sous `If-Match`. Aucun champ `label` résolu dans
   une langue ; aucune autre route que le dictionnaire ne varie selon la langue.

2. **Deux origines (R8).**
   - *Données administrables* : colonne `labels jsonb NOT NULL DEFAULT '{}'` sur chaque
     table propriétaire (déclarations et valeurs de propriétés, rôles, objets de la
     politique), versionnée avec la ligne, écrite par changeset sous le droit d'écriture
     de l'objet. Règle `core.label.invalid` : étiquette BCP 47 valide, 1 à 200
     caractères sans caractère de contrôle, langue par défaut présente.
   - *Codes du logiciel* (permissions, règles, problèmes, sortes, états, natures,
     usages) : libellés FR et EN compilés à côté de leur définition, dans le noyau et
     dans chaque module (fonction `labels()` du contrat de module, clause additive de
     l'ADR 0026). Un test vérifie la complétude.
   - Les modules fournissent les libellés de leurs propriétés et valeurs ; la
     synchronisation ne remplace jamais un libellé posé par un administrateur (R10).

3. **Une route : `GET /v1/labels` (R19).** Pour une langue, la table clé → libellé,
   rangée par espace (`value`, `property`, `role`, `permission`, `rule`, `problem`,
   `kind`, `state`, `policy.<collection>`…), filtrable par `spaces=`. Catalogues
   seulement : jamais les objets du référentiel. Les Problem Details gardent leur code
   (`type`) et un `title` neutre ; leur message lisible peut venir de l'espace `problem`.

4. **Négociation (R9, R21).** `?lang=` prioritaire, sinon `Accept-Language` ; choix par
   le schéma Lookup (RFC 4647 §3.4) parmi les langues disponibles, puis la langue par
   défaut de l'instance (`[i18n] default_locale`, publiée aux capacités). Réponse avec
   `Content-Language` et `Vary: Accept-Language`, sur cette route seulement. Une entrée
   sans libellé dans la langue servie prend celui de la langue par défaut et figure dans
   `fallback` ; sans aucun libellé, elle est absente et le client rend la clé.

5. **Révision et cache (R22).** Un compteur `label_revision`, incrémenté dans la
   transaction de tout changeset qui écrit une colonne `labels`, combiné à l'empreinte
   des libellés compilés (calculée au démarrage), forme la révision. ETag
   `"<révision>.<empreinte du corps>"` (forme de 006/H19), l'empreinte couvrant langue,
   espaces et filtrage. `valid_at` et `as_of` comme toute lecture ; `Cache-Control:
   private, max-age=86400` pour une lecture à `as_of` passé, `no-cache` au présent.

6. **Invalidation (R23).** Le changeset qui incrémente la révision écrit dans l'outbox
   `org.kartenn.labels.changed` ; le flux `GET /v1/events` (spec 022) le remet à toute
   connexion comme sorte `labels` (`{"revision": …}`, rien d'autre). Le client relit
   le dictionnaire sous `If-None-Match` ; à `ready` ou `reset`, il relit tout.

7. **Droits (R24).** Un espace de données n'est rendu qu'à qui lit la collection qui le
   possède, sur une portée quelconque (`policy.*` sous `policy.catalogue.read`, `role`
   sous `core.rbac.read` ou pour les rôles de ses propres liaisons, valeurs d'une
   propriété sensible sous `prop.<clé>.read`) ; les espaces de codes à tout
   authentifié. Route authentifiée, jamais publique.

8. **Clients.** Le SDK web (021) et la bibliothèque Python (023) gardent le
   dictionnaire en cache par (langue, révision) et résolvent localement, avec repli sur
   la clé. Les textes de l'interface (boutons, titres d'écran) restent dans les
   catalogues des clients (007/W11 amendée).

## Alternatives

- Libellé résolu dans chaque objet selon `Accept-Language` (Q-018-4, B) : un ETag et un
  cache par langue sur chaque objet, `Vary` partout.
- Table `labels` sur l'objet et `label` résolu dans la langue par défaut (Q-018-4, A) :
  deux sources pour un même texte, et toujours un client qui doit traduire les codes.
- Table de libellés partagée (Q-018-6, B) : une jointure par lecture et une version
  séparée de l'objet propriétaire.
- Libellés hors du temps (Q-018-5, B) : une vue datée montrerait un libellé postérieur.
- Révision tirée de `max(sys_start)` : un agrégat par table à chaque lecture, et aucune
  source pour l'événement.

## Consequences

- Les champs `label` disparaissent du contrat (rôles, objets de la politique) sans
  compatibilité (Q-018-2) ; `policy_value.label` migre vers les libellés des valeurs de
  `policy.environment` et des autres axes au noyau.
- Un changeset qui écrit des libellés verrouille la ligne de révision : ces changesets
  sont sérialisés entre eux (rares, à mesurer au banc).
- Le dictionnaire filtré par droits existe en autant de variantes que d'ensembles
  d'espaces lisibles ; la mise en mémoire par instance se clé sur cet ensemble.
- Une mise à jour du serveur qui change un libellé compilé change l'ETag, même sans
  écriture en base.
- 007/W11, 010/S29, 022/R2 et 023/T15 sont amendées (spec 018, « Amendements d'autres
  specs »).
