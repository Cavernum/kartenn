import { readFileSync } from "node:fs";
import { request } from "@playwright/test";
import { BASE_URL, ISSUER, STATE } from "../fixtures/env.ts";
import { totp } from "../fixtures/totp.ts";

/**
 * The bindings of the browser bench of the new shell (spec 021, REQ-021-30), through the
 * API under the break-glass session of `serve.sh budgets`: the issuer attached to the whole
 * organisation, the group of `alice` administrator everywhere — the bench edits a cell, into
 * the draft only. The data set itself is loaded by `kartenn-bench seed`.
 */
export default async function globalSetup(): Promise<void> {
  const proofs = readFileSync(`${STATE}breakglass-v2.txt`, "utf8");
  const passphrase = /^passphrase: (.+)$/m.exec(proofs)?.[1];
  const secret = /secret=([A-Z2-7]+)/.exec(proofs)?.[1];
  if (!passphrase || !secret) throw new Error(`no break-glass proofs in ${STATE}`);
  const api = await request.newContext({
    baseURL: BASE_URL,
    ignoreHTTPSErrors: true,
    extraHTTPHeaders: { Origin: BASE_URL },
  });
  try {
    const writes: [string, object][] = [
      ["/v1/auth/breakglass", { name: "e2e", passphrase, code: totp(secret) }],
      ["/v1/issuer-attachments?approval=self", { motive: "browser bench", issuer: ISSUER }],
      [
        "/v1/changesets?approval=self",
        {
          motive: "browser bench",
          operations: [
            { op: "set_group", key: "kartenn-admins", labels: { fr: "kartenn-admins" } },
            {
              op: "add_member",
              group: "kartenn-admins",
              member: { kind: "idp_group", issuer: ISSUER, group: "kartenn-admins" },
            },
            {
              op: "bind",
              group: "kartenn-admins",
              role: "administrator",
              scope: { kind: "global" },
            },
          ],
        },
      ],
    ];
    for (const [path, body] of writes) {
      const response = await api.post(path, { data: body });
      if (!response.ok()) {
        throw new Error(`POST ${path}: ${response.status()} ${await response.text()}`);
      }
    }
  } finally {
    await api.dispose();
  }
}
