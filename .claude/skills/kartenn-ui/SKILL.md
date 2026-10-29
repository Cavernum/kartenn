---
name: kartenn-ui
description: Contrat d'écriture de l'interface de kartenn (nouveau shell web/v2, bibliothèque @kartenn/ui, plugins de première partie) — jetons, couches CSS, composants et leurs motifs APG, états des vues, CSP, raccourcis réservés, pièges de Reka UI, clichés à éviter. À suivre pour toute écriture ou relecture sous web/.
paths:
  - "web/**"
---

# kartenn-ui — écrire l'interface de kartenn

Sources : spec 020 (`specs/020-systeme-de-design/`, design § « Brief », « Jetons »,
« Architecture CSS », « Inventaire des composants »), spec 021 (shell v2, § « Raccourcis »),
ADR 0036 à 0039, `docs/reference/systeme-de-design.md`. Ce skill les résume ; en cas
d'écart, la spec fait foi. Le code nouveau vit sous `web/v2/` (espace pnpm distinct,
non servi avant la bascule) ; `web/` ne reçoit que des correctifs.

## 1. Contrat

1. **Seulement `@kartenn/ui` et les jetons.** Un écran ou un plugin n'écrit ni bouton, ni
   champ, ni liste de choix, ni table brute (lint), n'importe pas `reka-ui` (lint), et ne
   pose aucune valeur brute : couleur, longueur, rayon, ombre, durée, police viennent d'un
   `var(--k-…)` (Stylelint). La seule source des valeurs est
   `web/v2/ui/src/tokens/tokens.css`.
2. **Tout composant nouveau remonte à l'agent principal.** Jamais de composant local à un
   écran qui double un composant de la bibliothèque ; la bibliothèque s'écrit par un seul
   agent à la fois (ADR 0039, § 6), un composant (ou une petite famille) par tâche, avec
   sa page de catalogue, ses captures et son script APG.
3. **Toute vue de données a trois états** : chargement en squelette de taille fixe ; vide
   avec une phrase et une action ; erreur avec le code copiable (`CodeBadge`) et
   l'identifiant de requête (`Request-Id`). Jamais de vue blanche.
4. **Lectures et mutations par le SDK** (clés de cache, préchargement, écritures, ADR
   0037) ; aucun `fetch` dans un écran.
5. **Couleur = sens, jamais seule** : un état a sa couleur, son icône **et** son libellé
   (`StatusBadge`) ; un environnement a sa pastille **et** son libellé (`EnvChip`) ; un
   différentiel a son signe (`+`, `−`, `~`) et son libellé.
6. **Clé et libellé** (REQ-020-21) : afficher le libellé dans la langue, la clé en
   infobulle et copiable ; libellé absent → la clé en chasse fixe, jamais une chaîne vide.
7. **Chasse fixe et chiffres tabulaires** pour adresses, préfixes, VID, MAC, codes,
   identifiants, compteurs et dates.

## 2. CSS

- Couches, dans cet ordre, déclarées une fois par `styles/layers.css` :
  `@layer reset, tokens, base, components, plugins, utilities`. Toute règle est dans une
  couche (règle `kartenn/known-layer`). Composants : `@layer components` ; plugins de
  première partie : `@layer plugins`, préfixe `k-<plugin>-`.
- Classes `k-<composant>` et `k-<composant>__<partie>` ; état en attribut ARIA ou
  `data-state` (pas de classe d'état) ; variante `data-variant` ; taille `data-size`.
- Propriétés logiques (`margin-inline-start`, `inline-size`), jamais physiques.
- Pas d'`!important`, pas d'identifiant, trois sélecteurs composés au plus.
- Ombres : `--k-shadow-overlay` et `--k-shadow-modal` dans les seules feuilles des
  composants flottants (liste `FLOATING` de `web/v2/stylelint.config.js`) ; ailleurs, un
  liseré intérieur `--k-shadow-inset-…` seulement.
- Une propriété personnalisée locale s'appelle `--k-<composant>-…` et vaut un jeton.
- Mouvement (020 § « Mouvement », politique et réalisation) : durées `--k-duration-*`,
  courbes `--k-ease-*`, amplitudes `--k-motion-*`, rotations `--k-turn-*` seulement ; sous
  mouvement réduit (préférence du système ou `data-motion="reduce"`), durées nulles sauf
  `fade`/`slow` et amplitudes nulles. `transform` (`translate`, `scale`, `rotate`),
  `opacity` et couleurs seulement ; opacité d'une couche en
  `max(var(--k-duration-medium), var(--k-duration-fade))`. Couche de Reka : entrée par
  `@starting-style`, sortie par `k-fade-out` (la `Presence` attend un `animationend`) ;
  focus placé sans attendre ; pas de `mode="out-in"`, pas de View Transitions, pas de
  minuterie. Jamais animés : tri, filtre, actualisation, valeurs, défilement, contenu d'un
  nœud, changement de page. Chevron de repli : `k-chevron` + `data-open`. Aucune animation
  en boucle hors indicateur de chargement ; le reflet du squelette (classe partagée
  `k-skeleton-fill`, jamais recopiée) tourne tant que la zone charge, arrêté par le
  mouvement réduit (système ou `data-motion="reduce"`, le mécanisme de WCAG 2.2.2).

### Structure avant style (spec 020 § « Structure avant style », 024/M12, M15)

La géométrie se règle avant la couleur : une maquette qui déborde ou se décale à une
largeur est fausse, même belle à 1280 px.
- **Pistes élastiques** : `minmax(0, 1fr)`, jamais `1fr` nu ; tout enfant de grille ou de
  flex qui porte un contenu long a `min-inline-size: 0`.
- **Contenu long** : il défile dans son emplacement (`overflow: auto`), jamais la page ;
  aucune page ne défile horizontalement de 1024 à 1920 px.
- **Tables** : en-tête et lignes dans une seule grille (`grid-template-columns` sur la
  grille, lignes en `subgrid`), en-tête collant dans le même conteneur de défilement ;
  jamais deux grilles qui « devraient » avoir les mêmes colonnes.
- **Surimpressions** : menus, sélecteurs, infobulles dans la couche `layers` du cadre
  (`providePortalTarget`) ; la surface en `isolation: isolate`, pour qu'un en-tête collant
  ne passe pas au-dessus du panneau droit ni d'un menu.
- **Barres d'outils** : `Toolbar` replie dans « … », jamais de retour à la ligne.
- **Messages** : une phrase va à la ligne, jamais d'ellipse sans moyen de lire le texte
  entier ; une liste longue dans un lieu étroit est une `ExpandableGrid`.
- **Cinq largeurs** : avant de rendre une maquette ou un écran, le voir à 1440, 1280,
  1024, 768 et 390 px, en clair et en sombre ; la capture le prouve ; rien ne défile de
  côté à 320 px hors d'une grille (`layout.spec`).
- **Adaptation** (spec 020 § « Adaptation à la largeur ») : le cadre seul lit la fenêtre
  (`@media` en syntaxe d'intervalle, seuils de `WIDTH_CLASSES` recopiés, `data-width`) ;
  un composant lit sa racine conteneur (`container-type: inline-size`, `@container` aux
  seuils de `CONTAINER_WIDTHS` seulement), jamais une `@media` de largeur ; `pointer` et
  `hover` en `@media` restent permis. Grilles : `priority` sur les colonnes secondaires
  (2 sous `md`, 3 sous `lg`), sélecteur « Colonnes » fourni par la grille ; tuiles en
  `Grid columns="auto"` ; rien de réservé au survol.
- **Droits** : une action interdite reste visible, `aria-disabled`, infobulle « Il faut
  `<permission>` » ; jamais d'encart permanent des droits.

## 3. CSP (ADR 0025, REQ-020-14)

- Aucun attribut `style` en chaîne (`style="…"`, `:style="'…'"`, gabarit), aucun `<style>`
  injecté, aucun `cssText`, aucun `setAttribute('style', …)`, aucun `v-html`.
- Valeur dynamique (position d'une couche, largeur de colonne, décalage d'une ligne
  virtualisée) : `el.style.setProperty('--k-…', v)` ou `:style` **en objet** dont les clés
  sont des jetons `--k-…` (CSSOM, permis par `style-src 'self'`).
- Reka UI : `ComboboxViewport` et `Splitter*` injectent un `<style>` : interdits (lint).
  Tout autre composant pris en défaut par le parcours CSP rejoint la liste.

## 4. Inventaire (motif APG — à compléter au fil de P4)

Statut : ☐ à écrire, ◐ version visuelle, ● complet (états, clavier, captures).

| Composant | Motif APG ou rôle | Statut |
|---|---|---|
| `Icon` | `aria-hidden` par défaut ; nom obligatoire si seule porteuse de sens (`meaningful` + `label`, vérifié au typage) | ◐ |
| `Stack`, `Cluster`, `Grid`, `Split` | mise en page ; écart par `gap` (échelle), `Split` dans l'ordre de lecture | ◐ |
| `Button`, `IconButton` | Button ; occupé = `aria-disabled` + `aria-busy` (reste focalisable), bascule `aria-pressed`, raccourci `aria-keyshortcuts` ; `disabled` natif sort de l'ordre de tabulation : une action refusée avec motif prend `forbidden` ou `reason`, jamais `disabled` ; occupé ou refusé, un `submit` n'envoie rien ; `IconButton` : infobulle automatique de son nom et de son raccourci (`shortcut`), décorative, montée ouverte seulement ; refusé, son nom puis le motif, la description = le motif seul ; cibles ≥ 24 px (`sm` en compacte) | ● |
| `TextField`, `FormField` | champ natif étiqueté ; erreur puis aide par `aria-describedby` | ◐ (`TextField`) |
| `Select`, `Combobox` | Listbox ; Combobox | ☐ |
| `FilterChip`, `FilterPicker` | Button + Combobox en séquence | ◐ (`FilterChip`) |
| `DateTimeField`, `PeriodPicker` | Date Picker Dialog ; spin button | ☐ |
| `Checkbox`, `Switch` | Checkbox ; Switch | ☐ |
| `Tabs` | Tabs (activation manuelle) | ◐ |
| `Menu` (bouton et menu) | Menu Button, Menu ; entrée désactivée focalisable (`aria-disabled`), motif en description et infobulle ; entrée à cocher (`menuitemcheckbox`, menu gardé ouvert) ; entrées `radio` (`menuitemradio` en groupe, l'une cochée) | ◐ |
| `Dialog`, `AlertDialog`, `Drawer` | Dialog (Modal), Alert Dialog ; `Drawer` modal seulement | ◐ |
| `SidePanel` | `complementary` non modal + Tabs ; surimpression de la surface à toute largeur, jamais à côté (hôte positionné, surface `isolation: isolate` ; la surface garde sa largeur et ses colonnes), redimensionnable (Window Splitter maison) ; `cover` en classe étroite (toute la surface, sans poignée) ; `sheet` en classe compacte (toute la fenêtre, « Retour ») ; `rail` : replié, languette dans le flux (piste `auto` de l'hôte) dont le chevron « Ouvrir le panneau » le rouvre ; entre et sort depuis son bord (`<Transition>`, jamais au premier rendu) ; ce qu'il couvre est `inert` (`cover` : son conteneur ; `sheet` : Dialog (Modal), tout jusqu'à `[data-k-modal-scope]`) ; par-dessus la fin, publie `--k-side-panel-inset` (marge de défilement de l'hôte) | ● |
| `CommitForm`, `SplitButton` | champs étiquetés, Ctrl+Entrée ; Button + Menu Button (entrées désactivées avec motif) | ● |
| `Toolbar` | Toolbar ; actions repliées dans « … » par priorité ; sous 360 px, l'action principale seule ; entrées `more` toujours dans « … » (un seul « … » par barre) | ● |
| `MatrixGrid` | Grid ; en-têtes de lignes et de colonnes figés ; croix de survol ou de la cellule active (`data-cross`, en-têtes compris) ; `axes` : sens lignes ↓ colonnes → au coin et pointes d'accent sur les bordures de la croix (`data-flow` : fin de l'en-tête source, bas de l'en-tête destination, début et haut de la cellule montrée ; pseudo-éléments, retournées en RTL) (virtualisation au-delà de 50 × 50 à venir) | ● |
| `Toast` | `status` / `alert`, pause au survol et au focus | ☐ |
| `Tooltip`, `Popover` | Tooltip (`useTooltip`, action refusée : `forbidden`, `reason` sur `Button`, `IconButton`, entrée de `Menu`) : focus clavier (`:focus-visible`) aussitôt, survol après 300 ms, survolable, fermé au départ du focus ou du pointeur (le pointeur qui part laisse ouverte celle du focus clavier), ou d'un défilement qui déplace l'ancre ; ouverte par `listeners(enabled, { pin, passEscape })`, Échap la ferme où que soit le focus (1.4.13) : sous le pointeur seul, rien d'autre ne reçoit la touche ; sous le focus, la couche autour reste (défaut empêché), sauf `passEscape` (infobulle du nom d'un `IconButton`) : la touche va ensuite au contrôle (son dialogue, sa grille) ; jamais sur un bouton dont le menu est ouvert (`aria-expanded`) ; un clic l'épingle si `pin`, sinon la ferme ; Dialog non modal | ◐ (`Tooltip` ●) |
| `Breadcrumb`, `SidebarNav` | Breadcrumb ; Disclosure dans `navigation` | ◐ |
| `CommandPalette` | Combobox dans un Dialog modal (liste `position="inline"`, sans Viewport) | ◐ |
| `DataGrid` | Grid (maison) ; une seule grille, lignes en `subgrid` (024) ; tri (`aria-sort`) et filtre de colonne (Alt+Bas, `filter`) ; navigation au clavier ; colonne de message `wrap` (va à la ligne) ; états de ligne `draft`, `review`, `removed` (barrée), ligne entrée dans un brouillon éclairée une fois (`data-fresh`) (virtualisation et édition à venir) ; racine conteneur, colonnes à `priority` masquées par `@container` et sélecteur « Colonnes » (`columnVisibility`), clé collante ; sélection Espace, Maj+Espace, Ctrl+A (`selectable` ou `"single"`) ; widgets d'une cellule hors tabulation (Entrée, F2, Échap) ; colonne jamais plus étroite que son en-tête ; erreur de cellule sur sa propre ligne | ◐ |
| `ExpandableGrid` | `DataGrid` compacte (compte de lignes, `visibleRows`, `compactColumns`) + `IconButton` « Agrandir » qui ouvre la même table dans un `Dialog` `lg` `contained` (focus piégé, Échap, focus rendu au bouton) ; tri et filtres de l'hôte, partagés | ● |
| `TreeGrid`, `Tree` | Treegrid ; Tree View ; garde d'affichage `misplaced` ; « + » d'un nœud (`addLabel`, événement `add`, 024/M10) ; clavier de la colonne d'arbre (Haut, Bas, Droite, Gauche, Entrée ou F2 vers le « + »), `expanded` en modèle | ◐ (`TreeGrid`) |
| `CodeBadge`, `KeyLabel`, `EnvChip`, `StatusBadge` | Button (copier, nom = contenu, « Copier » caché) ; — | ◐ |
| `Skeleton`, `EmptyState`, `ErrorState`, `InlineAlert` | `status` présent vide puis rempli, sans `aria-busy` (reflet en boucle `k-skeleton-fill`, fond fixe sous mouvement réduit) ; `alert` ; `status` | ◐ |
| `DatePill`, `DraftCounter`, `TimeBand` | Button ; bascule du panneau (`aria-expanded`, `aria-controls` ; ouvert, icône en croix) et Menu Button des brouillons ; `status` | ◐ (`DraftCounter` ●) |
| `Kbd`, `ShortcutHint` | — ; `Kbd` : une combinaison, `kbd` imbriqués, noms de touches dans la langue (Maj, Origine, Suppr) ; `ShortcutHint` : une combinaison ou une séquence (`[["g"], ["p"]]`, « g puis p »), jamais une séquence dans un seul `Kbd` ; `aria-keyshortcuts` pour une combinaison seulement (`shortcutAria`) | ● |
| `DiffList` | liste groupée, Disclosure ; signe, mot caché, `del`/`ins` | ◐ |
| `DiffValue` | `ins`/`del` en ligne : signe attaché à sa valeur, mot caché ; `changed` avant → après | ◐ |
| `BarChart` | `role="img"` nommé par un résumé + table cachée ; SVG maison mesuré (`ResizeObserver`), infobulle décorative au survol ; l'image prend le focus, flèches de barre en barre (région `polite`) | ● |
| `BarList` | table (en-têtes de ligne, valeurs écrites), barres décoratives ; échelle `max` ou la plus grande valeur | ● |

Écrans de référence : les maquettes validées du catalogue (`/_ui/ecrans/E0` à `E10`,
design § « Maquettes », reprises par la 024) ; un écran de la 021 se compare à sa maquette, jamais à
l'ancien shell.

## 5. Raccourcis réservés (spec 021, § « Raccourcis »)

Ctrl+K (palette, partout) ; `/` (filtre de la vue) ; `?` (aide) ; `g` + lettre
(navigation : `d p r a e s c i b x h o`, et `v m t q f` des plugins) ; `c` (créer) ; `e`
(modifier) ; Ctrl+Entrée (appliquer ; action du bouton scindé) ; Échap (fermer,
annuler) ; `[` (barre latérale) ; `]` (panneau droit) ;
Alt+Flèche bas sur un en-tête de grille (filtre de colonne, convention maison, dite dans
l'aide). Un composant qui consomme les caractères (recherche à la saisie de l'arbre)
porte `data-k-typeahead`. Les raccourcis à une touche se désactivent (WCAG 2.1.4) ; ils
sont montrés par `Kbd` (une combinaison) ou `ShortcutHint` (une séquence) dans les menus,
les infobulles et l'aide, lus au registre du SDK.

## 6. Pièges de Reka UI (prototype, `docs/spikes/005-framework-web.md`)

Version exacte épinglée ; toute montée repasse les scripts clavier et le parcours CSP.
- Sélection de l'arbre : Reka stocke une **copie** ; sélection et opérations par
  **identifiant**, nœuds `markRaw`, listes en `shallowRef`.
- Arbre virtualisé : Haut/Bas visent le voisin **rendu** (appuis perdus) ; Haut et Bas
  gérés par `@kartenn/ui` sur l'index aplati.
- Pas de `*`, Entrée sélectionne, tampon de recherche pollué par les touches non
  imprimables : gestionnaire placé avant ceux de Reka (`stopImmediatePropagation`).
- `ComboboxViewport`, `Splitter` : `<style>` injecté (CSP) ; interdits.
- Combobox ouverte : `aria-hidden` sur le reste de la page ; filtres en popover, deux Échap.
- Slots de `TreeVirtualizer` typés `Record<string, any>` : enveloppes typées dans
  `@kartenn/ui`, aucun transtypage dans les écrans.
- Ligne focalisée démontée par la molette : la garder montée.
- Nom accessible d'une ligne = tout son contenu : `aria-label` court, détail par
  `aria-describedby`.
- `DropdownMenuItem` écrit `aria-disabled` d'après sa prop `disabled`, qui sort aussi
  l'entrée des flèches : une entrée refusée garde `disabled` faux, `aria-disabled` posé par
  la directive de `Menu`, et `select` annulé.
- Tests au clavier : pas d'horloge figée (`page.clock.setFixedTime`) ; Vue ignore un
  événement daté de l'instant où le gestionnaire d'un ancêtre a été posé, et une horloge
  figée rend tous les instants égaux (`e2e/visual/clavier.spec.ts`).

## 7. Clichés à éviter (brief)

Cartes arrondies identiques à ombre grise ; dégradés décoratifs ; icônes colorées sans
sens ; animation d'entrée des listes ; gros titres creux ; couleur sans libellé. Aussi :
marges généreuses de vitrine (l'outil est dense : lignes de 28 px), texte gris trop pâle,
boutons primaires multiples dans une même zone.

## 8. Outillage

- Après chaque édition d'un fichier `web/**`, le hook `.claude/hooks/web-apres-edition.sh`
  passe Prettier, Stylelint et ESLint sur ce fichier ; un échec revient en diagnostic.
- Vérification : `k-web2-check` (Prettier, ESLint, Stylelint, vue-tsc, Vitest, captures) ;
  `k-web2-e2e` (parcours) ; références des captures : `k-web-shots --update`, dans devenv
  seulement, relues dans le diff avant commit.
- Rendu : catalogue `/_ui` (`?mode=light|dark`, `?densite=compacte|confortable`,
  `?langue=fr|en`) ; MCP `playwright`, `browser_snapshot` d'abord.
- Catalogue : un état = `{ name, description, props }` dans `<Nom>.catalogue.ts`, plus
  `slot` (texte), `demo` (composition), `interaction` (`hover`, `focus`, `press` joués
  avant la capture), `wide` (pleine largeur), `isolated` (couche modale : montrée seule,
  par `?etat=`, et vérifiée par axe sur sa page). Les couches flottantes vont dans
  `providePortalTarget` (le catalogue les garde dans `main`) ; les liens passent par
  `provideLink` (le routeur du shell). Captures : un état par mode, plus la page entière
  en densité confortable ; un survol attend le délai d'une infobulle avant la capture.
- Scripts clavier des composants de P4 : `web/v2/e2e/components/<composant>.spec.ts`
  (projet `visual`, donc `k-web2-check`), sur un état du catalogue ouvert par `openState`
  (horloge réelle) ; l'état `keyboard` d'un composant rassemble ce que le script joue.
- Relectures : sous-agents `ui-reviewer` (maquette, brief, doublons) et `a11y-auditor`
  (motif APG, WCAG 2.2 AA) à la fin de chaque composant, maquette ou écran.
