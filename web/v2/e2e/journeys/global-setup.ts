import { readFileSync } from "node:fs";
import { request, type APIRequestContext, type APIResponse } from "@playwright/test";
import { BASE_URL, ISSUER, STATE } from "../fixtures/env.ts";
import {
  bulkAddresses,
  DATA,
  EDITOR_WITHOUT_APPLY,
  GROUPS,
  widePrefixes,
} from "../fixtures/journeys.ts";
import { totp } from "../fixtures/totp.ts";

/**
 * What the journeys of the new shell read, loaded through the API under the break-glass
 * session of serve.sh (as web/e2e/global-setup.ts, whose data set the ported journeys
 * take over task by task): the issuer attached to the whole organisation and the bindings
 * of the groups of users.toml.
 */
export default async function globalSetup(): Promise<void> {
  const proofs = readFileSync(`${STATE}breakglass-v2.txt`, "utf8");
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
      "/v1/auth/breakglass",
      { name: "e2e", passphrase, code: totp(secret) },
      204,
    );
    await expectOk(api, "/v1/issuer-attachments?approval=self", {
      motive: "end-to-end journeys",
      issuer: ISSUER,
    });
    // Each group of users.toml is a member of the group of kartenn of the same key, which
    // holds the roles (spec 025: a binding serves a group of kartenn).
    await expectOk(api, "/v1/changesets?approval=self", {
      motive: "end-to-end journeys",
      operations: [
        // The role of the editor who may not apply (journey `edit`, REQ-021-37).
        { op: "set_role", ...EDITOR_WITHOUT_APPLY },
        ...Object.entries(GROUPS).flatMap(([key, roles]) => [
          { op: "set_group", key, labels: { fr: key } },
          {
            op: "add_member",
            group: key,
            member: { kind: "idp_group", issuer: ISSUER, group: key },
          },
          ...roles.map((role) => ({ op: "bind", group: key, role, scope: { kind: "global" } })),
        ]),
      ],
    });
    // The referential the palette searches (REQ-021-3): an entity, a domain that it owns
    // (`core.owner` from its creation, REQ-002-11), a prefix and an address in it.
    const entity = await created(api, "/v1/entities?approval=self", {
      motive: "end-to-end data set",
      slug: DATA.entity,
    });
    const declarations = (await (await api.get("/v1/property-declarations")).json()) as {
      items: { id: string; key: string }[];
    };
    const owner = declarations.items.find((d) => d.key === "core.owner")?.id;
    if (!owner) throw new Error("no declaration of core.owner");
    const domain = { local: 0 };
    await expectOk(api, "/v1/changesets?approval=self", {
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
        { op: "create_prefix", domain, prefix: DATA.prefix, nature: "network" },
        { op: "create_address", domain, address: DATA.address, usage: "host" },
      ],
    });
    // The referential of the lists and the tree (task 11): a network of 1 000 addresses, a
    // container of 1 000 networks, in documentation space (RFC 3849).
    const domains = (await (await api.get(`/v1/domains?slug=${DATA.domain}`)).json()) as {
      items: { id: string }[];
    };
    const production = { id: domains.items[0]!.id };
    // The VLAN of the data set (026): a group anchored on the domain, one VLAN in it.
    await expectOk(api, "/v1/changesets?approval=self", {
      motive: "end-to-end VLAN",
      operations: [
        {
          op: "create_vlan_group",
          local: 0,
          slug: DATA.vlan.group,
          anchor: { domain: production },
          vid_min: 1,
          vid_max: 99,
        },
        { op: "create_vlan", group: { local: 0 }, vid: DATA.vlan.vid, name: DATA.vlan.name },
      ],
    });
    await expectOk(api, "/v1/changesets?approval=self", {
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
    await expectOk(api, "/v1/changesets?approval=self", {
      motive: "end-to-end wide container",
      operations: [
        { op: "create_prefix", domain: production, prefix: DATA.wide.prefix, nature: "container" },
        ...widePrefixes().map((prefix) => ({
          op: "create_prefix",
          domain: production,
          prefix,
          nature: "network",
        })),
      ],
    });
  } finally {
    await api.dispose();
  }
}

async function expectOk(
  api: APIRequestContext,
  path: string,
  body: object,
  ...statuses: number[]
): Promise<APIResponse> {
  const response = await api.post(path, { data: body });
  const expected = statuses.length > 0 ? statuses : [200, 201];
  if (!expected.includes(response.status())) {
    throw new Error(`POST ${path}: ${response.status()} ${await response.text()}`);
  }
  return response;
}

/** Creates an object by its collection, and returns its identifier (from `Location`). */
async function created(api: APIRequestContext, path: string, body: object): Promise<string> {
  const response = await expectOk(api, path, body);
  const id = response.headers()["location"]?.split("/").pop();
  if (!id) throw new Error(`POST ${path}: no Location`);
  return id;
}
