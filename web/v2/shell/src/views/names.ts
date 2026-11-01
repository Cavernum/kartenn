import { shallowReactive } from "vue";

// The names of the records shown, by identifier, for the breadcrumb (REQ-021-4): a record
// says its object's name once read (`192.0.2.0/24` rather than its UUID), the crumb of
// its route reads it here. Reactive: the breadcrumb follows when the name arrives.

const names = shallowReactive(new Map<string, string>());

/** The name of the object `id` as its record read it, `undefined` until then. */
export function recordName(id: string): string | undefined {
  return names.get(id);
}

/** Keeps the name of a record; the oldest are forgotten past a few hundred. */
export function keepRecordName(id: string, name: string): void {
  if (names.get(id) === name) return;
  names.delete(id);
  names.set(id, name);
  if (names.size > 500) names.delete(names.keys().next().value!);
}
