---
status: proposed
date: 2026-10-23
---

# 0039 — Outillage Claude pour la qualité de l'interface

## Context

L'interface est écrite par des agents. Sans guidage, ils convergent vers une esthétique
générique, inventent des composants en double quand ils travaillent en parallèle, et ne
voient pas leur rendu ; la revue visuelle et clavier faite dans l'agent principal coûte
cher en contexte (captures). La documentation de Claude Code recommande de donner à
l'agent un moyen de vérifier son travail, des hooks déterministes plutôt que des
consignes, et des sous-agents à contexte isolé pour les relectures
(code.claude.com/docs : « best-practices », « hooks-guide », « sub-agents », « skills »).

Arbitrage de l'utilisateur du 2026-10-23 : D-12 (agents `ui-reviewer` et `a11y-auditor`,
skill `kartenn-ui`, hook après édition Stylelint et ESLint, `bench-runner` étendu à
`bench-web`). ADR 0024 (MCP Playwright, `k-web-check`, `k-web-e2e`), ADR 0036 (système
de design), CLAUDE.md (économie de contexte, `k-heavy`, trois agents d'implémentation au
plus).

## Decision

1. **Skill `kartenn-ui`** (`.claude/skills/kartenn-ui/SKILL.md`, chargé sur `web/**`,
   moins de 500 lignes, détail en annexes) : le contrat d'écriture de l'interface.
   - n'utiliser que `@kartenn/ui` et les jetons, aucune valeur brute ;
   - toute vue de données a ses trois états (chargement en squelette, vide avec phrase
     et action, erreur avec code copiable et identifiant de requête) ;
   - les lectures et mutations passent par le SDK (clés, optimisme, ADR 0037) ;
   - inventaire des composants (une ligne chacun, motif APG), écrans de référence
     (maquettes du catalogue), raccourcis clavier réservés, liste des clichés à éviter ;
   - tout composant nouveau remonte à l'agent principal, jamais local à un écran.
2. **Sous-agent `ui-reviewer`** (sonnet ; Read, Grep, Bash, MCP `playwright`) : compare
   un écran ou un composant à sa maquette validée et au skill (alignements, espacements,
   tailles, états, cohérence, doublons de composants) ; produit et lit les captures dans
   son propre contexte ; rend une liste d'écarts classés. Ne modifie rien.
3. **Sous-agent `a11y-auditor`** (sonnet ; Read, Grep, Bash `k-web-e2e`, MCP
   `playwright`) : parcours clavier selon le motif APG de chaque composant, ordre et
   visibilité du focus (WCAG 2.4.7, 2.4.11), noms accessibles, cibles de 24 px (2.5.8),
   alternative au glisser-déposer (2.5.7), raccourcis à une touche désactivables
   (2.1.4) ; rend les écarts. Ne modifie rien.
4. **Hook `PostToolUse`** sur `Edit|Write` des fichiers `web/**` : Prettier, puis
   Stylelint (feuilles et blocs de style) et ESLint sur le seul fichier touché ; sortie
   d'une ligne en succès, diagnostics en échec rendus à l'agent. Il ne lance ni tests ni
   build (coût).
5. **`bench-runner` étendu à `bench-web`** : rend le tableau des budgets du navigateur
   (taille, premier écran, INP, navigation en cache, CLS, trames perdues de l'arbre,
   mémoire) contre leurs seuils, sans les traces.
6. **Partage du travail** : jetons et bibliothèque écrits séquentiellement par un seul
   agent à la fois (une tâche = un composant ou une petite famille, avec sa page de
   catalogue, ses captures et son test APG) ; écrans parallélisés par zone ensuite, trois
   agents au plus, chacun en worktree. Le rôle de concepteur et les arbitrages
   esthétiques restent dans l'agent principal avec l'utilisateur.

## Résolution de D-1 : Vue 3 (2026-10-23)

- Le hook et le skill emploient `eslint-plugin-vue` (règles de l'ADR 0036) et
  `no-restricted-imports` pour les composants Reka interdits ; les exemples du skill sont
  des SFC `<script setup>`.
- Le skill `kartenn-ui` consigne les pièges de Reka relevés au prototype (sélection par
  copie, navigation virtualisée, interception du clavier, `<style>` injecté,
  `aria-hidden` du Combobox, types perdus dans les slots) ; `a11y-auditor` rejoue les
  scripts clavier à chaque montée de version de Reka.

## Alternatives

- **Relecture dans l'agent principal** : plus coûteuse en contexte (captures), moins
  indépendante (l'auteur relit son raisonnement).
- **Consignes dans CLAUDE.md seules** : consultatives, non déterministes.
- **Hook qui lance toute la vérification web** : trop lent après chaque édition, et
  concurrent de `k-heavy`.
- **Agent `perf-auditor` séparé** : doublon de `bench-runner`, qui sait déjà rendre un
  tableau de budgets.

## Consequences

- Nouveaux fichiers sous `.claude/` (agents, skill, réglage du hook) ; CLAUDE.md gagne
  une ligne qui renvoie au skill, sans le recopier.
- Les relectures UI produisent une liste d'écarts consignée à la clôture de la tâche ;
  les écarts acceptés par l'utilisateur deviennent des décisions de la spec 020.
- Le hook ralentit légèrement chaque édition web (Stylelint et ESLint sur un fichier).
