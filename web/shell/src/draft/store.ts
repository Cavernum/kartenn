import type { Operation } from "@kartenn/sdk";

// The drafts of the shell in IndexedDB (W10, design « Écriture » : brouillon): one per
// identity and tab, keyed `<identity>:<tab>`, indexed by identity. Each call opens the
// database and closes it when its transaction ends: a logout that deletes the database is
// never blocked by a connection left open.

export const DATABASE = "kartenn-drafts";
export const STORE = "drafts";
const BY_IDENTITY = "identity";

/** Drafts untouched for longer are cleared at startup. */
export const MAX_AGE_MS = 30 * 24 * 3600 * 1000;

/** A draft as stored. */
export interface DraftRecord {
  identity: string;
  tab: string;
  motive: string;
  valid_from: string | null;
  valid_to: string | null;
  operations: Operation[];
  /** The key of the last submission sent, replayed while the content does not change. */
  idempotency_key: string | null;
  /** The content the key was sent with. */
  content_hash: string | null;
  updated_at: number;
}

export function keyOf(identity: string, tab: string): string {
  return `${identity}:${tab}`;
}

/** An empty draft of `identity` in `tab`. */
export function emptyDraft(identity: string, tab: string, now = Date.now()): DraftRecord {
  return {
    identity,
    tab,
    motive: "",
    valid_from: null,
    valid_to: null,
    operations: [],
    idempotency_key: null,
    content_hash: null,
    updated_at: now,
  };
}

function done<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function open(factory: IDBFactory): Promise<IDBDatabase> {
  const request = factory.open(DATABASE, 1);
  request.onupgradeneeded = () => {
    const store = request.result.createObjectStore(STORE);
    store.createIndex(BY_IDENTITY, "identity");
  };
  return done(request).then((db) => {
    // A logout in another tab deletes the database: let it through (REQ-007-28).
    db.onversionchange = () => db.close();
    return db;
  });
}

/** Runs `work` in one transaction on the store, then closes the database. */
async function within<T>(
  factory: IDBFactory,
  mode: IDBTransactionMode,
  work: (store: IDBObjectStore) => Promise<T>,
): Promise<T> {
  const db = await open(factory);
  try {
    const transaction = db.transaction(STORE, mode);
    const ended = new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error ?? new Error("transaction failed"));
      transaction.onabort = () => reject(transaction.error ?? new Error("transaction aborted"));
    });
    const result = await work(transaction.objectStore(STORE));
    await ended;
    return result;
  } finally {
    db.close();
  }
}

export function load(
  factory: IDBFactory,
  identity: string,
  tab: string,
): Promise<DraftRecord | null> {
  return within(factory, "readonly", async (store) => {
    const value = (await done(store.get(keyOf(identity, tab)))) as DraftRecord | undefined;
    return value ?? null;
  });
}

export function save(factory: IDBFactory, record: DraftRecord): Promise<void> {
  // A plain copy: the record of the shell is reactive (a proxy), which IndexedDB's
  // structured clone refuses; a draft is JSON through and through.
  const plain = JSON.parse(JSON.stringify(record)) as DraftRecord;
  return within(factory, "readwrite", async (store) => {
    await done(store.put(plain, keyOf(plain.identity, plain.tab)));
  });
}

export function remove(factory: IDBFactory, identity: string, tab: string): Promise<void> {
  return within(factory, "readwrite", async (store) => {
    await done(store.delete(keyOf(identity, tab)));
  });
}

/** The drafts of `identity`, every tab. */
export function ofIdentity(factory: IDBFactory, identity: string): Promise<DraftRecord[]> {
  return within(
    factory,
    "readonly",
    (store) => done(store.index(BY_IDENTITY).getAll(identity)) as Promise<DraftRecord[]>,
  );
}

/** Deletes the drafts whose identity matches `doomed`, by a key cursor: never read. */
function purge(factory: IDBFactory, doomed: (identity: string) => boolean): Promise<number> {
  return within(factory, "readwrite", (store) => {
    return new Promise<number>((resolve, reject) => {
      let count = 0;
      const cursor = store.index(BY_IDENTITY).openKeyCursor();
      cursor.onerror = () => reject(cursor.error ?? new Error("cursor failed"));
      cursor.onsuccess = () => {
        const at = cursor.result;
        if (!at) return resolve(count);
        if (doomed(String(at.key))) {
          store.delete(at.primaryKey);
          count += 1;
        }
        at.continue();
      };
    });
  });
}

/** At a login: the drafts of any other identity go, unread (REQ-007-27). */
export function purgeOthers(factory: IDBFactory, identity: string): Promise<number> {
  return purge(factory, (other) => other !== identity);
}

/** At a logout: the drafts of the identity leaving. */
export function purgeIdentity(factory: IDBFactory, identity: string): Promise<number> {
  return purge(factory, (other) => other === identity);
}

/** Drafts of `identity` older than `maxAge`, deleted; the others returned. */
export async function purgeOld(
  factory: IDBFactory,
  identity: string,
  now = Date.now(),
  maxAge = MAX_AGE_MS,
): Promise<DraftRecord[]> {
  const drafts = await ofIdentity(factory, identity);
  const kept: DraftRecord[] = [];
  for (const draft of drafts) {
    if (now - draft.updated_at > maxAge) await remove(factory, draft.identity, draft.tab);
    else kept.push(draft);
  }
  return kept;
}
