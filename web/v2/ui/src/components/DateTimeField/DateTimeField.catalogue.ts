// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Stack from "../Stack/Stack.vue";
import TextField from "../TextField/TextField.vue";
import DateTimeField from "./DateTimeField.vue";

const ZONE = "Europe/Paris";

/**
 * The fields of the keyboard script (e2e/components/date-time-field.spec.ts): a field
 * before them to enter by Tab, an instant in a named zone and a day, each value written
 * under it as the host receives it.
 */
const Keyboard = defineComponent({
  setup() {
    const note = ref("");
    const instant = ref<string | null>(null);
    const day = ref<string | null>(null);
    const shown = (value: string | null) => value ?? "—";
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(TextField, {
          label: "Note",
          modelValue: note.value,
          "onUpdate:modelValue": (next: string) => (note.value = next),
        }),
        h(DateTimeField, {
          label: "Valide au",
          timeZone: ZONE,
          hint: "Date et heure de Paris.",
          modelValue: instant.value,
          "onUpdate:modelValue": (next: string | null) => (instant.value = next),
        }),
        h("p", { class: "k-cat-note" }, [
          "Valide au : ",
          h("code", { "data-value": "instant" }, shown(instant.value)),
        ]),
        h(DateTimeField, {
          label: "Créé le",
          granularity: "day",
          modelValue: day.value,
          "onUpdate:modelValue": (next: string | null) => (day.value = next),
        }),
        h("p", { class: "k-cat-note" }, [
          "Créé le : ",
          h("code", { "data-value": "day" }, shown(day.value)),
        ]),
      ]);
  },
});

export default {
  component: DateTimeField,
  pattern: "Spinbutton par segment (ISO 8601) et Date Picker Dialog",
  states: [
    {
      name: "empty",
      description: "Vide : les segments en attente, le fuseau nommé et son décalage",
      props: { label: "Valide au", timeZone: ZONE, hint: "Vide : maintenant." },
    },
    {
      name: "filled",
      description: "Un instant (UTC) lu dans le fuseau de Paris : 09:30 à UTC+01:00",
      props: { label: "Valide au", timeZone: ZONE, modelValue: "2025-11-03T08:30:00Z" },
    },
    {
      name: "summer",
      description: "Le même fuseau en été : décalage UTC+02:00",
      props: { label: "Valide au", timeZone: ZONE, modelValue: "2025-07-14T06:00:00Z" },
    },
    {
      name: "day",
      description: "Un jour seul (AAAA-MM-JJ) : ni heure ni fuseau",
      props: { label: "Créé le", granularity: "day", modelValue: "2026-01-15" },
    },
    {
      name: "small",
      description: "Taille sm (éditeur de cellule, barre) : le bouton tient dans le cadre",
      props: { label: "Valide au", timeZone: ZONE, size: "sm", modelValue: "2025-11-03T08:30:00Z" },
    },
    {
      name: "invalid",
      description: "31 février : gardé tel que saisi, refusé, le motif lié et le segment marqué",
      props: { label: "Valide au", timeZone: ZONE, defaultEntry: "2026-02-31 10:30" },
    },
    {
      name: "hour",
      description: "25 h : refusée, les bornes de l'heure dites",
      props: { label: "Valide au", timeZone: ZONE, defaultEntry: "2026-03-12 25:00" },
    },
    {
      name: "gap",
      description: "L'heure sautée au passage à l'heure d'été (Paris, 29 mars 2026, 02:30)",
      props: { label: "Valide au", timeZone: ZONE, defaultEntry: "2026-03-29 02:30" },
    },
    {
      name: "incomplete",
      description: "Incomplète au départ du champ : ce qui manque dit",
      props: { label: "Valide au", timeZone: ZONE, defaultEntry: "2026-01-15" },
    },
    {
      name: "readonly",
      description: "Lecture seule : bordure en tirets, sans calendrier",
      props: {
        label: "Connu au",
        timeZone: ZONE,
        readonly: true,
        modelValue: "2025-11-03T08:30:00Z",
      },
    },
    {
      name: "disabled",
      description: "Désactivé : hors de l'ordre de tabulation",
      props: { label: "Valide au", timeZone: ZONE, disabled: true },
    },
    {
      name: "focus",
      description: "Focus clavier sur un segment : rempli de l'accent, le cadre entouré",
      props: { label: "Valide au", timeZone: ZONE, modelValue: "2025-11-03T08:30:00Z" },
      interaction: "focus",
      target: "[role='spinbutton']",
    },
    {
      name: "calendar",
      description: "Calendrier ouvert (dialogue modal) : le jour choisi, aujourd'hui entouré",
      props: {
        label: "Valide au",
        timeZone: ZONE,
        modelValue: "2026-01-20T08:30:00Z",
        defaultOpen: true,
      },
      isolated: true,
    },
    {
      name: "keyboard",
      description: "Au clavier : saisie par segments, calendrier, refus ; la valeur écrite",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
