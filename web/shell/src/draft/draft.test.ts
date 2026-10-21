import { IDBFactory, IDBIndex, IDBObjectStore } from "fake-indexeddb";
import { afterEach, describe, expect, it, vi } from "vitest";
import { contentHash, DraftController, keyFor } from "./index.ts";
import {
  emptyDraft,
  load,
  MAX_AGE_MS,
  ofIdentity,
  purgeOld,
  purgeOthers,
  save,
  type DraftRecord,
} from "./store.ts";
import { Tab, TAB_KEY, type Channel } from "./tab.ts";

/** A `Storage` over a map, the test environment having none. */
class MemoryStorage implements Storage {
  constructor(private readonly items = new Map<string, string>()) {}
  get length(): number {
    return this.items.size;
  }
  clear(): void {
    this.items.clear();
  }
  getItem(key: string): string | null {
    return this.items.get(key) ?? null;
  }
  key(index: number): string | null {
    return [...this.items.keys()][index] ?? null;
  }
  removeItem(key: string): void {
    this.items.delete(key);
  }
  setItem(key: string, value: string): void {
    this.items.set(key, value);
  }
  copy(): MemoryStorage {
    return new MemoryStorage(new Map(this.items));
  }
}

/** The channels of one browser: a message reaches every other one, synchronously. */
class Bus {
  private readonly channels: Channel[] = [];
  channel(): Channel {
    const channels = this.channels;
    const channel: Channel = {
      onmessage: null,
      postMessage(data) {
        for (const other of [...channels]) if (other !== channel) other.onmessage?.({ data });
      },
    };
    channels.push(channel);
    return channel;
  }
}

function record(identity: string, tab: string, operations = 1): DraftRecord {
  return {
    ...emptyDraft(identity, tab),
    operations: Array.from({ length: operations }, (_, i) => ({ op: "noop", i }) as never),
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("the store of drafts (REQ-007-26, 27)", () => {
  it("keys a draft by identity and tab", async () => {
    const factory = new IDBFactory();
    await save(factory, record("alice", "t1", 1));
    await save(factory, record("alice", "t2", 2));
    await save(factory, record("bob", "t1", 3));
    expect((await load(factory, "alice", "t1"))?.operations).toHaveLength(1);
    expect((await load(factory, "alice", "t2"))?.operations).toHaveLength(2);
    expect((await load(factory, "bob", "t1"))?.operations).toHaveLength(3);
    expect(await load(factory, "bob", "t2")).toBeNull();
    expect(await ofIdentity(factory, "alice")).toHaveLength(2);
  });

  it("erases the drafts of another identity without reading them", async () => {
    const factory = new IDBFactory();
    await save(factory, record("alice", "t1"));
    await save(factory, record("bob", "t1"));
    await save(factory, record("bob", "t2"));
    const reads = [
      vi.spyOn(IDBObjectStore.prototype, "get"),
      vi.spyOn(IDBObjectStore.prototype, "getAll"),
      vi.spyOn(IDBObjectStore.prototype, "openCursor"),
      vi.spyOn(IDBIndex.prototype, "get"),
      vi.spyOn(IDBIndex.prototype, "getAll"),
      vi.spyOn(IDBIndex.prototype, "openCursor"),
    ];
    expect(await purgeOthers(factory, "alice")).toBe(2);
    for (const read of reads) expect(read).not.toHaveBeenCalled();
    vi.restoreAllMocks();
    expect(await ofIdentity(factory, "bob")).toEqual([]);
    expect(await ofIdentity(factory, "alice")).toHaveLength(1);
  });

  it("clears the drafts left for more than 30 days", async () => {
    const factory = new IDBFactory();
    const now = 1_000 * MAX_AGE_MS;
    await save(factory, { ...record("alice", "old"), updated_at: now - MAX_AGE_MS - 1 });
    await save(factory, { ...record("alice", "new"), updated_at: now - 1 });
    const kept = await purgeOld(factory, "alice", now);
    expect(kept.map((draft) => draft.tab)).toEqual(["new"]);
    expect(await load(factory, "alice", "old")).toBeNull();
  });
});

describe("the tab of a draft", () => {
  it("keeps its identifier in the storage of the tab", () => {
    const storage = new MemoryStorage();
    const tab = new Tab({ storage, channel: null, newId: () => "t1" });
    expect(tab.id).toBe("t1");
    expect(storage.getItem(TAB_KEY)).toBe("t1");
    expect(new Tab({ storage, channel: null, newId: () => "other" }).id).toBe("t1");
  });

  it("gives a duplicated tab a new identifier and a copy of the draft", () => {
    const bus = new Bus();
    const storage = new MemoryStorage();
    let n = 0;
    const newId = () => `id${(n += 1)}`;
    const original = new Tab({ storage, channel: bus.channel(), newId });
    const duplicates: [string, string][] = [];
    // A duplicated tab starts with a copy of the original's `sessionStorage`.
    const duplicate = new Tab({
      storage: storage.copy(),
      channel: bus.channel(),
      newId,
      onDuplicate: (from, to) => duplicates.push([from, to]),
    });
    expect(duplicate.id).not.toBe(original.id);
    expect(duplicates).toEqual([[original.id, duplicate.id]]);
    expect(storage.getItem(TAB_KEY)).toBe(original.id);
  });
});

/** A fetch that answers simulations as valid, and submissions by `answer`. */
function api(answer: (key: string | null) => Response | null) {
  const keys: (string | null)[] = [];
  const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    if (String(input).includes("simulate=true")) {
      return Response.json({ valid: true, violations: [], diff: [], cascaded: [], sections: [] });
    }
    const key = new Headers(init?.headers).get("Idempotency-Key");
    keys.push(key);
    const response = answer(key);
    if (!response) throw new TypeError("network");
    return response;
  });
  return { fetcher: fetcher as unknown as typeof fetch, keys };
}

async function started(factory: IDBFactory, fetcher: typeof fetch): Promise<DraftController> {
  let n = 0;
  const draft = new DraftController({
    factory,
    fetcher,
    tab: () => "t1",
    delay: 60_000,
    newKey: () => `key${(n += 1)}`,
  });
  await draft.start("alice");
  await draft.add({ op: "create_domain", local: 0, slug: "lab" } as never);
  await draft.setMotive("a lab");
  await draft.simulate();
  return draft;
}

describe("the submission of a draft", () => {
  it("keeps its key while the content does not change", async () => {
    const factory = new IDBFactory();
    let online = false;
    const { fetcher, keys } = api(() =>
      online ? Response.json({ changeset: { id: "c1" } }, { status: 201 }) : null,
    );
    const draft = await started(factory, fetcher);
    await draft.submit(true);
    expect(draft.state.submission).toMatchObject({ status: "failed", network: true });
    // The key is stored before it leaves: a reload replays it.
    expect((await load(factory, "alice", "t1"))?.idempotency_key).toBe(keys[0]);
    online = true;
    await draft.submit(true);
    expect(keys).toEqual(["key1", "key1"]);
    expect(draft.state.submission).toEqual({ status: "applied", changeset: "c1" });
    expect(draft.state.record?.operations).toEqual([]);
    expect(await load(factory, "alice", "t1")).toBeNull();
  });

  it("takes a new key once the content changes", async () => {
    const factory = new IDBFactory();
    const { fetcher, keys } = api(() => null);
    const draft = await started(factory, fetcher);
    await draft.submit(true);
    await draft.setMotive("another motive");
    await draft.simulate();
    await draft.submit(true);
    expect(keys).toEqual(["key1", "key2"]);
  });

  it("is refused before a valid simulation of the current content, and under as_of", async () => {
    const factory = new IDBFactory();
    const { fetcher } = api(() => null);
    const draft = await started(factory, fetcher);
    expect(draft.canSubmit(false)).toBe(false);
    expect(draft.canSubmit(true)).toBe(true);
    await draft.add({ op: "create_domain", local: 1, slug: "lab2" } as never);
    expect(draft.canSubmit(true)).toBe(false);
  });

  it("derives the key from the content", () => {
    const base = record("alice", "t1");
    const hash = contentHash(base);
    expect(contentHash({ ...base, updated_at: 1 })).toBe(hash);
    expect(contentHash({ ...base, motive: "x" })).not.toBe(hash);
    expect(keyFor({ ...base, idempotency_key: "k", content_hash: hash }, hash, () => "n")).toBe(
      "k",
    );
    expect(keyFor({ ...base, idempotency_key: "k", content_hash: "other" }, hash, () => "n")).toBe(
      "n",
    );
  });
});
