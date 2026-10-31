// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Button from "../Button/Button.vue";
import Cluster from "../Cluster/Cluster.vue";
import type { Period } from "../DateTimeField/dates.ts";
import Stack from "../Stack/Stack.vue";
import DatePill from "./DatePill.vue";

const ZONE = "Europe/Paris";
const PAST = "2025-11-03T08:30:00Z";
const KNOWN = "2025-12-01T07:00:00Z";
/** The clock of the demo's server, five minutes behind the browser's. */
const SKEW = 5 * 60 * 1000;
const REFUSED = "Refusé par le serveur : cette date dépasse son heure.";

/**
 * The pill of the keyboard script (e2e/components/date-pill.spec.ts) in a bar, a control
 * after it: the period applied is read by the host (written under it as the query of the
 * URL, 021 `valid_at` and `as_of`), unless its server refuses an `as_of` past its own
 * clock (a 400, its reason back on the field).
 */
const Keyboard = defineComponent({
  setup() {
    const period = ref<Period>({ validAt: null, asOf: null });
    const errors = ref<Partial<Record<keyof Period, string>>>();
    const query = () => {
      const params = new URLSearchParams();
      if (period.value.validAt) params.set("valid_at", period.value.validAt);
      if (period.value.asOf) params.set("as_of", period.value.asOf);
      return decodeURIComponent(params.toString()) || "(présent)";
    };
    const onApply = (next: Period) => {
      if (next.asOf && Date.parse(next.asOf) > Date.now() - SKEW) {
        errors.value = { asOf: REFUSED };
        return;
      }
      errors.value = undefined;
      period.value = next;
    };
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(Cluster, { gap: "2" }, () => [
          h(DatePill, {
            validAt: period.value.validAt,
            asOf: period.value.asOf,
            timeZone: ZONE,
            errors: errors.value,
            onApply,
          }),
          h(Button, { variant: "secondary" }, () => "Après la pastille"),
        ]),
        h("p", { class: "k-cat-note" }, ["URL : ", h("code", { "data-query": "" }, query())]),
      ]);
  },
});

export default {
  component: DatePill,
  pattern: "Button qui ouvre un Dialog non modal (le choix de la période)",
  states: [
    { name: "present", description: "Présent", props: { timeZone: ZONE } },
    {
      name: "past",
      description: "Hors présent : couleurs de la bande, fuseau nommé",
      props: { validAt: PAST, timeZone: ZONE },
    },
    {
      name: "known",
      description: "État connu à une date passée (as_of) : cadenas, « Connu au »",
      props: { asOf: KNOWN, timeZone: ZONE },
    },
    {
      name: "both",
      description: "Les deux dates",
      props: { validAt: PAST, asOf: KNOWN, timeZone: ZONE },
    },
    {
      name: "hover",
      description: "Survol",
      props: { timeZone: ZONE },
      interaction: "hover",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { validAt: PAST, timeZone: ZONE },
      interaction: "focus",
    },
    {
      name: "open",
      description:
        "Ouverte : le choix de la période sous la pastille, nommé par son titre, le focus sur « Valide au »",
      props: { validAt: PAST, timeZone: ZONE, defaultOpen: true },
      isolated: true,
    },
    {
      name: "host",
      description: "Refus de l'API (400 sur as_of) rendu sur son champ, la couche ouverte",
      props: {
        asOf: KNOWN,
        timeZone: ZONE,
        defaultOpen: true,
        errors: { asOf: REFUSED },
      },
      isolated: true,
    },
    {
      name: "keyboard",
      description:
        "Au clavier : ouvrir, appliquer, refus du serveur sur son champ, sortir par Tab ou Maj+Tab",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
