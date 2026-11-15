import { existsSync, readFileSync, rmSync } from "node:fs";
import { STATE } from "./env.ts";

// The process of the instance at the read quota by default (task 021-11p10-b2), started by
// journeys/quota.setup.ts and stopped by journeys/quota.teardown.ts.

/** The PID of the instance (`serve.sh quota` execs kartenn-server). */
export const QUOTA_PID = `${STATE}kartenn-v2-quota.pid`;

/**
 * Stops the instance by its PID, if it still runs (an interrupted run leaves it), and waits
 * until it is gone: its port free for the next one.
 */
export async function stopQuotaInstance(): Promise<void> {
  if (!existsSync(QUOTA_PID)) return;
  const pid = Number(readFileSync(QUOTA_PID, "utf8"));
  rmSync(QUOTA_PID);
  const command = `/proc/${pid}/cmdline`;
  if (!Number.isInteger(pid) || pid <= 0 || !existsSync(command)) return;
  // The same PID taken since by another process: not ours, left alone.
  if (!readFileSync(command, "utf8").includes("quota")) return;
  process.kill(pid, "SIGTERM");
  const until = Date.now() + 10_000;
  while (existsSync(command) && Date.now() < until) {
    await new Promise((done) => setTimeout(done, 100));
  }
}
