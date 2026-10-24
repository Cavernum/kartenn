// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import type { CatalogueEntry } from "../../catalogue.ts";
import { ids, problems } from "../../fixtures/index.ts";
import CodeBadge from "./CodeBadge.vue";

export default {
  component: CodeBadge,
  pattern: "Button (copier), annonce role=status",
  states: [
    { name: "code", description: "Code de problème", props: { code: problems.conflict } },
    {
      name: "labelled",
      description: "Identifiant de requête, nommé",
      props: { code: ids.request, label: "Request-Id" },
    },
    {
      name: "hover",
      description: "Survol",
      props: { code: problems.violation },
      interaction: "hover",
    },
    {
      name: "focus",
      description: "Focus visible",
      props: { code: problems.violation },
      interaction: "focus",
    },
    {
      name: "copied",
      description: "Copié (annoncé)",
      props: { code: ids.request, label: "Request-Id" },
      interaction: "press",
    },
  ],
} satisfies CatalogueEntry;
