<script setup lang="ts">
// The home page (REQ-021-9; design 021 « Écrans » : Accueil; maquette E0): a dashboard of
// tiles on `Grid`, fed by `GET /v1/stats` (each counter filtered by the reader's rights, a
// kind read nowhere absent, the changesets without `core.changeset.read`), read again on
// the event `stats`. The tiles of the core: objects by kind, changesets applied over 7 and
// 30 days with their histogram by day (`BarChart`), to review, mine pending, open anomalies,
// the VLAN; then the `widget` tile of each plugin (`useWidgets`, task 16), its error kept in
// its tile. The instant of the computation at the foot. Three states, for the counters as a
// whole: they are one read.
import { stats as statsQuery, type components } from "@kartenn/sdk";
import { BarChart, Grid, Skeleton, type ChartPoint } from "@kartenn/ui";
import { computed, onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useQuery } from "../data/query.ts";
import ExtensionElement from "../plugins/ExtensionElement.vue";
import { labelOf } from "../plugins/registry.ts";
import { placeId, useWidgets } from "../plugins/slots.ts";
import KLink from "../prefetch/KLink.vue";
import { serverSkew } from "../session/expiry.ts";
import { useSession } from "../rights/index.ts";
import { COLLECTIONS } from "./collections.ts";
import ProblemState from "../problems/ProblemState.vue";
import { useKeyLabel } from "./data.ts";
import AttributeList from "./parts/AttributeList.vue";
import ViewPanel from "./parts/ViewPanel.vue";

type Stats = components["schemas"]["Stats"];

const { t, locale } = useI18n();
const session = useSession();
const stats = useQuery(() => statsQuery.query());
const data = computed(() => stats.data.value?.data as Stats | undefined);

const number = (value: number) => new Intl.NumberFormat(locale.value).format(value);
/** The entries of the navigation that name each kind of the core (`core.prefix` → Préfixes). */
const KIND_ENTRIES: Record<string, string> = {
  ...Object.fromEntries(COLLECTIONS.map((c) => [c.type, c.id])),
  "core.vlan": "vlans",
  "core.vlan_group": "vlanGroups",
};
const kinds = computed(() =>
  Object.entries(data.value?.objects ?? {}).map(([kind, count]) => ({
    key: kind,
    // A kind the navigation does not name: its label of the dictionary, else its key.
    label: KIND_ENTRIES[kind] ? t(`nav.${KIND_ENTRIES[kind]}`) : (keyLabel("kind", kind) ?? kind),
    value: number(count),
    mono: true,
  })),
);

/** The days of the histogram, today last (UTC days, as the server counts them). */
const days = computed<ChartPoint[]>(() => {
  const counted = data.value?.changesets?.by_day ?? [];
  const at = data.value ? new Date(data.value.computed_at) : new Date();
  const day = new Intl.DateTimeFormat(locale.value, {
    day: "numeric",
    month: "short",
    timeZone: "UTC",
  });
  return counted.map((value, i) => {
    const date = new Date(
      Date.UTC(at.getUTCFullYear(), at.getUTCMonth(), at.getUTCDate() - (counted.length - 1 - i)),
    );
    return { key: date.toISOString().slice(0, 10), label: day.format(date), value };
  });
});

// « il y a 2 min », kept current while the page stays open.
const now = ref(Date.now());
const tick = setInterval(() => (now.value = Date.now()), 30_000);
onBeforeUnmount(() => clearInterval(tick));
const computedAgo = computed(() => {
  if (!data.value) return "";
  // `computed_at` is the server's instant: read on its clock (the `Date` of its answers).
  const seconds = Math.round(
    (Date.parse(data.value.computed_at) - (now.value + serverSkew())) / 1000,
  );
  const words = new Intl.RelativeTimeFormat(locale.value, { numeric: "auto" });
  return Math.abs(seconds) < 60
    ? words.format(Math.min(0, seconds), "second")
    : Math.abs(seconds) < 3600
      ? words.format(Math.round(seconds / 60), "minute")
      : words.format(Math.round(seconds / 3600), "hour");
});
const reviews = computed(() => session.value.rights.has("core.changeset.read"));
const widgets = useWidgets();
const keyLabel = useKeyLabel();
const tr = (key: string) => t(key);
</script>

<template>
  <h1 class="k-visually-hidden">{{ t("nav.home") }}</h1>
  <ProblemState
    v-if="stats.isError.value"
    :title="t('views.home.error')"
    :error="stats.error.value"
    :retry="stats.refetch"
  >
    {{ t("views.errorText") }}
  </ProblemState>
  <!-- One skeleton, said once (its own status region), of the height of a row of tiles. -->
  <Skeleton v-else-if="stats.isPending.value" variant="text" :rows="6" />
  <template v-else-if="data">
    <Grid columns="auto" gap="3" class="k-home-tiles">
      <ViewPanel :title="t('views.home.objects')" :detail="t('views.home.readable')">
        <AttributeList :items="kinds" />
      </ViewPanel>
      <ViewPanel
        v-if="data.changesets"
        :title="t('views.home.applied')"
        :detail="t('views.home.allAuthors')"
        class="k-home-tile-wide"
      >
        <dl class="k-home-figures">
          <div>
            <dt>{{ t("views.home.days7") }}</dt>
            <dd class="k-num">{{ number(data.changesets.applied_7d) }}</dd>
          </div>
          <div>
            <dt>{{ t("views.home.days30") }}</dt>
            <dd class="k-num">{{ number(data.changesets.applied_30d) }}</dd>
          </div>
        </dl>
        <BarChart
          :label="t('views.home.chart')"
          :points="days"
          :point-label="t('views.home.chartDay')"
          :value-label="t('views.home.chartValue')"
        />
      </ViewPanel>
      <ViewPanel
        v-if="reviews"
        :title="t('views.home.toReview')"
        :detail="t('views.home.toReviewDetail')"
      >
        <dl class="k-home-figures">
          <div>
            <dt>{{ t("views.home.toApply") }}</dt>
            <dd class="k-num">{{ number(data.pending.to_apply) }}</dd>
          </div>
        </dl>
        <template #footer>
          <KLink to="/proposals">{{ t("views.home.review") }}</KLink>
        </template>
      </ViewPanel>
      <ViewPanel :title="t('views.home.mine')" :detail="t('views.home.mineDetail')">
        <dl class="k-home-figures">
          <div>
            <dt>{{ t("views.home.inReview") }}</dt>
            <dd class="k-num">{{ number(data.pending.mine) }}</dd>
          </div>
        </dl>
        <template v-if="reviews" #footer>
          <KLink to="/changesets">{{ t("views.home.myChangesets") }}</KLink>
        </template>
      </ViewPanel>
      <ViewPanel :title="t('views.home.anomalies')">
        <dl class="k-home-figures">
          <div>
            <dt>{{ t("views.home.anomaliesOpen") }}</dt>
            <dd class="k-num">{{ number(data.anomalies.open) }}</dd>
          </div>
        </dl>
        <template #footer>
          <KLink to="/anomalies">{{ t("views.home.seeAnomalies") }}</KLink>
        </template>
      </ViewPanel>
      <ViewPanel v-if="data.vlans" :title="t('nav.vlans')" :detail="t('views.home.readable')">
        <AttributeList
          :items="[
            {
              key: 'reserved',
              label: t('views.home.vlanReserved'),
              value: number(data.vlans.reserved),
              mono: true,
            },
            {
              key: 'deprecated',
              label: t('views.home.vlanDeprecated'),
              value: number(data.vlans.deprecated),
              mono: true,
            },
          ]"
        />
      </ViewPanel>
      <!-- The tiles of the plugins after the core's (REQ-021-13): an error stays in its tile,
           a plugin still loading has its skeleton. -->
      <ViewPanel
        v-for="widget in widgets"
        :key="placeId(widget.extension)"
        :title="labelOf(tr, widget.extension)"
        :detail="t('views.home.plugin', { name: widget.extension.plugin })"
      >
        <ExtensionElement
          :tag="widget.extension.element"
          :plugin="widget.extension.plugin"
          :label="labelOf(tr, widget.extension)"
          :status="widget.status"
        />
      </ViewPanel>
    </Grid>
    <p class="k-home-muted">
      {{ t("views.home.computed") }}
      <time :datetime="data.computed_at">{{ computedAgo }}</time
      >{{ t("views.home.reread") }}
    </p>
  </template>
</template>

<style>
@layer components {
  /* The histogram takes two tracks; the smaller tiles fill the holes it leaves. */
  .k-home-tiles {
    grid-auto-flow: dense;
  }

  .k-home-tiles > .k-home-tile-wide {
    grid-column: span 2;
  }

  /* A surface too narrow for two tiles: the histogram in the one column. */
  @container k-surface (width < 640px) {
    .k-home-tiles > .k-home-tile-wide {
      grid-column: auto;
    }
  }

  /* Large counters with their term above. */
  .k-home-figures {
    display: flex;
    flex-wrap: wrap;
    gap: var(--k-space-6);
  }

  .k-home-figures dt,
  .k-home-muted {
    color: var(--k-text-muted);
  }

  .k-home-figures dd {
    font-size: var(--k-text-2xl);
    line-height: var(--k-text-2xl-line);
    font-weight: var(--k-weight-semibold);
  }
}
</style>
