// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// The types of object and their collections (design « Couche de données »): the keys of
// the cache name an object by its qualified type (`core.prefix`, `vlan.vlan`), as the
// event stream does (022), and a list by the path of its collection. The core and the
// first-party modules are known here; a third-party plugin registers its own types.

/** The collections of the core and of the first-party modules, by qualified type. */
const KNOWN: Record<string, string> = {
  "core.entity": "/v1/entities",
  "core.site": "/v1/sites",
  "core.domain": "/v1/domains",
  "core.prefix": "/v1/prefixes",
  "core.range": "/v1/ranges",
  "core.address": "/v1/addresses",
  "vlan.group": "/v1/vlan-groups",
  "vlan.vlan": "/v1/vlans",
  "machines.cluster": "/v1/clusters",
  "machines.machine": "/v1/machines",
  "machines.interface": "/v1/interfaces",
  "policy.axis": "/v1/policy-axes",
  "policy.value": "/v1/policy-values",
  "policy.flow_type": "/v1/flow-types",
  "policy.service": "/v1/services",
  "policy.pattern": "/v1/policy-patterns",
  "policy.rule": "/v1/policy-rules",
  "policy.condition": "/v1/admissibility-conditions",
  "policy.signal": "/v1/risk-signals",
  "policy.exempt_role": "/v1/policy-exempt-roles",
  "policy.inference_rule": "/v1/inference-rules",
  "flow.flow": "/v1/flows",
  // Not objects of the referential, but read and invalidated the same way.
  changeset: "/v1/changesets",
};

const collections = new Map<string, string>(Object.entries(KNOWN));

/**
 * The qualified type of a kind: the kinds of the core are bare in the operations and the
 * cascades (`prefix`), qualified in the events (`core.prefix`).
 */
export function qualify(kind: string): string {
  return kind.includes(".") || kind === "changeset" ? kind : `core.${kind}`;
}

/** The collection of a type (`/v1/prefixes`), `null` if unknown. */
export function collectionOf(type: string): string | null {
  return collections.get(qualify(type)) ?? null;
}

/** The type whose collection is `path`, `null` if none. */
export function typeOfCollection(path: string): string | null {
  for (const [type, collection] of collections) if (collection === path) return type;
  return null;
}

/** A plugin's type of object and its collection (`/v1/…`); refused if taken by another. */
export function registerType(type: string, collection: string): void {
  const known = collections.get(type);
  if (known !== undefined && known !== collection) {
    throw new Error(`type ${type} is already served by ${known}`);
  }
  if (!/^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$/.test(type)) {
    throw new TypeError(`${type}: a type is qualified, <module>.<name>`);
  }
  if (!/^\/v1\/[a-z0-9-]+$/.test(collection)) {
    throw new TypeError(`${collection}: a collection is /v1/<name>`);
  }
  collections.set(type, collection);
}
