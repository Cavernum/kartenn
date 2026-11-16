// The summary of an object a body names (REQ-006-42): apart from `collections.ts`, so that
// `parts/vlan.ts`, which that module reads, tells one without importing it back.

/**
 * A summary of an object in a field (`{id, label?, slug?, href?}`, REQ-006-42): its natural
 * key in `label`; `label`, `slug` and `href` left out together for an object the caller may
 * not read, or one gone.
 */
export interface Summary {
  id: string;
  label?: string | null;
  slug?: string | null;
  href?: string | null;
  kind?: string | null;
}

/**
 * Whether `value` is a summary of an object: an `id`, and no `system` — an external
 * reference (`{system, id, label?}`) has the other members too, and names no object of
 * kartenn (task 021-11p10-d2).
 */
export function isSummary(value: unknown): value is Summary {
  if (typeof value !== "object" || value === null) return false;
  const members = value as { id?: unknown; system?: unknown };
  return typeof members.id === "string" && members.system === undefined;
}
