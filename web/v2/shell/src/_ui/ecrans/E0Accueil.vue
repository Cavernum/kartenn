<script setup lang="ts">
// E0 — Accueil (design « Maquettes », 024/M10; 021, « Écrans » : Accueil): a dashboard of
// tiles on `Grid`. Tiles of the core first (objects by kind, changesets applied over 7 and
// 30 days with their histogram by day, to apply, mine pending, open anomalies), fed by the
// fixture of `GET /v1/stats`; then the `widget` tiles of two plugins (VLAN, Flux), and the
// one of a third still loading (Politique). The instant of the snapshot at the foot.
import { Button, Grid, Skeleton, Stack, StatusBadge } from "@kartenn/ui";
import AVenir from "./AVenir.vue";
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
const peak = Math.max(...STATS.changesets.by_day);
const days = STATS.changesets.by_day.map((count, i, all) => {
  const date = new Date(Date.UTC(2026, 0, 15 - (all.length - 1 - i)));
  return {
    key: i,
    label: DAY.format(date),
    count,
    height: { "--k-ecran-bar": String(count / peak) },
  };
});
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
        <AVenir component="BarChart" task="à planifier">
          <ol class="k-ecran-bars" aria-label="Changesets appliqués par jour, sur 30 jours">
            <li v-for="day in days" :key="day.key" class="k-ecran-bars__day" :style="day.height">
              <span class="k-visually-hidden">{{ day.label }} : {{ day.count }}</span>
            </li>
          </ol>
        </AVenir>
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
            { key: 'group', label: 'Groupe le plus rempli', value: 'campus-ouest, 92 %' },
          ]"
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

  /* Stand-in of the bar chart to come: one bar per day, its height set by CSSOM. */
  .k-ecran-bars {
    display: flex;
    gap: var(--k-space-0-5);
    align-items: end;
    block-size: var(--k-space-12);
    list-style: none;
  }

  .k-ecran-bars__day {
    flex: 1;
    min-inline-size: 0;
    block-size: calc(var(--k-space-12) * var(--k-ecran-bar, 0));
    min-block-size: var(--k-space-px);
    background-color: var(--k-accent);
  }
}
</style>
