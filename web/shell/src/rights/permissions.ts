// Rights in the interface (REQ-007-20, design « Droits dans l'interface »): an aid to show
// or hide; the server stays the only judge, its 403 shown as a refusal.

/**
 * Whether the role entry `entry` covers `permission`: `*` covers everything, a `*` segment
 * covers one segment (`prop.*.write` covers `prop.core.owner.write`, whatever the dots of
 * the key), an entry without `*` covers itself only.
 */
export function covers(entry: string, permission: string): boolean {
  if (entry === "*" || entry === permission) return true;
  if (!entry.includes("*")) return false;
  const [head, tail] = entry.split("*", 2) as [string, string];
  return (
    permission.length > head.length + tail.length &&
    permission.startsWith(head) &&
    permission.endsWith(tail)
  );
}

/** The entries held, on any scope, and whether one of them covers a permission. */
export class Rights {
  readonly entries: readonly string[];

  constructor(entries: Iterable<string>) {
    this.entries = [...new Set(entries)];
  }

  has(permission: string): boolean {
    return this.entries.some((entry) => covers(entry, permission));
  }

  /** Any of `permissions`. */
  hasAny(permissions: readonly string[]): boolean {
    return permissions.some((p) => this.has(p));
  }
}

export const NO_RIGHTS = new Rights([]);
