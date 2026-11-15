import { test as teardown } from "@playwright/test";
import { stopQuotaInstance } from "../fixtures/quota.ts";

// The instance at the read quota by default stopped once its journey is done (the project
// `quota-instance-stop`, the teardown of `quota-instance`; task 021-11p10-b2).
teardown("the instance at the read quota by default stopped", async () => {
  await stopQuotaInstance();
});
