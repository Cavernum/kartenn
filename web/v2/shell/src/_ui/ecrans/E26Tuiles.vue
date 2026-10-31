<script setup lang="ts">
// E26 — Tuiles de l'Accueil (spec 017, task 1; points `widget` of the two plugins,
// REQ-017-35; 021/X38): at most one read-only tile per plugin on the home page, after the
// tiles of the core (E0). `policy-home`: open inference mismatches and the last audit
// report; `flow-home`: requests to review (with `flow.review`), deadlines within 7 days,
// the user's requests pending. Each line leads to its view; a line, or a tile, the user
// cannot open is not shown; the counts are read again by the event stream, never polled.
import { Grid, StatusBadge } from "@kartenn/ui";
import { RouterLink, useRoute } from "vue-router";
import Attributs from "./EcranAttributs.vue";
import Cadre from "./EcranCadre.vue";
import Panneau from "./EcranPanneau.vue";
import { SPACES_017, lien } from "./donnees017.ts";

const props = defineProps<{ vue: string }>();
const route = useRoute();
const officer = props.vue === "responsable";
const to = (ecran: string, vue: string) => lien(ecran, vue, route.query);
</script>

<template>
  <Cadre
    title="Accueil"
    active="home"
    space="referential"
    :spaces="SPACES_017"
    :draft="0"
    :crumbs="[{ label: 'Accueil' }]"
  >
    <p class="k-ecran-muted">Les tuiles du cœur (E0) précèdent celles des plugins.</p>
    <Grid columns="auto" gap="3">
      <Panneau v-if="officer" title="Politique" detail="plugin Politique">
        <Attributs
          :items="[
            { key: 'mismatches', label: 'Écarts d\'inférence ouverts' },
            { key: 'report', label: 'Dernier rapport d\'audit' },
            { key: 'running', label: 'En cours' },
          ]"
        >
          <template #mismatches
            ><RouterLink :to="to('E18', 'ecarts')" class="k-num">4</RouterLink></template
          >
          <template #report>
            <span class="k-e26-line"
              ><RouterLink :to="to('E20', 'rapport')" class="k-mono">rpt-0041</RouterLink
              ><StatusBadge state="ok" label="Fait" /><span class="k-ecran-muted k-num"
                >au 2026-01-01</span
              ></span
            >
          </template>
          <template #running>
            <span class="k-e26-line"
              ><RouterLink :to="to('E20', 'liste')" class="k-mono">rpt-0042</RouterLink
              ><StatusBadge state="info" label="En cours"
            /></span>
          </template>
        </Attributs>
      </Panneau>
      <Panneau title="Flux" detail="plugin Flux">
        <Attributs
          :items="[
            ...(officer ? [{ key: 'review', label: 'À instruire' }] : []),
            { key: 'expiring', label: 'Échéances sous 7 jours' },
            { key: 'mine', label: 'Mes demandes en attente' },
          ]"
        >
          <template #review
            ><RouterLink :to="to('E23', 'file')" class="k-num">3</RouterLink></template
          >
          <template #expiring>
            <span class="k-e26-line"
              ><RouterLink :to="to('E24', 'echeances')" class="k-num">2</RouterLink
              ><StatusBadge state="warn" label="J-5 au plus tôt"
            /></span>
          </template>
          <template #mine
            ><RouterLink :to="to('E21', 'instruction')" class="k-num">1</RouterLink></template
          >
        </Attributs>
      </Panneau>
    </Grid>
    <p class="k-ecran-muted">
      {{
        officer
          ? "Vu avec policy.catalogue.read, policy.rule.admin et flow.review."
          : "Vu par un demandeur (flow.request, sans flow.review ni policy.catalogue.read) : ni ligne « À instruire », ni tuile Politique."
      }}
      Relues à chaque changement, par le flux d'événements.
    </p>
  </Cadre>
</template>

<style>
@layer components {
  .k-e26-line {
    display: inline-flex;
    flex-wrap: wrap;
    gap: var(--k-space-2);
    align-items: center;
  }
}
</style>
