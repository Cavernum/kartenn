// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, ref } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoColumn from "../../demo/DemoColumn.vue";
import CommitForm from "./CommitForm.vue";

/** The form at the width of the panel; a submission is said under it. */
const demo = defineComponent({
  props: {
    summary: { type: String, default: "" },
    description: { type: String, default: "" },
    required: Boolean,
    error: { type: String, default: undefined },
  },
  setup(props) {
    const summary = ref(props.summary);
    const description = ref(props.description);
    const sent = ref("");
    return () =>
      h(DemoColumn, () => [
        h(CommitForm, {
          summary: summary.value,
          "onUpdate:summary": (v: string) => (summary.value = v),
          description: description.value,
          "onUpdate:description": (v: string) => (description.value = v),
          required: props.required,
          error: props.error,
          onSubmit: () => (sent.value = `Soumis : ${summary.value}`),
        }),
        h("p", { role: "status", class: "k-cat-note" }, sent.value),
      ]);
  },
});

export default {
  component: CommitForm,
  pattern: "champs étiquetés ; Ctrl+Entrée émet submit",
  states: [
    { name: "empty", description: "Vide, résumé obligatoire", props: { required: true }, demo },
    {
      name: "prefilled",
      description: "Motif prérempli (« Appliquer »), modifiable",
      props: { required: true, summary: "Maintenance du référentiel" },
      demo,
    },
    {
      name: "described",
      description: "Résumé et description",
      props: {
        required: true,
        summary: "Bascule des adresses de la salle B",
        description: "Les passerelles changent de plage ; les VIP suivent.",
      },
      demo,
    },
    {
      name: "missing",
      description: "Motif requis manquant (refusé à la soumission)",
      props: { required: true, error: "Le résumé est obligatoire." },
      demo,
    },
  ],
} satisfies CatalogueEntry;
