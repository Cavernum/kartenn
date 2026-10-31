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
        problem: { type: problems.unavailable, requestId: ids.request },
        retryLabel: "Réessayer",
      },
      wide: true,
    },
    {
      name: "precondition",
      description: "Erreur 412 : message, code et identifiant de requête copiables (REQ-020-22)",
      props: {
        title: "Ce préfixe a changé depuis votre lecture : relisez-le avant de le modifier.",
        problem: { type: problems.precondition, requestId: ids.request },
        retryLabel: "Relire",
      },
      wide: true,
    },
    {
      name: "violations",
      description: "Violations : chaque gravité, message et code (un avertissement le dit)",
      props: {
        title: "Le changeset est refusé.",
        problem: {
          type: problems.refused,
          requestId: ids.request,
          violations: [
            {
              rule: problems.violation,
              detail: "Le préfixe chevauche un voisin.",
              severity: "error",
            },
            {
              rule: "core.owner.unknown",
              detail: "Le responsable est inconnu.",
              severity: "error",
            },
            {
              rule: "prop.dns.ttl-missing",
              detail: "Le TTL DNS est attendu en production.",
              severity: "warning",
            },
          ],
        },
      },
      wide: true,
    },
    {
      name: "forbidden",
      description: "Sans nouvelle tentative possible",
      props: {
        title: "Vous n'avez pas le droit de lire ce domaine.",
        problem: { type: problems.forbidden, requestId: ids.request },
      },
      wide: true,
    },
    {
      name: "network",
      description: "Sans réponse de l'API : la phrase et Réessayer, aucun code inventé",
      props: { title: "Le serveur est injoignable.", retryLabel: "Réessayer" },
      wide: true,
    },
  ],
} satisfies CatalogueEntry;
