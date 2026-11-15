import { spawn } from "node:child_process";
import { closeSync, openSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { request, test as setup } from "@playwright/test";
import { ISSUER, QUOTA_URL, STATE } from "../fixtures/env.ts";
import { QUOTA_DATA, quotaAddresses } from "../fixtures/journeys.ts";
import { QUOTA_PID, stopQuotaInstance } from "../fixtures/quota.ts";
import { breakglass, created, expectOk } from "./global-setup.ts";

// The instance at the read quota by default (task 021-11p10-b2; web/v2/e2e/quota.toml), for
// the journey of the quota alone: the project `quota-instance` of journeys.config.ts, a
// dependency of `quota-chromium` and `quota-firefox`, so that no other run starts it. Not a
// `webServer` of the configuration, which every run would start. `serve.sh quota` (after
// the `webServer` of the journeys: their builds and plugins) recreates its database; this
// writes its data set through the API, as global-setup.ts does for the other one; the
// project `quota-instance-stop` (quota.teardown.ts) stops it, by its PID.

const repository = fileURLToPath(new URL("../../../..", import.meta.url));

setup("the instance at the read quota by default, its data set", async () => {
  setup.setTimeout(120_000);
  // The instance of an interrupted run, if it still holds the port.
  await stopQuotaInstance();
  // Detached: it outlives the worker of this project, until quota.teardown.ts. What
  // serve.sh says before kartenn-server starts goes to the log the instance writes then.
  const log = openSync(`${STATE}kartenn-v2-quota.log`, "w");
  const server = spawn("dash", ["web/v2/e2e/serve.sh", "quota"], {
    cwd: repository,
    detached: true,
    stdio: ["ignore", log, log],
  });
  closeSync(log);
  server.unref();
  writeFileSync(QUOTA_PID, String(server.pid));
  let exited: number | null = null;
  server.on("exit", (code) => (exited = code ?? -1));

  const probe = await request.newContext({ baseURL: QUOTA_URL, ignoreHTTPSErrors: true });
  try {
    const ready = Date.now() + 60_000;
    for (;;) {
      if (exited !== null) throw new Error(`serve.sh quota exited (${exited})`);
      const status = await probe
        .get("/health/ready", { timeout: 2_000 })
        .then((r) => r.status())
        .catch(() => 0);
      if (status === 200) break;
      if (Date.now() > ready) throw new Error("the quota instance not ready in 60 s");
      await new Promise((done) => setTimeout(done, 250));
    }
  } finally {
    await probe.dispose();
  }

  const api = await breakglass(QUOTA_URL, "breakglass-v2-quota.txt");
  try {
    await expectOk(api, "/v1/issuer-attachments?approval=self", {
      motive: "end-to-end journey of the quota",
      issuer: ISSUER,
    });
    // The users of the journey, one by browser (QUOTA_USERS), each its own quota (by
    // identity): their groups of users.toml and the roles they hold on the other instance.
    await expectOk(api, "/v1/changesets?approval=self", {
      motive: "end-to-end journey of the quota",
      operations: Object.entries({ "kartenn-admins": "administrator", lecteurs: "reader" }).flatMap(
        ([group, role]) => [
          { op: "set_group", key: group, labels: { fr: group } },
          {
            op: "add_member",
            group,
            member: { kind: "idp_group", issuer: ISSUER, group },
          },
          { op: "bind", group, role, scope: { kind: "global" } },
        ],
      ),
    });
    // Ten pages of addresses in a network, a text property inherited from their domain;
    // the domain owned by an entity (`core.owner`, mandatory on a domain).
    const entity = await created(api, "/v1/entities?approval=self", {
      motive: "end-to-end ten pages",
      slug: QUOTA_DATA.domain,
    });
    const declarations = (await (await api.get("/v1/property-declarations")).json()) as {
      items: { id: string; key: string }[];
    };
    const owner = declarations.items.find((d) => d.key === "core.owner")?.id;
    if (!owner) throw new Error("no declaration of core.owner");
    await expectOk(api, "/v1/changesets?approval=self", {
      motive: "end-to-end ten pages",
      operations: [
        { op: "create_domain", local: 0, slug: QUOTA_DATA.domain },
        {
          op: "assign_property",
          object: { local: 0 },
          kind: "domain",
          property: { id: owner },
          values: [entity],
        },
        {
          op: "declare_property",
          local: 1,
          key: QUOTA_DATA.property,
          type: "text",
          max_len: 16,
          cardinality: "single",
          carriers: ["domain", "prefix", "address"],
          inherit: "addressing",
        },
        {
          op: "assign_property",
          object: { local: 0 },
          kind: "domain",
          property: { local: 1 },
          values: [QUOTA_DATA.value],
        },
        { op: "create_prefix", domain: { local: 0 }, prefix: QUOTA_DATA.prefix, nature: "network" },
        ...quotaAddresses().map((address) => ({
          op: "create_address",
          domain: { local: 0 },
          address,
          usage: "host",
        })),
      ],
    });
  } finally {
    await api.dispose();
  }
});
