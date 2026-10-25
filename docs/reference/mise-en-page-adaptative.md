# Mise en page adaptative

Lu le 2026-10-25. Les exigences des sources sont marquées **[N]** (normatif) ; les choix de
kartenn **[K]**. Aucune source ne fixe de seuils de largeur : ceux de la 020 sont des
choix de kartenn.

## Sources

- [WCAG22] WCAG 2.2 (Recommandation W3C), https://www.w3.org/TR/WCAG22/ : SC 1.4.10
  `#reflow`, 1.4.4 `#resize-text`, 1.4.12 `#text-spacing`, 1.3.4 `#orientation`, 2.5.8
  `#target-size-minimum` ; documents « Understanding » (informatifs) :
  https://www.w3.org/WAI/WCAG22/Understanding/reflow.html,
  https://www.w3.org/WAI/WCAG22/Understanding/resize-text.html,
  https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html.
- [MQ4] Media Queries Level 4 (CRD du 19 février 2026), § 2.4.3 (syntaxe d'intervalle),
  § 7 (`pointer`, `hover`, `any-*`), https://www.w3.org/TR/mediaqueries-4/.
- [CONTAIN3] CSS Containment Level 3 (WD 2022), § 4.1 à 4.4, § 6 (unités `cq*`),
  https://www.w3.org/TR/css-contain-3/ ; [COND5] CSS Conditional Rules Level 5 (WD du
  30 octobre 2025), § 5.4 `@container`, § 6.1 requêtes de taille, § 6.2 requêtes de style,
  https://www.w3.org/TR/css-conditional-5/.
- [VALUES4] CSS Values 4 (WD du 12 mars 2024), § 6.1.2 (unités `sv*`, `lv*`, `dv*`),
  https://www.w3.org/TR/css-values-4/.
- [GRID1] CSS Grid Level 1, § 7.2.1 (`minmax()`, `fit-content()`), § 7.2.3.2
  (`auto-fit`), https://www.w3.org/TR/css-grid-1/ ; [GRID2] CSS Grid Level 2 (CRD du
  26 mars 2025), § 7.2 `subgrid`, https://www.w3.org/TR/css-grid-2/.
- [SEL4] Selectors Level 4 (WD du 22 janvier 2026), § 4.5 `:has()`,
  https://www.w3.org/TR/selectors-4/.
- [TEXT4] CSS Text Level 4 (WD du 14 août 2026), § 5.4 `text-wrap-style`,
  https://www.w3.org/TR/css-text-4/.
- [ANCHOR1] CSS Anchor Positioning 1 (WD du 27 mars 2026),
  https://www.w3.org/TR/css-anchor-position-1/.
- Prise en charge : webstatus.dev et données de compatibilité de la documentation MDN
  (secondaires), lues le 2026-10-25.

## Ce que disent les sources

1. **SC 1.4.10 Reflow** (AA) **[N]** : le contenu se présente sans perte et sans
   défilement dans deux dimensions à une largeur de 320 px CSS (contenu à défilement
   vertical), « except for parts of the content which require two-dimensional layout for
   usage or meaning ». Note 1 : 320 px équivalent à 1280 px zoomés à 400 %. Note 2 : les
   tableaux de données (pas leurs cellules) et les interfaces qui gardent une barre
   d'outils visible pendant la manipulation en font partie ; le défilement à deux
   dimensions y est acceptable. Understanding : limiter ce défilement aux seules sections
   qui l'exigent ; un tableau peut défiler dans son propre conteneur.
2. **SC 1.4.4 Resize Text** (AA) **[N]** : texte agrandi à 200 % sans perte ; s'applique à
   chaque variante de la page présentée selon la taille d'écran.
3. **SC 1.4.12 Text Spacing** (AA) **[N]** : aucune perte quand l'utilisateur impose
   interligne 1,5, paragraphes 2, lettres 0,12, mots 0,16 : pas de hauteur fixe de texte.
4. **SC 1.3.4 Orientation** (AA) **[N]** : pas d'orientation unique imposée sauf si
   essentielle.
5. **SC 2.5.8 Target Size (Minimum)** (AA) **[N]** : cibles d'au moins 24 × 24 px CSS, sauf
   espacement, équivalent, en ligne, contrôle du navigateur, essentiel ; indépendant du
   zoom.
6. **[MQ4]** : `(400px < width < 1000px)` est la syntaxe d'intervalle ; `pointer: coarse`
   signale un pointeur principal peu précis, sans garantie (l'agent choisit le dispositif
   « principal ») ; `any-pointer` et `any-hover` couvrent l'union des dispositifs.
7. **[CONTAIN3]/[COND5]** : `container-type: inline-size` applique les confinements de mise
   en page, de style et de taille en ligne ; un élément interroge ses ancêtres conteneurs,
   pas lui-même ; sans conteneur, les unités `cq*` valent celles de la petite fenêtre. Un
   conteneur `inline-size` ne tire plus sa largeur de son contenu : elle doit venir du
   contexte.
8. **[VALUES4] § 6.1.2** : les unités `v*` se rapportent à la grande fenêtre ; `dv*` suit
   l'interface du navigateur et peut changer sans que la fenêtre change (coûteux).
9. **[GRID1]** : `auto-fit` replie les pistes vides ; `minmax(min(100%, X), 1fr)` évite le
   débordement sous la largeur minimale. **[GRID2]** : `subgrid` reprend les pistes du
   parent.
10. **[TEXT4]** : `text-wrap: balance` équilibre les lignes (l'agent peut y renoncer
    au-delà de dix lignes) ; `pretty` favorise la qualité sur la vitesse.

## Prise en charge (secondaire)

| Fonction | État |
|---|---|
| Requêtes de conteneur de taille, unités `cq*` | largement disponibles |
| Requêtes de style | nouvellement disponibles (mai 2026) : pas sans repli |
| `:has()`, `subgrid` | largement disponibles (depuis 2023) |
| `text-wrap: balance` | nouvellement disponible (2024) ; plafond de lignes propre à chaque moteur |
| `scrollbar-gutter` | nouvellement disponible (décembre 2024) |
| Positionnement par ancre | données incomplètes : amélioration progressive |

## Implications pour kartenn

- **[K]** Le cadre lit la fenêtre (`@media`), les composants lisent leur conteneur
  (`@container`) ; détail dans la 020, § « Adaptation à la largeur ».
- **[N → K]** Les grilles défilent dans leur propre région (point 1) ; tout le reste se
  replie à 320 px, vérifié par un test à cette largeur.
- **[N]** Pas de hauteur fixe de texte (point 3), pas d'orientation imposée (point 4),
  pas de `user-scalable=no` ni `maximum-scale` (point 2).
- **[K]** Cibles de 44 px pour les actions principales sous `any-pointer: coarse` ; 24 px
  partout (point 5).
- **[K]** Pas de `clamp()` sur le texte courant d'une application dense ; réservé aux
  titres et aux marges.

## Non trouvé ou non relu

- Aucune exigence HTML normative pour `<meta name=viewport>` (CSS Viewport 1 est un
  premier brouillon, incohérent sur `device-width`).
- Non relus en entier : COND5 § 5.4 et § 6.2, VALUES4 § 10.2 (`clamp()`), CSS Overflow 3
  (`scrollbar-gutter`), errata de WCAG 2.2.
