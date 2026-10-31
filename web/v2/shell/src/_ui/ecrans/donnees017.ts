/**
 * Fictitious data of the mock-ups of the plugins `policy` and `flow` (spec 017, task 1;
 * E14 to E27), beside the shared ones of `donnees.ts` (flow types, services, rules,
 * machines, register): the sidebar with the views of both manifests, the people, the
 * patterns, the review queue, the deadlines, the nonconformities, the inference rules and
 * the audit reports. Documentation blocks only (RFC 5737, RFC 3849); invented teams.
 * The clock of the captures is frozen on 2026-01-15, 10:00 (Paris).
 */

import type { SidebarGroup, State } from "@kartenn/ui";
import type { LocationQuery, RouteLocationRaw } from "vue-router";
import { SPACES } from "./sidebar.ts";

/** The day of the frozen clock, and the revision of the policy read at it. */
export const TODAY = "2026-01-15";
export const REVISION = "r42";

/**
 * The sidebar of the mock-ups of the 017: the spaces of the frame, with every view of the
 * two manifests that is an entry of its space (`/x/policy/<vue>`, `/x/flow/<vue>`); the
 * view `changes` is reached from a changeset (action « Effet sur la politique »), the view
 * `request` from the register (« Proposer des flux ») and from the cards.
 */
export const SPACES_017: SidebarGroup[] = SPACES.map((space) =>
  space.id === "policy"
    ? {
        ...space,
        entries: [
          { id: "matrix", label: "Matrice", icon: "matrix", to: "/x/policy/matrix" },
          { id: "rules", label: "Règles et motifs", icon: "shield", to: "/x/policy/rules" },
          { id: "catalogue", label: "Catalogue", icon: "tag", to: "/x/policy/catalogue" },
          { id: "inference", label: "Inférence", icon: "git-compare", to: "/x/policy/inference" },
          {
            id: "compliance",
            label: "Conformité",
            icon: "clipboard-check",
            to: "/x/policy/compliance",
          },
          { id: "reports", label: "Rapports d'audit", icon: "file-diff", to: "/x/policy/reports" },
        ],
      }
    : space.id === "flows"
      ? {
          ...space,
          entries: [
            { id: "flows", label: "Flux", icon: "flow", to: "/x/flow/flows" },
            {
              id: "flow-review",
              label: "Instruction",
              icon: "send",
              to: "/x/flow/review",
              badge: 3,
            },
            {
              id: "expiring",
              label: "Échéances",
              icon: "clock",
              to: "/x/flow/expiring",
              badge: 2,
            },
            {
              id: "nonconformities",
              label: "Non-conformités",
              icon: "triangle-alert",
              to: "/x/flow/nonconformities",
              badge: 4,
            },
          ],
        }
      : space,
);

/** A link to a view of a mock-up, keeping the switches of the catalogue. */
export function lien(ecran: string, vue: string, query: LocationQuery): RouteLocationRaw {
  const kept: Record<string, string> = { vue };
  for (const key of ["mode", "densite", "langue"]) {
    const value = query[key];
    if (typeof value === "string") kept[key] = value;
  }
  return { path: `/_ui/ecrans/${ecran}`, query: kept };
}

/** The people of the mock-ups: the user, and colleagues of other teams. */
export const ME = "Camille Martin";
export const PEOPLE = {
  lou: "Lou Bernard",
  sacha: "Sacha Morel",
  alex: "Alex Durand",
  noa: "Noa Petit",
} as const;

/** The entities a flow is requested for (core.owner). */
export const ENTITIES = {
  nord: "Atelier Nord",
  sud: "Atelier Sud",
  pilote: "Service Pilote",
} as const;

/** The effect of a rule or the state of a cell of the matrix, said in words (U12). */
export type Effect = "allow" | "deny" | "conditional" | "default";
export const EFFECT: Record<Effect, { state: State; label: string }> = {
  allow: { state: "ok", label: "Autoriser" },
  deny: { state: "ko", label: "Interdire" },
  conditional: { state: "warn", label: "Sous motif" },
  default: { state: "neutral", label: "Refus par défaut" },
};

/** The verdict of an evaluation (REQ-010-11) and its outcome (REQ-011-21). */
export type Verdict = "allowed" | "allowed_pending" | "denied";
export const VERDICT: Record<Verdict, { state: State; label: string }> = {
  allowed: { state: "ok", label: "Conforme" },
  allowed_pending: { state: "warn", label: "Sous réserve" },
  denied: { state: "ko", label: "Non conforme" },
};
export type Synthesis = "allowed" | "partial" | "denied";
export const SYNTHESIS: Record<Synthesis, { state: State; label: string }> = {
  allowed: { state: "ok", label: "Conforme" },
  partial: { state: "warn", label: "En partie conforme" },
  denied: { state: "ko", label: "Non conforme" },
};
export type Outcome = "automatic" | "review" | "noncompliant";
export const OUTCOME: Record<Outcome, { state: State; label: string }> = {
  automatic: { state: "ok", label: "Enregistrement automatique" },
  review: { state: "review", label: "À instruire" },
  noncompliant: { state: "ko", label: "Non conforme" },
};

/** The labels of the rules of `donnees.ts` and of the 017 (key and label, REQ-020-21). */
export const RULE_LABEL: Record<string, string> = {
  "app-meme-env": "Applicatif dans un environnement",
  "rece-vers-prod": "Recette vers production",
  "deny-deve-prod": "Développement vers production",
  "deny-prod-hors-prod": "Production vers hors production",
  "admin-par-bastion": "Administration par le bastion",
  "deny-admin-hors-bastion": "Administration hors bastion",
  "bdd-meme-env": "Bases dans leur environnement",
  "deny-rece-vers-prod-bdd": "Données de production en recette",
  "fichier-prod": "Fichiers de production",
};

/** The patterns of the policy (REQ-010-8): key, label, pedagogical text. */
export interface Pattern extends Record<string, unknown> {
  key: string;
  label: string;
  text: string;
  rules: number;
  from: string;
}
export const PATTERNS: Pattern[] = [
  {
    key: "api-lecture",
    label: "Lecture d'API",
    text: "Le flux ne lit que des API exposées en lecture, sans écriture ni export de masse.",
    rules: 2,
    from: "2025-04-02",
  },
  {
    key: "bastion-admin",
    label: "Administration par le bastion",
    text: "La session part du bastion, nominative et enregistrée.",
    rules: 1,
    from: "2025-02-11",
  },
  {
    key: "object-storage-relay",
    label: "Relais de stockage objet",
    text: "Les fichiers passent par le relais de stockage objet, chiffrés et à durée de rétention fixée.",
    rules: 1,
    from: "2025-09-30",
  },
  {
    key: "sauvegarde-chiffree",
    label: "Sauvegarde chiffrée",
    text: "La sauvegarde est chiffrée de bout en bout, la clé reste chez le propriétaire des données.",
    rules: 0,
    from: "2025-11-20",
  },
];

/** A changeset of the mock-ups of the 017 (invented ULID). */
export const CHANGESET = {
  policy: "01JF3B6D8F0H2K4M6P8R0T2V4X",
  request: "01JF3C1E3G5J7L9N1Q3S5U7W9Y",
  extension: "01JF3C4H6K8M0P2R4T6V8X0Z2B",
} as const;
