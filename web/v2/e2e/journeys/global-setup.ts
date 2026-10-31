import { readFileSync } from "node:fs";
import { request, type APIRequestContext } from "@playwright/test";
import { BASE_URL, ISSUER, STATE } from "../fixtures/env.ts";
import { GROUPS } from "../fixtures/journeys.ts";
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
      operations: Object.entries(GROUPS).flatMap(([key, roles]) => [
        { op: "set_group", key, labels: { fr: key } },
        { op: "add_member", group: key, member: { kind: "idp_group", issuer: ISSUER, group: key } },
        ...roles.map((role) => ({ op: "bind", group: key, role, scope: { kind: "global" } })),
      ]),
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
): Promise<void> {
  const response = await api.post(path, { data: body });
  const expected = statuses.length > 0 ? statuses : [200, 201];
  if (!expected.includes(response.status())) {
    throw new Error(`POST ${path}: ${response.status()} ${await response.text()}`);
  }
}
