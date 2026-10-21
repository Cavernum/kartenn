import { readFileSync } from "node:fs";
import { request, type APIRequestContext, type APIResponse } from "@playwright/test";
import { BASE_URL, ISSUER, STATE } from "./fixtures/env.ts";
import { bulkAddresses, DATA, GROUPS } from "./fixtures/data.ts";
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
    for (const [group, role] of Object.entries(GROUPS)) {
      await expectOk(api, "POST", "/v1/bindings", {
        motive: "end-to-end journeys",
        subject: { kind: "group", issuer: ISSUER, group },
        role,
        scope: { kind: "global" },
      });
    }
    const entity = await created(api, "/v1/entities", { slug: DATA.entity });
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
  } finally {
    await api.dispose();
  }
}

async function expectOk(
  api: APIRequestContext,
  method: "POST",
  path: string,
  body: object,
  ...statuses: number[]
): Promise<APIResponse> {
  const response = await api.fetch(path, { method, data: body });
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
