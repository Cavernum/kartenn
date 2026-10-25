# Mouvement dans l'interface

Lu le 2026-10-25. Les exigences des sources sont marquées **[N]** (normatif) ; les choix de
kartenn **[K]**. Aucune source officielle ne fixe de durées ni de courbes : tous les
nombres en millisecondes ci-dessous sont des choix de kartenn.

## Sources

- [WCAG22] WCAG 2.2, https://www.w3.org/TR/WCAG22/ ; documents « Understanding » :
  https://www.w3.org/WAI/WCAG22/Understanding/animation-from-interactions.html,
  https://www.w3.org/WAI/WCAG22/Understanding/pause-stop-hide.html.
- [MQ5] Media Queries Level 5 (WD du 19 février 2026), § 12.1 `prefers-reduced-motion`,
  https://www.w3.org/TR/mediaqueries-5/.
- [TRANS2] CSS Transitions Level 2 (WD du 4 février 2026), § 2.1, 2.5, 3.3,
  https://www.w3.org/TR/css-transitions-2/.
- [EASING2] CSS Easing Functions Level 2 (FPWD du 29 août 2024), § 2.3,
  https://www.w3.org/TR/css-easing-2/.
- [VT1] CSS View Transitions Level 1 (CRD du 28 mars 2024), § 4.2, 7.1.1,
  https://www.w3.org/TR/css-view-transitions-1/.
- [VUE] Documentation de Vue, `<Transition>`,
  https://vuejs.org/guide/built-ins/transition.html (documentation du cadriciel,
  informative).
- Compatibilité : données `browser-compat-data` (dépôt public de la documentation MDN),
  secondaires, lues le 2026-10-25.

## Ce que disent les sources

1. **SC 2.3.3 Animation from Interactions** (AAA, visé et non exigé) **[N]** : « Motion
   animation triggered by interaction can be disabled, unless the animation is essential
   […] ». La technique suffisante C39 est la requête `prefers-reduced-motion`. Les
   changements de couleur, de flou ou d'opacité qui ne changent ni taille, ni forme, ni
   position ne sont **pas** du mouvement au sens de WCAG. Un élément qui apparaît sans
   transition n'est pas animé.
2. **SC 2.2.2 Pause, Stop, Hide** (A) **[N]** : ce qui bouge seul plus de 5 s, en même
   temps que d'autres contenus, doit pouvoir être mis en pause, arrêté ou masqué. Une
   animation de chargement peut être essentielle si l'interaction est impossible pendant
   ce temps (note 4). Une auto-actualisation doit pouvoir être arrêtée ou espacée.
3. **SC 2.3.1** (A) **[N]** : rien ne clignote plus de trois fois par seconde.
4. **[MQ5] § 12.1** : `reduce` signale une préférence pour une interface qui « removes or
   replaces » les animations de mouvement. C'est une définition, sans obligation pour
   l'auteur.
5. **[TRANS2]** : `transition-behavior: allow-discrete` démarre aussi les transitions des
   propriétés discrètes (`display`, `overlay`), qui basculent à 50 % ; `@starting-style`
   donne le style de départ d'un élément qui apparaît.
6. **[EASING2] § 2.3** : `ease-out` = `cubic-bezier(0, 0, .58, 1)`, `ease-in` =
   `cubic-bezier(.42, 0, 1, 1)`.
7. **[VT1]** : pendant une View Transition, le test de pointage vise l'élément racine
   (§ 7.1.1) et les éléments capturés ne reçoivent plus le pointeur (§ 4.2) : les entrées
   sont bloquées le temps de l'animation.
8. **[VUE]** : `height` ou `margin` déclenchent la mise en page, à animer avec prudence ;
   `mode="out-in"` attend la fin de la sortie avant l'entrée.

## Compatibilité (secondaire)

| Fonction | Chrome | Firefox | Safari |
|---|---|---|---|
| `@starting-style` | 117 | 129 | 17.5 |
| `transition-behavior` | 117 | 129 (`display` discret à revérifier) | 17.4 |
| `interpolate-size`, `calc-size()` | 129 | non | non |
| View Transitions (même document) | 111 | 144 | 18 |

## Implications pour kartenn

- **[K]** Le mouvement dit un changement d'état ou une provenance (d'où vient le panneau) ;
  jamais décoratif. L'état final s'exprime toujours sans mouvement : couleur, forme,
  texte, `aria-*`.
- **[K]** Seules `transform` et `opacity` s'animent, plus la couleur de fond pour le survol
  et le surlignage. Rien sur la mise en page des grilles ; `interpolate-size` écarté
  (Firefox et Safari ne l'ont pas).
- **[K]** Aucune animation ne bloque l'entrée : le focus est placé à l'ouverture sans
  attendre `transitionend` ; une couche est active dès sa première image ; pas de
  `mode="out-in"` ; pas de View Transitions (point 7).
- **[K]** Interruptible : une transition CSS se renverse d'elle-même ; pas de minuterie
  JavaScript.
- **[N → K]** Sous `prefers-reduced-motion: reduce` (et sous le réglage « Mouvement réduit »
  de l'application, technique SCR40) : translations et changements d'échelle à 0 ms ;
  fondus d'opacité et de couleur gardés à 80 ms (pas du mouvement au sens de WCAG).
- **[N]** Ce qui bouge seul plus de 5 s doit pouvoir être mis en pause, arrêté ou masqué
  (SC 2.2.2) ; l'actualisation automatique d'une vue peut être suspendue.
- **[K]** Le reflet du squelette tourne tant que la zone charge (`infinite`) : arrêté après
  deux passages, il ne se voyait plus sur une attente un peu longue et passait pour un
  état figé (revue du 2026-10-25, soir). Le mécanisme d'arrêt que demande le SC 2.2.2 est
  le réglage : la préférence du système (`prefers-reduced-motion: reduce`) et l'entrée
  « Mouvement : réduit » du menu de l'utilisateur, accessible à tout moment, qui pose
  `data-motion="reduce"` ; sous l'un ou l'autre, la barre reste sur son fond fixe, sans
  reflet. Le reflet est un indicateur de chargement (note 4 du SC : essentiel tant que
  l'interaction attend les données), une seule règle partagée par tous les squelettes.
- **[K]** Les durées et courbes sont celles des jetons de la 020 (§ Mouvement) ; les
  sorties sont plus courtes que les entrées ; rien au-delà de 300 ms pour répondre à une
  action.

## Non trouvé ou non relu

- Aucune source officielle pour les durées, les courbes, ni le délai avant d'afficher un
  squelette.
- Non relus : texte complet du SC 2.3.1, SC 2.2.1 (toasts qui disparaissent seuls),
  CSS Animations 1, Web Animations, View Transitions Level 2.
