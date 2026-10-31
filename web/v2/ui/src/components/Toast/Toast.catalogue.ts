// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { defineComponent, h, type PropType } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { ids, prefixes, problems } from "../../fixtures/index.ts";
import Button from "../Button/Button.vue";
import Cluster from "../Cluster/Cluster.vue";
import Stack from "../Stack/Stack.vue";
import TextField from "../TextField/TextField.vue";
import Toast from "./Toast.vue";
import ToastRegion from "./ToastRegion.vue";
import { createToasts, type ToastInput } from "./toasts.ts";

/** A success, as an edit shows it. */
const created: ToastInput = {
  tone: "ok",
  title: `Préfixe ${prefixes[1]} ajouté au brouillon.`,
};

/** A failed write: its problem type and its Request-Id, kept until dismissed. */
const failed: ToastInput = {
  tone: "ko",
  title: "L'enregistrement a échoué : le service est momentanément indisponible.",
  description:
    "Rien n'a été écrit ; votre saisie est gardée. Si l'erreur persiste, transmettez ces deux codes.",
  code: problems.unavailable,
  requestId: ids.request,
};

/** The region, the toasts given shown at once; held for the capture (no duration). */
const Shown = defineComponent({
  props: { toasts: { type: Array as PropType<ToastInput[]>, required: true } },
  setup(props) {
    const store = createToasts();
    for (const toast of props.toasts) store.show({ duration: Infinity, ...toast });
    return () => [
      h("p", null, "Les toasts paraissent en bas de la fenêtre, au milieu, par-dessus la page."),
      h(ToastRegion, { toasts: store }),
    ];
  },
});

/**
 * The toasts of the keyboard script (e2e/components/toast.spec.ts): a success that goes
 * after 4 s, an error that stays; a field before them to type in while they come.
 */
const Keyboard = defineComponent({
  setup() {
    const store = createToasts();
    let count = 0;
    return () =>
      h(Stack, { gap: "3" }, () => [
        h(TextField, { label: "Note" }),
        h(Cluster, { gap: "2" }, () => [
          h(
            Button,
            {
              onClick: () =>
                store.show({
                  ...created,
                  title: `Préfixe ${prefixes[count++ % prefixes.length]} ajouté au brouillon.`,
                  duration: 4000,
                }),
            },
            () => "Ajouter",
          ),
          h(Button, { onClick: () => store.show(failed) }, () => "Enregistrer"),
        ]),
        h(ToastRegion, { toasts: store }),
      ]);
  },
});

export default {
  component: Toast,
  pattern: "status (polite) ; alert (assertive), pause au survol et au focus",
  states: [
    {
      name: "ok",
      description: "Succès, dit poliment (status) ; il part au bout de 8 s, tenu ici",
      props: { toasts: [created] },
      demo: Shown,
      isolated: true,
    },
    {
      name: "error",
      description:
        "Erreur, dite aussitôt (alert), avec son code et son Request-Id ; elle reste jusqu'à ce qu'on la ferme",
      props: { toasts: [failed] },
      demo: Shown,
      isolated: true,
    },
    {
      name: "stack",
      description:
        "Plusieurs : les erreurs au-dessus, les autres dans l'ordre d'arrivée ; une information avec son action",
      props: {
        toasts: [
          failed,
          {
            tone: "info",
            title: "Proposé, en revue : votre modification attend une validation.",
            description: "Vous n'avez pas le droit d'appliquer vous-même les adresses.",
            code: "changeset.approval.self.missing",
            action: { label: "Voir le changeset", run: () => undefined },
          },
          created,
        ],
      },
      demo: Shown,
      isolated: true,
    },
    {
      name: "keyboard",
      description:
        "Au clavier : aucun toast ne prend le focus ; Tab les atteint, Échap ou la croix les ferment ; survol et focus les retiennent",
      props: {},
      demo: Keyboard,
    },
  ],
} satisfies CatalogueEntry;
