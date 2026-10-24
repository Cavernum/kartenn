// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { ids, problems } from "../../fixtures/index.ts";
import ErrorState from "./ErrorState.vue";

export default {
  component: ErrorState,
  pattern: "role=alert, codes copiables",
  states: [
    {
      name: "retry",
      description: "Phrase, code, identifiant de requête, Réessayer",
      props: {
        title: "Le service ne répond pas.",
        code: problems.unavailable,
        requestId: ids.request,
        retryLabel: "Réessayer",
      },
      wide: true,
    },
    {
      name: "forbidden",
      description: "Sans nouvelle tentative possible",
      props: {
        title: "Vous n'avez pas le droit de lire ce domaine.",
        code: problems.forbidden,
        requestId: ids.request,
      },
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
