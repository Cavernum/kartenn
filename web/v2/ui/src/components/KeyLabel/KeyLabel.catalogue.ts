// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { domains } from "../../fixtures/index.ts";
import KeyLabel from "./KeyLabel.vue";

export default {
  component: KeyLabel,
  pattern: "— (libellé, à défaut la clé)",
  states: [
    {
      name: "labelled",
      description: "Libellé dans la langue",
      props: { value: domains[0].key, label: domains[0].label },
    },
    {
      name: "with-key",
      description: "Libellé et clé côte à côte",
      props: { value: domains[1].key, label: domains[1].label, showKey: true },
    },
    {
      name: "key-only",
      description: "Sans libellé : la clé en chasse fixe",
      props: { value: domains[2].key, label: domains[2].label },
    },
  ],
} satisfies CatalogueEntry;
