// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import Checkbox from "../Checkbox/Checkbox.vue";
import Stack from "../Stack/Stack.vue";
import Switch from "../Switch/Switch.vue";
import FormField from "./FormField.vue";

const COLUMNS = ["Préfixe", "Usage", "Propriétaire"] as const;

/**
 * A group of checkboxes in a `FormField` (`group`): named by its label, described by its
 * error and its help; `required` asks for one column at least, and the error comes (said)
 * as soon as none is left (the keyboard script, e2e/components/text-field.spec.ts).
 */
const Columns = defineComponent({
  props: { initial: { type: Array<string>, default: () => ["Préfixe", "Usage"] } },
  setup(props) {
    const chosen = ref<string[]>([...props.initial]);
    return () =>
      h(
        FormField,
        {
          label: "Colonnes de l'export",
          group: true,
          required: true,
          hint: "Dans l'ordre de la liste.",
          error: chosen.value.length ? undefined : "Choisissez au moins une colonne.",
        },
        () =>
          h(Stack, { gap: "0" }, () =>
            COLUMNS.map((column) =>
              h(Checkbox, {
                label: column,
                modelValue: chosen.value.includes(column),
                "onUpdate:modelValue": (on: boolean | "mixed") =>
                  (chosen.value = on
                    ? COLUMNS.filter((c) => c === column || chosen.value.includes(c))
                    : chosen.value.filter((c) => c !== column)),
              }),
            ),
          ),
      );
  },
});

/** Two binary settings in a group, with their help. */
const Settings = () =>
  h(FormField, { label: "Affichage", group: true, hint: "Pris en compte aussitôt." }, () =>
    h(Stack, { gap: "0" }, () => [
      h(Switch, { label: "Montrer les adresses réservées", modelValue: true }),
      h(Switch, { label: "Montrer les préfixes vides" }),
    ]),
  );

export default {
  component: FormField,
  pattern: "étiquette, erreur puis aide par aria-describedby ; group nommé (cases, bascules)",
  states: [
    {
      name: "group",
      description: "Groupe de cases : nommé par l'étiquette, décrit par l'aide",
      props: {},
      demo: Columns,
    },
    {
      name: "invalid",
      description: "Groupe obligatoire sans choix : l'erreur dite et liée",
      props: {},
      demo: () => h(Columns, { initial: [] }),
    },
    {
      name: "settings",
      description: "Groupe de bascules (réglages pris aussitôt)",
      props: {},
      demo: Settings,
    },
  ],
} satisfies CatalogueEntry;
