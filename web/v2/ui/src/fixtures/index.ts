// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

/**
 * Fictitious data of the catalogue and the mock-ups (design « Catalogue »): documentation
 * address blocks (RFC 5737, RFC 3849), reserved names (RFC 2606), invented environments,
 * identifiers and problem codes of kartenn. No link with any real information system.
 */

/** Environments: key, label, hue of the closed palette (REQ-020-8). */
export const environments = [
  { key: "prod", label: "Production", hue: 1 },
  { key: "rece", label: "Recette", hue: 2 },
  { key: "deve", label: "Développement", hue: 3 },
  { key: "form", label: null, hue: 4 },
] as const;

/** Addressing domains. */
export const domains = [
  { key: "dc-nord", label: "Centre de données nord" },
  { key: "dc-sud", label: "Centre de données sud" },
  { key: "bureaux", label: null },
] as const;

/** Prefixes of documentation blocks. */
export const prefixes = [
  "192.0.2.0/24",
  "192.0.2.0/26",
  "192.0.2.64/26",
  "198.51.100.0/24",
  "203.0.113.0/24",
  "2001:db8::/32",
  "2001:db8:10::/48",
] as const;

/** Addresses of documentation blocks. */
export const addresses = [
  "192.0.2.10",
  "192.0.2.11",
  "198.51.100.7",
  "203.0.113.42",
  "2001:db8:10::1",
] as const;

/** Identifiers of objects and requests. */
export const ids = {
  object: "01JD4K7Q9V2M8X3N5P6R7S8T9W",
  changeset: "01JD4K8B2C3D4E5F6G7H8J9K0M",
  request: "req_7f3a9c2e5b1d4068",
} as const;

/** Problem types of kartenn (`urn:kartenn:problem:<slug>`) and codes of violations. */
export const problems = {
  conflict: "urn:kartenn:problem:conflict",
  forbidden: "urn:kartenn:problem:forbidden",
  unavailable: "urn:kartenn:problem:unavailable",
  violation: "prefix.overlaps-sibling",
} as const;
