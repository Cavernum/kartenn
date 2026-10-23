<script setup lang="ts">
// The icons (design « Icônes »): the review sheet of the business icons beside their
// Lucide neighbours (Q-020-7, to be validated by the user), then the closed list.
import { Icon, lucideIcons, metierIcons, type IconName } from "@kartenn/ui";

const labels: Record<keyof typeof metierIcons, string> = {
  prefix: "Préfixe",
  range: "Plage",
  address: "Adresse",
  domain: "Domaine d'adressage",
  vlan: "VLAN",
  interface: "Interface",
  cluster: "Machine en cluster",
  flow: "Flux",
  environment: "Environnement",
  exception: "Dérogation",
  changeset: "Changeset",
  draft: "Brouillon",
  matrix: "Matrice de politique",
};
const neighbours: Record<keyof typeof metierIcons, IconName[]> = {
  prefix: ["network", "layers"],
  range: ["arrow-up-down", "minus"],
  address: ["tag", "network"],
  domain: ["globe", "layers"],
  vlan: ["tag", "network"],
  interface: ["plug", "server"],
  cluster: ["server", "network"],
  flow: ["arrow-right", "send"],
  environment: ["tag", "layers"],
  exception: ["shield", "triangle-alert"],
  changeset: ["file-diff", "git-compare"],
  draft: ["pencil", "file-diff"],
  matrix: ["shield", "lock"],
};
const metier = Object.keys(metierIcons) as (keyof typeof metierIcons)[];
const lucide = Object.keys(lucideIcons) as IconName[];
</script>

<template>
  <section class="k-cat-section" aria-labelledby="ui-icons-title">
    <h1 id="ui-icons-title" class="k-cat-title">Icônes</h1>
    <p class="k-cat-lead">
      Lucide (trait 1,5) et icônes métier dessinées sur la même grille de 24 unités. Tailles 16 et
      20 px ; agrandissement ×3 pour la relecture du dessin.
    </p>

    <h2 class="k-cat-subtitle">Planche de revue des icônes métier — à valider</h2>
    <table class="k-cat-table k-cat-icons-sheet">
      <thead>
        <tr>
          <th scope="col">Icône</th>
          <th scope="col">16 px</th>
          <th scope="col">20 px</th>
          <th scope="col">×3</th>
          <th scope="col">Voisines Lucide</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="name in metier" :key="name" :data-icon="name">
          <th scope="row">
            {{ labels[name] }} <code class="k-cat-value">{{ name }}</code>
          </th>
          <td><Icon :name="name" /></td>
          <td><Icon :name="name" :size="20" /></td>
          <td class="k-cat-icon-large"><Icon :name="name" :size="20" /></td>
          <td>
            <span class="k-cat-neighbours">
              <span v-for="other in neighbours[name]" :key="other" class="k-cat-neighbour">
                <Icon :name="other" :size="20" />
                <code class="k-cat-value">{{ other }}</code>
              </span>
            </span>
          </td>
        </tr>
      </tbody>
    </table>

    <h2 class="k-cat-subtitle">Liste fermée Lucide ({{ lucide.length }})</h2>
    <ul class="k-cat-icon-grid">
      <li v-for="name in lucide" :key="name" class="k-cat-icon-cell">
        <Icon :name="name" :size="20" />
        <code>{{ name }}</code>
      </li>
    </ul>
  </section>
</template>

<style>
@layer components {
  .k-cat-icons-sheet td {
    color: var(--k-text);
  }

  .k-cat-icon-large .k-icon {
    inline-size: calc(var(--k-icon-size-lg) * 3);
    block-size: calc(var(--k-icon-size-lg) * 3);
  }

  .k-cat-neighbours {
    display: flex;
    gap: var(--k-space-4);
  }

  .k-cat-neighbour {
    display: inline-flex;
    gap: var(--k-space-1);
    align-items: center;
  }

  .k-cat-icon-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(calc(var(--k-space-12) * 3), 1fr));
    gap: var(--k-space-2);
  }

  .k-cat-icon-cell {
    display: flex;
    gap: var(--k-space-2);
    align-items: center;
    font-size: var(--k-text-sm);
  }
}
</style>
