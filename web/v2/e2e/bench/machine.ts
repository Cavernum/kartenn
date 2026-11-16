import { readdirSync, readFileSync } from "node:fs";
import { availableParallelism, loadavg } from "node:os";

// What the machine did while the bench measured (task 021-22e-l): its load, the CPU it
// spent, and the part of it that went to the browser, to the servers of the bench, and to
// the rest. Read from /proc on the side of Node, never from the page: the measure stays
// what it was. Linux only; elsewhere every figure is absent and the lines say so.

/** Clock ticks of /proc (`getconf CLK_TCK`, 100 on Linux on every architecture in use). */
const TICK = 100;

function read(path: string): string | undefined {
  try {
    return readFileSync(path, "utf8");
  } catch {
    return undefined;
  }
}

/**
 * The pressure stall information of the kernel (PSI): for the CPU, the I/O and the memory,
 * by `some` (a task waited) and `full` (all of them did), the part of the last 60 s spent
 * waiting (`avg60`, in %) and the time waited since boot (`total`, in µs).
 */
function pressure(): Map<string, { avg60: number | undefined; total: number }> {
  const stalls = new Map<string, { avg60: number | undefined; total: number }>();
  for (const resource of ["cpu", "io", "memory"])
    for (const line of read(`/proc/pressure/${resource}`)?.split("\n") ?? []) {
      const [kind] = line.split(" ");
      const total = /total=(\d+)/.exec(line)?.[1];
      const avg60 = /avg60=([\d.]+)/.exec(line)?.[1];
      if (kind && total)
        stalls.set(`${resource} ${kind}`, {
          avg60: avg60 === undefined ? undefined : Number(avg60),
          total: Number(total),
        });
    }
  return stalls;
}

/**
 * The load averages over 1, 5 and 15 min, and the CPUs they weigh against (on Linux the load
 * counts the tasks waiting on I/O too); then the stalls of the kernel over the last 60 s
 * (PSI `avg60`), which tell a CPU taken from an I/O waited for.
 */
export function loadLine(): string {
  const [one, five, fifteen] = loadavg().map((v) => v.toFixed(2));
  const stalls = [...pressure()]
    .map(([k, { avg60 }]) => `${k} ${avg60 === undefined ? "unknown" : `${avg60.toFixed(2)} %`}`)
    .join(", ");
  return (
    `load ${one} ${five} ${fifteen} (1, 5, 15 min) on ${availableParallelism()} CPUs;` +
    ` stalls over 60 s ${stalls || "unknown"}`
  );
}

/** The fields of a /proc stat line after its `(comm)`, and the comm itself. */
function statOf(text: string): { comm: string; fields: string[] } {
  const open = text.indexOf("(");
  const close = text.lastIndexOf(")");
  return {
    comm: text.slice(open + 1, close),
    fields: text
      .slice(close + 2)
      .trim()
      .split(" "),
  };
}

interface Proc {
  ppid: number;
  /** User and system time, in ticks. */
  ticks: number;
  /** Its comm, then its kind for a process of the browser (`kindOf`). */
  kind: string;
  /** Per thread name: ticks (read for the browser's processes only). */
  threads: Map<string, number>;
}

/** Every process of the machine, by pid: its parent, its CPU time, its comm. */
function processes(): Map<number, Proc> {
  const procs = new Map<number, Proc>();
  for (const entry of readdirSync("/proc")) {
    const pid = Number(entry);
    const text = Number.isInteger(pid) ? read(`/proc/${pid}/stat`) : undefined;
    if (!text) continue;
    const { comm, fields } = statOf(text);
    // After the comm: state(0) ppid(1) … utime(11) stime(12).
    procs.set(pid, {
      ppid: Number(fields[1]),
      ticks: Number(fields[11]) + Number(fields[12]),
      kind: comm,
      threads: new Map(),
    });
  }
  return procs;
}

/** The pids under `root`, itself among them. */
function under(procs: Map<number, Proc>, root: number): Set<number> {
  const found = new Set([root]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const [pid, p] of procs)
      if (!found.has(pid) && found.has(p.ppid)) {
        found.add(pid);
        grew = true;
      }
  }
  return found;
}

/** What a Chromium process is, by its `--type=` (none: the browser process itself). */
function kindOf(pid: number, comm: string): string {
  // Split on spaces as well: a child of Chromium rewrites its title into one argument.
  const cmdline = read(`/proc/${pid}/cmdline`)?.split(/[\0 ]/) ?? [];
  const type = cmdline.find((a) => a.startsWith("--type="))?.slice(7);
  if (type) {
    const sub = cmdline.find((a) => a.startsWith("--utility-sub-type="))?.slice(19);
    return sub ? `${type}:${sub.split(".").at(-1)}` : type;
  }
  return comm;
}

/** The ticks of each thread of `pid`, by thread name. */
function threadsOf(pid: number): Map<string, number> {
  const threads = new Map<string, number>();
  let tasks: string[] = [];
  try {
    tasks = readdirSync(`/proc/${pid}/task`);
  } catch {
    return threads;
  }
  for (const tid of tasks) {
    const text = read(`/proc/${pid}/task/${tid}/stat`);
    if (!text) continue;
    const { comm, fields } = statOf(text);
    // Pools number their threads: `ThreadPoolForeg` and its siblings counted together.
    const name = comm.replace(/\d+$/, "");
    threads.set(name, (threads.get(name) ?? 0) + Number(fields[11]) + Number(fields[12]));
  }
  return threads;
}

/** A sample of the machine: its CPU ticks, every process, the stalls of the kernel. */
interface Sample {
  at: number;
  busy: number;
  total: number;
  procs: Map<number, Proc>;
  pressure: ReturnType<typeof pressure>;
}

function sample(): Sample | undefined {
  const cpu = read("/proc/stat")?.split("\n")[0];
  if (!cpu?.startsWith("cpu ")) return undefined;
  const v = cpu.split(/\s+/).slice(1).map(Number);
  // user nice system idle iowait irq softirq steal: idle and iowait are not busy.
  const total = v.slice(0, 8).reduce((a, b) => a + b, 0);
  const busy = total - v[3]! - v[4]!;
  return { at: performance.now(), busy, total, procs: processes(), pressure: pressure() };
}

/** The browser of this worker: the processes under it, by kind, with their threads. */
function browserOf(procs: Map<number, Proc>, threads: boolean): Map<number, Proc> {
  const ours = new Map<number, Proc>();
  for (const pid of under(procs, process.pid)) {
    if (pid === process.pid) continue;
    const p = procs.get(pid)!;
    ours.set(pid, {
      ...p,
      kind: kindOf(pid, p.kind),
      threads: threads ? threadsOf(pid) : new Map(),
    });
  }
  return ours;
}

/** Every CPU's highest frequency, in kHz (cores of two kinds on a hybrid processor). */
function maxFrequencies(): Map<number, number> {
  const max = new Map<number, number>();
  let dirs: string[] = [];
  try {
    dirs = readdirSync("/sys/devices/system/cpu").filter((d) => /^cpu\d+$/.test(d));
  } catch {
    return max;
  }
  for (const d of dirs) {
    const khz = Number(read(`/sys/devices/system/cpu/${d}/cpufreq/cpuinfo_max_freq`));
    if (khz) max.set(Number(d.slice(3)), khz);
  }
  return max;
}

/**
 * Where the main thread of each renderer of the browser runs, sampled every `every` ms:
 * the CPU it last ran on (field `processor` of its stat), and that CPU's frequency then.
 * A walk of /proc at each sample: off by default (`K_BENCH_CORES=1` turns it on), never in
 * the windows of a verdict.
 */
function sampleCores(every: number) {
  const renderers = new Map<number, { cpu: number; khz: number }[]>();
  const timer = setInterval(() => {
    for (const [pid, p] of browserOf(processes(), false)) {
      if (p.kind !== "renderer") continue;
      const text = read(`/proc/${pid}/stat`);
      if (!text) continue;
      // After the comm, `processor` is the 37th field (39th of the line).
      const cpu = Number(statOf(text).fields[36]);
      const khz = Number(read(`/sys/devices/system/cpu/cpu${cpu}/cpufreq/scaling_cur_freq`));
      renderers.set(pid, [...(renderers.get(pid) ?? []), { cpu, khz }]);
    }
  }, every);
  return {
    /** For the renderer `busiest`: its share of samples on the fastest cores, its mean GHz. */
    stop(busiest: number | undefined): string {
      clearInterval(timer);
      const samples = busiest === undefined ? undefined : renderers.get(busiest);
      if (!samples || samples.length === 0) return "not sampled";
      const max = maxFrequencies();
      const top = Math.max(...max.values());
      const fast = samples.filter((s) => max.get(s.cpu) === top).length;
      const ghz = samples.reduce((sum, s) => sum + s.khz, 0) / samples.length / 1e6;
      return (
        `${((fast / samples.length) * 100).toFixed(0)} % of ${samples.length} samples on the` +
        ` cores of ${(top / 1e6).toFixed(2)} GHz at most, ${ghz.toFixed(2)} GHz on average`
      );
    },
  };
}

/** A window of the machine: started by `watchMachine`, closed by its `stop`. */
export interface Watched {
  /** One line for reading: the machine's CPU, the browser's, the bench's, the rest. */
  stop(): string;
}

/**
 * Watches the machine from now until `stop`: the stalls of the kernel, the CPU of the
 * machine in all, that of the browser of this worker by process kind and its busiest
 * threads, that of the servers of the bench (the runner's other children: kartenn-server,
 * the identity provider), and the rest of the machine, in CPUs on average over the window
 * (1 = one CPU busy all along); with `K_BENCH_CORES=1`, where the page's main thread ran.
 */
export function watchMachine(): Watched {
  const before = sample();
  const browserBefore = before && browserOf(before.procs, true);
  const cores = before && process.env.K_BENCH_CORES === "1" ? sampleCores(50) : undefined;
  return {
    stop() {
      const after = sample();
      if (!before || !after || !browserBefore) {
        cores?.stop(undefined);
        return "no /proc: machine not watched";
      }
      const browserAfter = browserOf(after.procs, true);
      const seconds = (after.at - before.at) / 1000;
      const cpus = (ticks: number) => (ticks / TICK / seconds).toFixed(2);
      const machine = after.busy - before.busy;
      const span = after.total - before.total;
      // By kind of process, and by thread name over the whole browser; the renderer that
      // spent the most CPU over the window is the page's.
      const kinds = new Map<string, number>();
      const threads = new Map<string, number>();
      let browser = 0;
      let busiest: number | undefined;
      let most = -1;
      for (const [pid, p] of browserAfter) {
        const was = browserBefore.get(pid);
        const spent = p.ticks - (was?.ticks ?? 0);
        browser += spent;
        kinds.set(p.kind, (kinds.get(p.kind) ?? 0) + spent);
        if (p.kind === "renderer" && spent > most) [busiest, most] = [pid, spent];
        for (const [name, t] of p.threads) {
          const key = `${p.kind}/${name}`;
          threads.set(key, (threads.get(key) ?? 0) + t - (was?.threads.get(name) ?? 0));
        }
      }
      const placed = cores?.stop(busiest);
      // The servers: the runner's other descendants (`webServer` of the config).
      let servers = 0;
      const runner = after.procs.get(process.pid)?.ppid;
      if (runner !== undefined)
        for (const pid of under(after.procs, runner)) {
          if (pid === runner || browserAfter.has(pid) || pid === process.pid) continue;
          servers += after.procs.get(pid)!.ticks - (before.procs.get(pid)?.ticks ?? 0);
        }
      const workerSpent =
        (after.procs.get(process.pid)?.ticks ?? 0) - (before.procs.get(process.pid)?.ticks ?? 0);
      // Ticks of /proc/stat are per CPU (USER_HZ on each), of the same unit.
      const rest = machine - browser - servers - workerSpent;
      const top = (m: Map<string, number>, n: number) =>
        [...m]
          .filter(([, t]) => t > 0)
          .sort((a, b) => b[1] - a[1])
          .slice(0, n)
          .map(([k, t]) => `${k} ${cpus(t)}`)
          .join(", ");
      // The part of the window stalled, by resource (some task, all of them).
      const stalls = [...after.pressure]
        .map(([k, { total }]) => {
          const was = before.pressure.get(k)?.total ?? total;
          return `${k} ${(((total - was) / 1e6 / seconds) * 100).toFixed(1)} %`;
        })
        .join(", ");
      return (
        `stalls ${stalls || "unknown (no /proc/pressure)"};` +
        ` machine ${cpus(machine)} CPUs busy of ${(span / TICK / seconds).toFixed(0)}` +
        ` (${((machine / span) * 100).toFixed(0)} %); browser ${cpus(browser)}` +
        ` (${top(kinds, 6)}); its busiest threads ${top(threads, 6)}; servers of the bench` +
        ` ${cpus(servers)}, the runner ${cpus(workerSpent)}, the rest of the machine ${cpus(rest)}` +
        (placed === undefined ? "" : `; the page's main thread ${placed}`)
      );
    },
  };
}
