<script setup lang="ts">
// E0 — Accueil (design « Maquettes », 024/M10; 021, « Écrans » : Accueil): a dashboard of
// tiles on `Grid`. Tiles of the core first (objects by kind, changesets applied over 7 and
// 30 days with their histogram by day, `BarChart`, to apply, mine pending, open
// anomalies), fed by the fixture of `GET /v1/stats`; then the `widget` tiles of two plugins
// (VLAN with the occupation of its groups in a `BarList`, Flux), and the one of a third
// still loading (Politique). The instant of the snapshot at the foot.
import {
  BarChart,
  BarList,
  Button,
  Grid,
  Skeleton,
  Stack,
  StatusBadge,
  type ChartPoint,
} from "@kartenn/ui";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { KIND_LABELS, STATS } from "./donnees.ts";

defineProps<{ vue: string }>();

const number = new Intl.NumberFormat("fr-FR");
const kinds = (Object.keys(STATS.objects) as (keyof typeof STATS.objects)[]).map((kind) => ({
  key: kind,
  label: KIND_LABELS[kind],
  value: number.format(STATS.objects[kind]),
}));

/** The days of the histogram, today last (UTC days of the fixture). */
const DAY = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short", timeZone: "UTC" });
const days: ChartPoint[] = STATS.changesets.by_day.map((value, i, all) => {
  const date = new Date(Date.UTC(2026, 0, 15 - (all.length - 1 - i)));
  return { key: date.toISOString().slice(0, 10), label: DAY.format(date), value };
});

/** The occupation of the VLAN groups, as the widget of the plugin counts it. */
const groups: ChartPoint[] = STATS.modules.vlan.groups.map((g) => ({
  key: g.key,
  label: g.key,
  value: g.used,
}));
const percent = (value: number) => `${value} %`;
</script>

<template>
  <Cadre title="Accueil" active="home" space="referential" :crumbs="[{ label: 'Accueil' }]">
    <Grid columns="auto" gap="3" class="k-ecran-tiles">
      <Panneau title="Objets" detail="lisibles par vous">
        <Attributs :items="kinds.map((k) => ({ ...k, mono: true }))" />
      </Panneau>
      <Panneau title="Changesets appliqués" detail="tous auteurs" class="k-ecran-tile-wide">
        <dl class="k-ecran-figures">
          <div>
            <dt>7 derniers jours</dt>
            <dd class="k-num">{{ STATS.changesets.applied_7d }}</dd>
          </div>
          <div>
            <dt>30 derniers jours</dt>
            <dd class="k-num">{{ STATS.changesets.applied_30d }}</dd>
          </div>
        </dl>
        <BarChart
          label="Changesets appliqués par jour, sur 30 jours"
          :points="days"
          point-label="Jour (UTC)"
          value-label="Changesets appliqués"
        />
      </Panneau>
      <Panneau title="À instruire" detail="changesets d'autres auteurs">
        <dl class="k-ecran-figures">
          <div>
            <dt>En attente de votre validation</dt>
            <dd class="k-num">{{ STATS.pending.to_apply }}</dd>
          </div>
        </dl>
        <template #footer>
          <Button size="sm" icon="git-pull-request">Instruire</Button>
        </template>
      </Panneau>
      <Panneau title="Mes changesets" detail="en attente">
        <dl class="k-ecran-figures">
          <div>
            <dt>Soumis, en revue</dt>
            <dd class="k-num">{{ STATS.pending.mine }}</dd>
          </div>
        </dl>
        <template #footer>
          <Button size="sm" variant="ghost" icon="changeset">Voir mes changesets</Button>
        </template>
      </Panneau>
      <Panneau title="Anomalies ouvertes">
        <dl class="k-ecran-figures">
          <div>
            <dt>Sur les objets que vous lisez</dt>
            <dd class="k-num">{{ STATS.anomalies.open }}</dd>
          </div>
        </dl>
        <StatusBadge state="warn" label="3 nouvelles depuis hier" />
        <template #footer>
          <Button size="sm" variant="ghost" icon="triangle-alert">Voir les anomalies</Button>
        </template>
      </Panneau>
      <Panneau title="VLAN" detail="plugin VLAN">
        <Attributs
          :items="[
            { key: 'reserved', label: 'Réservés', value: String(STATS.modules.vlan.reserved) },
            {
              key: 'deprecated',
              label: 'Obsolètes',
              value: String(STATS.modules.vlan.deprecated),
            },
          ]"
        />
        <p class="k-ecran-muted">Occupation des groupes</p>
        <BarList
          label="Occupation des groupes de VLAN"
          :items="groups"
          :max="100"
          mono
          :format="percent"
          point-label="Groupe"
          value-label="Occupation"
        />
      </Panneau>
      <Panneau title="Flux" detail="plugin Flux">
        <Attributs
          :items="[
            {
              key: 'noncompliant',
              label: 'Non conformes',
              value: String(STATS.modules.flow.noncompliant),
            },
            {
              key: 'waivers',
              label: 'Dérogations finissant sous 30 jours',
              value: String(STATS.modules.flow.waivers_expiring),
            },
          ]"
        />
      </Panneau>
      <Panneau title="Politique" detail="plugin Politique">
        <Stack gap="2" aria-busy="true">
          <span class="k-visually-hidden">Chargement de la tuile Politique</span>
          <Skeleton variant="text" :rows="3" />
        </Stack>
      </Panneau>
    </Grid>
    <p class="k-ecran-muted">
      Compteurs calculés
      <time datetime="2026-01-15T08:58:00Z">il y a 2 min</time>, relus à chaque changement.
    </p>
  </Cadre>
</template>

<style>
@layer components {
  /* The histogram takes two tracks; the smaller tiles fill the holes it leaves. */
  .k-ecran-tiles {
    grid-auto-flow: dense;
  }

  .k-ecran-tiles > .k-ecran-tile-wide {
    grid-column: span 2;
  }

  /* A surface too narrow for two tiles: the histogram in the one column. */
  @container k-surface (width < 640px) {
    .k-ecran-tiles > .k-ecran-tile-wide {
      grid-column: auto;
    }
  }

  /* Large counters with their term above. */
  .k-ecran-figures {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-6);
  }

  .k-ecran-figures dt {
    color: var(--k-text-muted);
  }

  .k-ecran-figures dd {
    font-size: var(--k-text-2xl);
    line-height: var(--k-text-2xl-line);
    font-weight: var(--k-weight-semibold);
  }
}
</style>
