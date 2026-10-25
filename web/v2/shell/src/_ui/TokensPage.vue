<script setup lang="ts">
// The tokens (design « Jetons »): swatches, scales, type, density, motion. Values are read
// from the root (getComputedStyle) after each switch, so the page shows what the current
// mode and density resolve to.
import { STATES, tokens } from "@kartenn/ui";
import { nextTick, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";

const route = useRoute();
const values = ref<Record<string, string>>({});

async function read(): Promise<void> {
  await nextTick();
  const style = getComputedStyle(document.documentElement);
  values.value = Object.fromEntries(
    tokens.map((token) => [token, style.getPropertyValue(token).trim()]),
  );
}
onMounted(read);
watch(() => route.query, read);

const named = (names: string[]) => names.map((name) => `--k-${name}`);
const colourGroups = [
  {
    title: "Surfaces et bordures",
    tokens: named(["bg", "surface", "surface-sunken", "border", "border-strong", "border-control"]),
  },
  {
    title: "Texte, accent, focus, bande",
    tokens: named([
      "text",
      "text-muted",
      "accent",
      "accent-fg",
      "accent-soft",
      "focus-ring",
      "band-bg",
      "band-fg",
    ]),
  },
  {
    title: "Barre latérale",
    tokens: named([
      "sidebar-bg",
      "sidebar-fg",
      "sidebar-active-bg",
      "sidebar-active-fg",
      "sidebar-active-icon",
    ]),
  },
  {
    title: "Environnements",
    tokens: named(["env-1", "env-2", "env-3", "env-4", "env-5", "env-6"]),
  },
  { title: "Différentiel", tokens: named(["diff-added", "diff-removed", "diff-changed"]) },
];
const stateLabels: Record<(typeof STATES)[number], string> = {
  ok: "Conforme",
  warn: "Avertissement",
  ko: "Erreur",
  review: "En revue",
  info: "Brouillon",
  neutral: "Inactif",
};
const pick = (prefix: string) => tokens.filter((token) => token.startsWith(prefix));
const spaces = pick("--k-space-");
const radii = pick("--k-radius-");
const shadows = pick("--k-shadow-");
const sizes = ["xs", "sm", "md", "lg", "xl", "2xl", "mono"] as const;
const heights = named([
  "row-height",
  "header-height",
  "control-height-sm",
  "control-height-md",
  "control-height-lg",
]);
const motion = [
  ...pick("--k-duration-"),
  ...pick("--k-ease-"),
  ...pick("--k-motion-"),
  ...pick("--k-turn-"),
];
const figures = [
  ["10.0.0.0/8", "16 777 216", "12,5 %"],
  ["10.12.0.0/16", "65 536", "48,0 %"],
  ["10.12.144.0/20", "4 096", "91,3 %"],
  ["192.0.2.0/24", "256", "7,1 %"],
  ["2001:db8::/32", "2^96", "0,0 %"],
];
const swatch = (token: string) => ({ "--k-cat-swatch": `var(${token})` });
const size = (token: string) => ({ "--k-cat-size": `var(${token})` });
</script>

<template>
  <section class="k-cat-section" aria-labelledby="ui-tokens-title">
    <h1 id="ui-tokens-title" class="k-cat-title">Jetons</h1>
    <p class="k-cat-lead">
      Source unique : <code>@kartenn/ui/src/tokens/tokens.css</code>. Les valeurs ci-dessous sont
      celles du mode et de la densité choisis.
    </p>

    <template v-for="group in colourGroups" :key="group.title">
      <h2 class="k-cat-subtitle">{{ group.title }}</h2>
      <ul class="k-cat-swatches">
        <li v-for="token in group.tokens" :key="token" class="k-cat-swatch-item">
          <span class="k-cat-swatch" :style="swatch(token)" />
          <code>{{ token }}</code>
          <code class="k-cat-value">{{ values[token] }}</code>
        </li>
      </ul>
    </template>

    <h2 class="k-cat-subtitle">États</h2>
    <table class="k-cat-table">
      <thead>
        <tr>
          <th scope="col">État</th>
          <th scope="col">Texte sur surface</th>
          <th scope="col">Texte sur fond atténué</th>
          <th scope="col">Jetons</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="state in STATES" :key="state" :data-k-state="state">
          <th scope="row">
            <code>{{ state }}</code>
          </th>
          <td class="k-cat-state-fg">{{ stateLabels[state] }}</td>
          <td>
            <span class="k-cat-state-chip">{{ stateLabels[state] }}</span>
          </td>
          <td>
            <code>--k-state-{{ state }}-fg</code>, <code>-bg</code>, <code>-border</code>
          </td>
        </tr>
      </tbody>
    </table>

    <h2 class="k-cat-subtitle">Espacement</h2>
    <ul class="k-cat-scale">
      <li v-for="token in spaces" :key="token" class="k-cat-scale__item">
        <code>{{ token }}</code>
        <span class="k-cat-bar" :style="size(token)" />
        <code class="k-cat-value">{{ values[token] }}</code>
      </li>
    </ul>

    <h2 class="k-cat-subtitle">Rayons et élévation</h2>
    <ul class="k-cat-swatches">
      <li v-for="token in radii" :key="token" class="k-cat-swatch-item">
        <span class="k-cat-radius" :style="size(token)" />
        <code>{{ token }}</code>
        <code class="k-cat-value">{{ values[token] }}</code>
      </li>
      <li v-for="token in shadows" :key="token" class="k-cat-swatch-item">
        <span class="k-cat-shadow" :style="{ '--k-cat-shadow': `var(${token})` }" />
        <code>{{ token }}</code>
      </li>
    </ul>

    <h2 class="k-cat-subtitle">Typographie</h2>
    <ul class="k-cat-type">
      <li v-for="name in sizes" :key="name" :data-k-text="name" class="k-cat-type__item">
        <code>--k-text-{{ name }}</code>
        <span class="k-cat-type__sample">
          {{
            name === "mono"
              ? "10.12.144.0/20 · VID 1042 · 3c:22:fb:01:9a:7e"
              : "Préfixes du domaine Recette — 1 248 adresses"
          }}
        </span>
      </li>
    </ul>
    <p class="k-cat-weights">
      <span class="k-cat-weight" data-k-weight="regular">Normal 400</span>
      <span class="k-cat-weight" data-k-weight="medium">Moyen 500</span>
      <span class="k-cat-weight" data-k-weight="semibold">Demi-gras 600</span>
    </p>

    <h2 class="k-cat-subtitle">Colonne de chiffres</h2>
    <table class="k-cat-table k-cat-figures">
      <caption class="k-cat-note">
        Chiffres tabulaires : les colonnes s'alignent, en chasse fixe comme en sans.
      </caption>
      <thead>
        <tr>
          <th scope="col">Préfixe</th>
          <th scope="col">Adresses</th>
          <th scope="col">Occupation</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="[prefix, count, used] in figures" :key="prefix">
          <td>
            <code>{{ prefix }}</code>
          </td>
          <td class="k-num">{{ count }}</td>
          <td class="k-num">{{ used }}</td>
        </tr>
      </tbody>
    </table>

    <h2 class="k-cat-subtitle">Densité</h2>
    <ul class="k-cat-scale">
      <li v-for="token in heights" :key="token" class="k-cat-scale__item">
        <code>{{ token }}</code>
        <span class="k-cat-height" :style="size(token)" />
        <code class="k-cat-value">{{ values[token] }}</code>
      </li>
    </ul>

    <h2 class="k-cat-subtitle">Mouvement</h2>
    <ul class="k-cat-list">
      <li v-for="token in motion" :key="token">
        <code>{{ token }}</code> : <code class="k-cat-value">{{ values[token] }}</code>
      </li>
    </ul>
  </section>
</template>

<style>
@layer components {
  .k-cat-swatches {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(calc(var(--k-space-12) * 5), 1fr));
    gap: var(--k-space-3);
  }

  .k-cat-swatch-item {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: var(--k-space-0-5) var(--k-space-2);
    align-items: center;
    font-size: var(--k-text-sm);
  }

  .k-cat-swatch-item > :first-child {
    grid-row: span 2;
  }

  .k-cat-swatch {
    inline-size: var(--k-space-8);
    block-size: var(--k-space-8);
    background-color: var(--k-cat-swatch);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-radius-md);
  }

  .k-cat-figures .k-num {
    text-align: end;
  }

  .k-cat-state-fg {
    color: var(--k-cat-state-fg);
  }

  .k-cat-state-chip {
    padding-inline: var(--k-space-2);
    color: var(--k-cat-state-fg);
    background-color: var(--k-cat-state-bg);
    border: var(--k-border-width) solid var(--k-cat-state-border);
    border-radius: var(--k-radius-full);
  }

  [data-k-state="ok"] {
    --k-cat-state-fg: var(--k-state-ok-fg);
    --k-cat-state-bg: var(--k-state-ok-bg);
    --k-cat-state-border: var(--k-state-ok-border);
  }

  [data-k-state="warn"] {
    --k-cat-state-fg: var(--k-state-warn-fg);
    --k-cat-state-bg: var(--k-state-warn-bg);
    --k-cat-state-border: var(--k-state-warn-border);
  }

  [data-k-state="ko"] {
    --k-cat-state-fg: var(--k-state-ko-fg);
    --k-cat-state-bg: var(--k-state-ko-bg);
    --k-cat-state-border: var(--k-state-ko-border);
  }

  [data-k-state="review"] {
    --k-cat-state-fg: var(--k-state-review-fg);
    --k-cat-state-bg: var(--k-state-review-bg);
    --k-cat-state-border: var(--k-state-review-border);
  }

  [data-k-state="info"] {
    --k-cat-state-fg: var(--k-state-info-fg);
    --k-cat-state-bg: var(--k-state-info-bg);
    --k-cat-state-border: var(--k-state-info-border);
  }

  [data-k-state="neutral"] {
    --k-cat-state-fg: var(--k-state-neutral-fg);
    --k-cat-state-bg: var(--k-state-neutral-bg);
    --k-cat-state-border: var(--k-state-neutral-border);
  }

  .k-cat-scale {
    display: grid;
    gap: var(--k-space-1);
  }

  .k-cat-scale__item {
    display: grid;
    grid-template-columns: calc(var(--k-space-12) * 4) auto 1fr;
    gap: var(--k-space-3);
    align-items: center;
    font-size: var(--k-text-sm);
  }

  .k-cat-bar {
    inline-size: var(--k-cat-size);
    block-size: var(--k-space-2);
    background-color: var(--k-accent);
  }

  .k-cat-height {
    inline-size: var(--k-space-12);
    block-size: var(--k-cat-size);
    background-color: var(--k-accent-soft);
    border: var(--k-border-width) solid var(--k-accent);
  }

  .k-cat-radius,
  .k-cat-shadow {
    inline-size: var(--k-space-12);
    block-size: var(--k-space-8);
    background-color: var(--k-surface);
    border: var(--k-border-width) solid var(--k-border-strong);
    border-radius: var(--k-cat-size, var(--k-radius-lg));
  }

  .k-cat-shadow {
    /* stylelint-disable-next-line kartenn/floating-shadow -- the swatch of the token */
    box-shadow: var(--k-cat-shadow);
  }

  .k-cat-type {
    display: grid;
    gap: var(--k-space-2);
  }

  .k-cat-type__item {
    display: grid;
    grid-template-columns: calc(var(--k-space-12) * 3) 1fr;
    gap: var(--k-space-4);
    align-items: baseline;
  }

  .k-cat-type__item > code {
    font-size: var(--k-text-sm);
    color: var(--k-text-muted);
  }

  [data-k-text="xs"] .k-cat-type__sample {
    font-size: var(--k-text-xs);
    line-height: var(--k-text-xs-line);
  }

  [data-k-text="sm"] .k-cat-type__sample {
    font-size: var(--k-text-sm);
    line-height: var(--k-text-sm-line);
  }

  [data-k-text="md"] .k-cat-type__sample {
    font-size: var(--k-text-md);
    line-height: var(--k-text-md-line);
  }

  [data-k-text="lg"] .k-cat-type__sample {
    font-size: var(--k-text-lg);
    line-height: var(--k-text-lg-line);
  }

  [data-k-text="xl"] .k-cat-type__sample {
    font-size: var(--k-text-xl);
    line-height: var(--k-text-xl-line);
  }

  [data-k-text="2xl"] .k-cat-type__sample {
    font-size: var(--k-text-2xl);
    line-height: var(--k-text-2xl-line);
  }

  [data-k-text="mono"] .k-cat-type__sample {
    font-family: var(--k-font-mono);
    font-size: var(--k-text-mono);
    line-height: var(--k-text-mono-line);
  }

  .k-cat-weights {
    display: flex;
    gap: var(--k-space-6);
  }

  [data-k-weight="regular"] {
    font-weight: var(--k-weight-regular);
  }

  [data-k-weight="medium"] {
    font-weight: var(--k-weight-medium);
  }

  [data-k-weight="semibold"] {
    font-weight: var(--k-weight-semibold);
  }
}
</style>
