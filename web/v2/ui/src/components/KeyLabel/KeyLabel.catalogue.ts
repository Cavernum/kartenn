// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { h } from "vue";
import type { CatalogueEntry } from "../../catalogue.ts";
import { domains, environments } from "../../fixtures/index.ts";
import { currentLocale, type Locale } from "../../i18n.ts";
import Cluster from "../Cluster/Cluster.vue";
import KeyLabel from "./KeyLabel.vue";

/**
 * `prod` with its labels by language (REQ-020-21): « Production » in French, its key in
 * English (no English label); the key copyable in both (e2e/components/key-label.spec.ts).
 */
const LABELS: Partial<Record<Locale, string>> = { fr: environments[0].label };

const Fallback = () =>
  h(Cluster, { gap: "4" }, () => [
    h(KeyLabel, { value: environments[0].key, label: LABELS[currentLocale()] ?? null }),
    h(KeyLabel, { value: domains[2].key, label: domains[2].label }),
  ]);

export default {
  component: KeyLabel,
  pattern: "Button (copier la clé) ; libellé, à défaut la clé",
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
    {
      name: "hover",
      description: "Survol : la clé dans la bulle, et comment la copier",
      props: { value: domains[0].key, label: domains[0].label },
      interaction: "hover",
    },
    {
      name: "focus",
      description: "Focus visible : la bulle aussitôt",
      props: { value: domains[0].key, label: domains[0].label },
      interaction: "focus",
    },
    {
      name: "plain",
      description: "Sans bouton (dans un contrôle) : la clé toujours à côté",
      props: { value: domains[0].key, label: domains[0].label, plain: true },
    },
    {
      name: "fallback",
      description: "prod : « Production » en français, sa clé dans une langue sans libellé",
      props: {},
      demo: Fallback,
    },
  ],
} satisfies CatalogueEntry;
