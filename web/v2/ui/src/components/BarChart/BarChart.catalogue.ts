// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import BarChart from "./BarChart.vue";
import type { ChartPoint } from "./chart.ts";

/** Thirty UTC days ending on the 15th of January 2026, with quiet weekends. */
const COUNTS = [
  4, 6, 3, 0, 0, 5, 7, 4, 6, 2, 0, 0, 8, 5, 3, 6, 4, 0, 0, 7, 9, 5, 3, 6, 0, 0, 8, 6, 11, 7,
];
const DAY = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short", timeZone: "UTC" });
const days = (counts: number[]): ChartPoint[] =>
  counts.map((value, i) => {
    const date = new Date(Date.UTC(2026, 0, 15 - (counts.length - 1 - i)));
    return { key: date.toISOString().slice(0, 10), label: DAY.format(date), value };
  });

const common = {
  label: "Changesets appliqués par jour",
  pointLabel: "Jour",
  valueLabel: "Appliqués",
};

export default {
  component: BarChart,
  pattern: "role img nommé par un résumé, table de données cachée",
  states: [
    {
      name: "days",
      description: "Trente jours, un pic",
      props: { ...common, points: days(COUNTS) },
      wide: true,
    },
    {
      name: "week",
      description: "Sept jours : barres larges",
      props: { ...common, points: days(COUNTS.slice(-7)) },
      wide: true,
    },
    {
      name: "hover",
      description: "Une barre survolée : son jour et sa valeur en infobulle",
      props: { ...common, points: days(COUNTS.slice(-7)) },
      wide: true,
      interaction: "hover",
      target: ".k-bar-chart__bar:last-of-type .k-bar-chart__hit",
    },
    {
      name: "empty",
      description: "Série nulle : la ligne de base et l'échelle à 1",
      props: { ...common, points: days([0, 0, 0, 0, 0, 0, 0]) },
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
