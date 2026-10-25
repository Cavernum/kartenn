import { readFileSync } from "node:fs";
import { request, type APIRequestContext, type APIResponse } from "@playwright/test";
import { BASE_URL, ISSUER, STATE } from "./fixtures/env.ts";
import {
  appliedByAuthor,
  bulkAddresses,
  CONTRIBUTOR,
  DATA,
  GROUPS,
  widePrefixes,
} from "./fixtures/data.ts";
import { totp } from "./fixtures/totp.ts";

/**
 * Loads, through the API and under the break-glass session of serve.sh, what the journeys
 * read (spec 007, design « Bout en bout et développement »): the issuer attached to the
 * whole organisation, the bindings of the groups of users.toml, then the data set by
 * changesets.
 */
export default async function globalSetup(): Promise<void> {
  const proofs = readFileSync(`${STATE}breakglass.txt`, "utf8");
  const passphrase = /^passphrase: (.+)$/m.exec(proofs)?.[1];
  const secret = /secret=([A-Z2-7]+)/.exec(proofs)?.[1];
  if (!passphrase || !secret) throw new Error(`no break-glass proofs in ${STATE}`);

  const api = await request.newContext({
    baseURL: BASE_URL,
    ignoreHTTPSErrors: true,
    // CSRF (REQ-006-28): a cookie session writes with the origin of the instance.
    extraHTTPHeaders: { Origin: BASE_URL },
  });
  try {
    await expectOk(
      api,
      "POST",
      "/v1/auth/breakglass",
      { name: "e2e", passphrase, code: totp(secret) },
      204,
    );
    await expectOk(api, "POST", "/v1/issuer-attachments", {
      motive: "end-to-end journeys",
      issuer: ISSUER,
    });
    await expectOk(api, "PUT", `/v1/roles/${CONTRIBUTOR.key}`, {
      motive: "end-to-end journeys",
      labels: { fr: CONTRIBUTOR.label },
      entries: CONTRIBUTOR.entries,
    });
    // Each group of users.toml is a member of the group of kartenn of the same key, which
    // holds the roles (spec 025: a binding serves a group of kartenn).
    const group = (key: string) => [
      { op: "set_group", key, labels: { fr: key } },
      { op: "add_member", group: key, member: { kind: "idp_group", issuer: ISSUER, group: key } },
    ];
    await expectOk(api, "POST", "/v1/changesets", {
      motive: "end-to-end journeys",
      operations: Object.entries(GROUPS).flatMap(([key, roles]) => [
        ...group(key),
        ...roles.map((role) => ({ op: "bind", group: key, role, scope: { kind: "global" } })),
      ]),
    });
    const entity = await created(api, "/v1/entities", { slug: DATA.entity });
    // The group `redacteurs` (task 19): readers everywhere, able to read the roles (so that
    // the interface knows its scoped role), editors of one entity only.
    const confined = await created(api, "/v1/entities", { slug: DATA.confined });
    await expectOk(api, "POST", "/v1/changesets", {
      motive: "end-to-end journeys",
      operations: [
        {
          op: "set_role",
          key: "e2e_rbac_reader",
          labels: { fr: "Lecture des droits" },
          entries: ["core.rbac.read"],
        },
        ...group("redacteurs"),
        { op: "bind", group: "redacteurs", role: "reader", scope: { kind: "global" } },
        { op: "bind", group: "redacteurs", role: "e2e_rbac_reader", scope: { kind: "global" } },
        {
          op: "bind",
          group: "redacteurs",
          role: "editor",
          scope: { kind: "entity", id: { id: confined } },
        },
      ],
    });
    // A domain carries its owner from its creation (`core.owner`, REQ-002-11): one changeset.
    const declarations = (await (await api.get("/v1/property-declarations")).json()) as {
      items: { id: string; key: string }[];
    };
    const owner = declarations.items.find((d) => d.key === "core.owner")?.id;
    if (!owner) throw new Error("no declaration of core.owner");
    const domain = { local: 0 };
    await expectOk(api, "POST", "/v1/changesets", {
      motive: "end-to-end data set",
      operations: [
        { op: "create_domain", local: 0, slug: DATA.domain },
        {
          op: "assign_property",
          object: domain,
          kind: "domain",
          property: { id: owner },
          values: [entity],
        },
        ...DATA.prefixes.map(([prefix, nature]) => ({
          op: "create_prefix",
          domain,
          prefix,
          nature,
        })),
        ...DATA.addresses.map((address) => ({
          op: "create_address",
          domain,
          address,
          usage: "host",
        })),
      ],
    });
    // The long list (REQ-007-31): one network and its 1 000 addresses, in a second changeset.
    const domains = (await (await api.get(`/v1/domains?slug=${DATA.domain}`)).json()) as {
      items: { id: string }[];
    };
    const production = { id: domains.items[0]!.id };
    await expectOk(api, "POST", "/v1/changesets", {
      motive: "end-to-end long list",
      operations: [
        { op: "create_prefix", domain: production, prefix: DATA.bulk.prefix, nature: "network" },
        ...bulkAddresses().map((address) => ({
          op: "create_address",
          domain: production,
          address,
          usage: "host",
        })),
      ],
    });
    // The wide container of the tree (REQ-007-31): 10⁴ direct children, by changesets of
    // 2 500 operations.
    await expectOk(api, "POST", "/v1/changesets", {
      motive: "end-to-end wide container",
      operations: [
        { op: "create_prefix", domain: production, prefix: DATA.wide.prefix, nature: "container" },
      ],
    });
    const wide = widePrefixes();
    for (let start = 0; start < wide.length; start += 2500) {
      await expectOk(api, "POST", "/v1/changesets", {
        motive: "end-to-end wide container",
        operations: wide.slice(start, start + 2500).map((prefix) => ({
          op: "create_prefix",
          domain: production,
          prefix,
          nature: "network",
        })),
      });
    }
  } finally {
    await api.dispose();
  }
}

async function expectOk(
  api: APIRequestContext,
  method: "POST" | "PUT",
  path: string,
  body: object,
  ...statuses: number[]
): Promise<APIResponse> {
  const response = await api.fetch(appliedByAuthor(method, path), { method, data: body });
  const expected = statuses.length > 0 ? statuses : [200, 201];
  if (!expected.includes(response.status())) {
    throw new Error(`${method} ${path}: ${response.status()} ${await response.text()}`);
  }
  return response;
}

/** Creates an object by its collection, and returns its identifier (from `Location`). */
async function created(api: APIRequestContext, path: string, body: object): Promise<string> {
  const response = await expectOk(api, "POST", path, { motive: "end-to-end data set", ...body });
  const id = response.headers()["location"]?.split("/").pop();
  if (!id) throw new Error(`POST ${path}: no Location`);
  return id;
}
