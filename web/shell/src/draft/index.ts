import type { Operation } from "@kartenn/sdk";
import { draftHost } from "@kartenn/sdk/host";
import { reactive, watch } from "vue";
import { useSession } from "../rights/index.ts";
import { shellFetch } from "../session/index.ts";
import {
  emptyDraft,
  load,
  purgeOld,
  purgeOthers,
  remove,
  save,
  type DraftRecord,
} from "./store.ts";
import { browserChannel, Tab } from "./tab.ts";

// The draft (REQ-007-26, 27; W10; design « Écriture » : brouillon): operations gathered
// in the browser, simulated after each change, submitted as one changeset with an
// idempotency key kept until the answer. Nothing leaves the browser but the simulations
// and the submission.

/** A violation as the API serves it (REQ-006-21). */
export interface DraftViolation {
  rule: string;
  severity: string;
  detail: string;
  operation?: number | null;
  pointer?: string | null;
}

/** What a simulation reports (`ChangesetSimulated`), or the violations of a 422. */
export interface Report {
  valid: boolean;
  violations: DraftViolation[];
  diff: { property: string; count: number; before?: string | null; after?: string | null }[];
  cascaded: { kind: string; id: string }[];
}

export type Simulation =
  | { status: "idle" }
  | { status: "running"; hash: string }
  | { status: "done"; hash: string; report: Report }
  | { status: "failed"; hash: string; detail: string };

export type Submission =
  | { status: "idle" }
  | { status: "sending" }
  | { status: "applied" | "pending"; changeset: string | null }
  | { status: "invalid"; report: Report }
  | { status: "failed"; network: boolean; detail: string };

export interface DraftState {
  /** False without IndexedDB (strict private browsing): the quick edit only. */
  available: boolean;
  record: DraftRecord | null;
  simulation: Simulation;
  submission: Submission;
  /** The drawer of the draft, open. */
  open: boolean;
}

/** A stable digest of what a submission carries (FNV-1a, 64 bits, in hexadecimal). */
export function contentHash(record: DraftRecord): string {
  const text = JSON.stringify([
    record.motive,
    record.valid_from,
    record.valid_to,
    record.operations,
  ]);
  let hash = 0xcbf29ce484222325n;
  for (let i = 0; i < text.length; i += 1) {
    hash ^= BigInt(text.charCodeAt(i));
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, "0");
}

/** The key of a submission: the one already sent while the content is the same. */
export function keyFor(record: DraftRecord, hash: string, newKey: () => string): string {
  return record.content_hash === hash && record.idempotency_key ? record.idempotency_key : newKey();
}

/** The body of the submission of a draft (`Submission` of the contract). */
export function bodyOf(record: DraftRecord): Record<string, unknown> {
  return {
    motive: record.motive,
    operations: record.operations,
    ...(record.valid_from ? { valid_from: record.valid_from } : {}),
    ...(record.valid_to ? { valid_to: record.valid_to } : {}),
  };
}

function reportOf(body: unknown): Report {
  const value = (body ?? {}) as Partial<Report>;
  return {
    valid: value.valid === true,
    violations: Array.isArray(value.violations) ? value.violations : [],
    diff: Array.isArray(value.diff) ? value.diff : [],
    cascaded: Array.isArray(value.cascaded) ? value.cascaded : [],
  };
}

function changesetOf(body: unknown): string | null {
  const changeset = (body as { changeset?: { id?: unknown } } | null)?.changeset;
  return typeof changeset?.id === "string" ? changeset.id : null;
}

function detailOf(body: unknown, fallback: string): string {
  const detail = (body as { detail?: unknown; title?: unknown } | null) ?? {};
  if (typeof detail.detail === "string") return detail.detail;
  if (typeof detail.title === "string") return detail.title;
  return fallback;
}

export interface DraftDeps {
  factory: IDBFactory | null;
  fetcher: typeof fetch;
  tab: () => string;
  /** Quiet time before a simulation (design: 500 ms). */
  delay?: number;
  newKey?: () => string;
  now?: () => number;
  /** Waits before another try of a key still in progress (409). */
  pause?: (ms: number) => Promise<void>;
}

export class DraftController {
  readonly state: DraftState = reactive({
    available: true,
    record: null,
    simulation: { status: "idle" },
    submission: { status: "idle" },
    open: false,
  });
  private timer: ReturnType<typeof setTimeout> | null = null;
  private running: AbortController | null = null;
  private saving: Promise<void> = Promise.resolve();
  private readonly delay: number;
  private readonly newKey: () => string;
  private readonly now: () => number;
  private readonly pause: (ms: number) => Promise<void>;

  constructor(private readonly deps: DraftDeps) {
    this.delay = deps.delay ?? 500;
    this.newKey = deps.newKey ?? (() => crypto.randomUUID());
    this.now = deps.now ?? (() => Date.now());
    this.pause = deps.pause ?? ((ms) => new Promise((resolve) => setTimeout(resolve, ms)));
    this.state.available = deps.factory !== null;
  }

  /** At a login of `identity`: other identities' drafts go, this tab's comes back. */
  async start(identity: string): Promise<void> {
    const factory = this.deps.factory;
    if (!factory) return;
    try {
      await purgeOthers(factory, identity);
      await purgeOld(factory, identity, this.now());
      const tab = this.deps.tab();
      this.state.record = (await load(factory, identity, tab)) ?? emptyDraft(identity, tab);
    } catch {
      this.state.available = false;
      this.state.record = null;
    }
    this.publish();
    this.schedule();
  }

  /** At a logout, or when no one is logged in. */
  stop(): void {
    this.cancel();
    this.state.record = null;
    this.state.simulation = { status: "idle" };
    this.state.submission = { status: "idle" };
    this.publish();
  }

  /** A duplicated tab: the draft of the original, copied under the new identifier. */
  async copyTo(from: string, to: string): Promise<void> {
    const record = this.state.record;
    const factory = this.deps.factory;
    if (!record || !factory || record.tab !== from) return;
    const copy: DraftRecord = { ...record, tab: to, idempotency_key: null, content_hash: null };
    this.state.record = copy;
    await this.write(() => save(factory, copy));
  }

  get operations(): readonly Operation[] {
    return this.state.record?.operations ?? [];
  }

  add(operation: Operation): Promise<void> {
    return this.change((r) => ({ ...r, operations: [...r.operations, operation] }));
  }

  replace(index: number, operation: Operation): Promise<void> {
    return this.change((r) => ({
      ...r,
      operations: r.operations.map((op, i) => (i === index ? operation : op)),
    }));
  }

  removeAt(index: number): Promise<void> {
    return this.change((r) => ({ ...r, operations: r.operations.filter((_, i) => i !== index) }));
  }

  setMotive(motive: string): Promise<void> {
    return this.change((r) => ({ ...r, motive }));
  }

  setPeriod(validFrom: string | null, validTo: string | null): Promise<void> {
    return this.change((r) => ({ ...r, valid_from: validFrom, valid_to: validTo }));
  }

  /** Empties the draft (the user's choice), its key forgotten. */
  async clear(): Promise<void> {
    const record = this.state.record;
    if (!record) return;
    await this.change(() => emptyDraft(record.identity, record.tab, this.now()));
  }

  /** May the draft be submitted: simulated as it stands, valid, with a motive. */
  canSubmit(writable: boolean): boolean {
    const record = this.state.record;
    const simulation = this.state.simulation;
    return (
      writable &&
      record !== null &&
      record.operations.length > 0 &&
      record.motive.trim() !== "" &&
      this.state.submission.status !== "sending" &&
      simulation.status === "done" &&
      simulation.hash === contentHash(record) &&
      simulation.report.valid
    );
  }

  /** Simulates now (after the quiet time, or on demand). */
  async simulate(): Promise<void> {
    const record = this.state.record;
    this.cancel();
    if (!record || record.operations.length === 0) {
      this.state.simulation = { status: "idle" };
      return;
    }
    const hash = contentHash(record);
    const running = new AbortController();
    this.running = running;
    this.state.simulation = { status: "running", hash };
    try {
      const response = await this.deps.fetcher("/v1/changesets?simulate=true", {
        method: "POST",
        headers: { Accept: "application/json", "Content-Type": "application/json" },
        body: JSON.stringify(bodyOf(record)),
        signal: running.signal,
      });
      const body = await response.json().catch(() => null);
      if (running.signal.aborted) return;
      if (response.ok || response.status === 422) {
        this.state.simulation = { status: "done", hash, report: reportOf(body) };
      } else {
        this.state.simulation = {
          status: "failed",
          hash,
          detail: detailOf(body, response.statusText),
        };
      }
    } catch (error) {
      if (running.signal.aborted) return;
      this.state.simulation = { status: "failed", hash, detail: String(error) };
    } finally {
      if (this.running === running) this.running = null;
    }
  }

  /**
   * Submits the draft as one changeset. The key is written in the draft before it leaves:
   * a lost answer, a retry, a reload replay the same key, so one changeset at most.
   */
  async submit(writable: boolean): Promise<void> {
    if (!this.canSubmit(writable)) return;
    const factory = this.deps.factory;
    let record = this.state.record!;
    const hash = contentHash(record);
    record = { ...record, idempotency_key: keyFor(record, hash, this.newKey), content_hash: hash };
    this.state.record = record;
    if (factory) await this.write(() => save(factory, record));
    this.state.submission = { status: "sending" };
    for (let attempt = 0; ; attempt += 1) {
      const response = await this.deps
        // Spec 024 (REQ-024-13): applied at once by who holds approval.self, pending otherwise.
        .fetcher("/v1/changesets?approval=self", {
          method: "POST",
          headers: {
            Accept: "application/json",
            "Content-Type": "application/json",
            "Idempotency-Key": record.idempotency_key!,
          },
          body: JSON.stringify(bodyOf(record)),
        })
        .catch(() => null);
      if (!response) {
        this.state.submission = { status: "failed", network: true, detail: "network" };
        return;
      }
      const body = await response.json().catch(() => null);
      if (response.status === 201 || response.status === 202) {
        const changeset = changesetOf(body);
        if (factory) await this.write(() => remove(factory, record.identity, record.tab));
        this.state.record = emptyDraft(record.identity, record.tab, this.now());
        this.state.simulation = { status: "idle" };
        this.state.submission = {
          status: response.status === 201 ? "applied" : "pending",
          changeset,
        };
        this.publish();
        return;
      }
      if (response.status === 409 && attempt < 5) {
        await this.pause(1000);
        continue;
      }
      if (response.status === 422) {
        const report = reportOf(body);
        this.state.submission = { status: "invalid", report };
        this.state.simulation = { status: "done", hash, report: { ...report, valid: false } };
        return;
      }
      this.state.submission = {
        status: "failed",
        network: false,
        detail: detailOf(body, response.statusText),
      };
      return;
    }
  }

  private async change(mutate: (record: DraftRecord) => DraftRecord): Promise<void> {
    const current = this.state.record;
    if (!current) return;
    const next = { ...mutate(current), updated_at: this.now() };
    this.state.record = next;
    if (this.state.submission.status !== "sending") this.state.submission = { status: "idle" };
    this.publish();
    this.schedule();
    const factory = this.deps.factory;
    if (factory) await this.write(() => save(factory, next));
  }

  /** Writes in the order of the changes: a slow one never overwrites a later one. */
  private write(work: () => Promise<void>): Promise<void> {
    this.saving = this.saving.then(work).catch(() => undefined);
    return this.saving;
  }

  /** Resolves once every change made so far is in IndexedDB. */
  saved(): Promise<void> {
    return this.saving;
  }

  private publish(): void {
    draftHost.set(this.state.record?.operations ?? []);
  }

  private cancel(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
    this.running?.abort();
    this.running = null;
  }

  private schedule(): void {
    this.cancel();
    if (!this.state.record || this.state.record.operations.length === 0) {
      this.state.simulation = { status: "idle" };
      return;
    }
    this.timer = setTimeout(() => {
      this.timer = null;
      void this.simulate();
    }, this.delay);
  }
}

function browserFactory(): IDBFactory | null {
  try {
    return globalThis.indexedDB ?? null;
  } catch {
    return null;
  }
}

function tabStorage(): Storage | null {
  try {
    return globalThis.sessionStorage ?? null;
  } catch {
    return null;
  }
}

let controller: DraftController | null = null;

/** The draft of the shell (one per document). */
export function useDraft(): DraftController {
  if (!controller) {
    let tab: Tab | null = null;
    controller = new DraftController({
      factory: browserFactory(),
      fetcher: shellFetch,
      tab: () => tab!.id,
    });
    const draft = controller;
    tab = new Tab({
      storage: tabStorage(),
      channel: browserChannel(),
      onDuplicate: (from, to) => void draft.copyTo(from, to),
    });
  }
  return controller;
}

/**
 * Follows the session: a login loads the draft of the identity in this tab (others'
 * erased), a logout lets it go; a plugin's `draft.add` lands in it and opens the drawer.
 */
export function installDraft(): DraftController {
  const draft = useDraft();
  draftHost.onAdd((operation) => {
    void draft.add(operation);
    draft.state.open = true;
  });
  const session = useSession();
  watch(
    () => session.value.me?.identity ?? null,
    (identity, before) => {
      if (identity === before) return;
      if (identity) void draft.start(identity);
      else draft.stop();
    },
    { immediate: true },
  );
  return draft;
}
