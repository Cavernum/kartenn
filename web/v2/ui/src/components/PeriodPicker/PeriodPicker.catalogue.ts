// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import type { Period } from "../DateTimeField/dates.ts";
import Stack from "../Stack/Stack.vue";
import PeriodPicker from "./PeriodPicker.vue";

const ZONE = "Europe/Paris";

/**
 * The picker of the keyboard script (e2e/components/period-picker.spec.ts): the period
 * applied is written under it as the query of the URL (021, `valid_at` and `as_of`), and
 * given back to the picker as the host's.
 */
const Keyboard = defineComponent({
  setup() {
    const period = ref<Period>({ validAt: null, asOf: null });
    const query = () => {
      const params = new URLSearchParams();
      if (period.value.validAt) params.set("valid_at", period.value.validAt);
      if (period.value.asOf) params.set("as_of", period.value.asOf);
      return decodeURIComponent(params.toString()) || "(présent)";
    };
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(PeriodPicker, {
          validAt: period.value.validAt,
          asOf: period.value.asOf,
          timeZone: ZONE,
          onApply: (next: Period) => (period.value = next),
        }),
        h("p", { class: "k-cat-note" }, ["URL : ", h("code", { "data-query": "" }, query())]),
      ]);
  },
});

export default {
  component: PeriodPicker,
  pattern: "Formulaire de deux DateTimeField (valid_at, as_of) et raccourcis",
  states: [
    {
      name: "present",
      description: "Au présent : les deux dates vides, le fuseau nommé",
      props: { timeZone: ZONE },
    },
    {
      name: "past",
      description: "Hors présent : valide au 3 novembre 2025, « Revenir au présent » offert",
      props: { timeZone: ZONE, validAt: "2025-11-03T08:30:00Z" },
    },
    {
      name: "both",
      description: "Les deux axes : valide au 3 novembre, tel que connu le 1er décembre",
      props: { timeZone: ZONE, validAt: "2025-11-03T08:30:00Z", asOf: "2025-12-01T07:00:00Z" },
    },
    {
      name: "refused",
      description: "« Connu au » dans l'avenir : refusé à l'envoi, le motif lié au champ",
      props: { timeZone: ZONE, asOf: "2027-06-01T08:00:00Z" },
      interaction: "press",
      target: "button[type='submit']",
    },
    {
      name: "keyboard",
      description: "Au clavier : saisie, raccourcis, refus, envoi ; l'URL écrite",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
