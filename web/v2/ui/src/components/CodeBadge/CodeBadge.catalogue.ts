// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import DemoColumn from "../../demo/DemoColumn.vue";
import { ids, problems } from "../../fixtures/index.ts";
import Cluster from "../Cluster/Cluster.vue";
import CodeBadge from "./CodeBadge.vue";

/** A long code in a narrow column: cut, whole in its bubble. */
const Cut = () =>
  h(DemoColumn, { size: "sm" }, () => h(CodeBadge, { code: problems.precondition }));

/** The codes of a refusal, for the keyboard script (e2e/components/code-badge.spec.ts). */
const Keyboard = () =>
  h(Cluster, { gap: "2" }, () => [
    h(CodeBadge, { code: problems.precondition }),
    h(CodeBadge, { code: ids.request, label: "Request-Id" }),
  ]);

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
      description: "Copié : dit par la région, coché (la bulle le dit tant que le pointeur reste)",
      props: { code: ids.request, label: "Request-Id" },
      interaction: "press",
    },
    {
      name: "cut",
      description: "Coupé par la largeur : entier dans la bulle, au survol et au focus",
      props: {},
      demo: Cut,
      interaction: "focus",
    },
    { name: "keyboard", description: "Deux codes, copiés au clavier", props: {}, demo: Keyboard },
  ],
} satisfies CatalogueEntry;
