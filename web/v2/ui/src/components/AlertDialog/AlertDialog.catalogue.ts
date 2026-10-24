// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import AlertDialog from "./AlertDialog.vue";

export default {
  component: AlertDialog,
  pattern: "Alert Dialog (focus sur Annuler)",
  states: [
    {
      name: "destructive",
      description: "Confirmation destructive",
      props: {
        title: "Abandonner le brouillon ?",
        description: "Les 12 opérations en brouillon seront perdues.",
        confirm: "Abandonner",
        danger: true,
        open: true,
      },
      isolated: true,
    },
    {
      name: "confirm",
      description: "Confirmation simple",
      props: {
        title: "Appliquer le changeset ?",
        description: "Les modifications seront visibles de tous.",
        confirm: "Appliquer",
        open: true,
      },
      isolated: true,
    },
  ],
} satisfies CatalogueEntry;
