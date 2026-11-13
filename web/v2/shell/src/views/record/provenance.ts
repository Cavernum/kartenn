import type { components } from "@kartenn/sdk";

/**
 * The provenance of an assignment, as the server stores it (REQ-003-14; ADR 0026,
 * amendment 14): typed by the author of its changeset (`input`), the default of a source of
 * expected values (`source:<id>@<revision>`, `#<detail>` after it, REQ-003-22, REQ-016-5),
 * or derived by a module (`module:<module>#<motif>`). A motif the shell knows is said in
 * words — for `kube`, `label:<key>`, the value read from a label of the workload; any other
 * motif is shown as written, named by its module.
 */
export type Provenance =
  | { kind: "input" }
  | { kind: "source"; code: string; source: string; revision: string | null }
  | { kind: "label"; module: string; key: string }
  | { kind: "module"; module: string; motif: string }
  | { kind: "other"; code: string };

/** The modules whose motif `label:<key>` names a label of the object they read. */
const LABEL_MODULES: ReadonlySet<string> = new Set(["kube"]);
const LABEL = "label:";
const SOURCE = "source:";

/** The provenance `stored` read; `null` when the server gives none. */
export function provenanceOf(stored: string | null | undefined): Provenance | null {
  if (!stored) return null;
  if (stored === "input") return { kind: "input" };
  if (stored.startsWith(SOURCE)) {
    const rest = stored.slice(SOURCE.length);
    const hash = rest.indexOf("#");
    const head = hash >= 0 ? rest.slice(0, hash) : rest;
    const at = head.lastIndexOf("@");
    if (at === 0 || !head) return { kind: "other", code: stored };
    return at > 0
      ? { kind: "source", code: stored, source: head.slice(0, at), revision: head.slice(at + 1) }
      : { kind: "source", code: stored, source: head, revision: null };
  }
  if (stored.startsWith("module:")) {
    const rest = stored.slice("module:".length);
    const hash = rest.indexOf("#");
    const module = hash > 0 ? rest.slice(0, hash) : "";
    const motif = hash > 0 ? rest.slice(hash + 1) : "";
    if (module && motif) {
      const key = motif.startsWith(LABEL) ? motif.slice(LABEL.length) : "";
      return LABEL_MODULES.has(module) && key
        ? { kind: "label", module, key }
        : { kind: "module", module, motif };
    }
  }
  return { kind: "other", code: stored };
}

type ValueSource = components["schemas"]["ValueSource"];

/**
 * Where a resolved value comes from, as the column « Source » of the properties says it
 * (task 021-11p4; maquette E2): set on the object itself; inherited from its origin, named;
 * inherited from an object the caller may not read (its existence not disclosed,
 * REQ-005-19); or, on the object itself, the default of a source or derived by a module —
 * said in its place of « direct ». Inherited, the origin is said before the provenance:
 * the explanation tells both.
 */
export type ValueOrigin =
  | { kind: "direct" }
  | { kind: "inherited"; origin: components["schemas"]["NodeRef"] }
  | { kind: "unreadable" }
  | { kind: "derived"; provenance: Exclude<Provenance, { kind: "input" }> };

/** The origin of a value from its `sources` entry; `null` without one (a masked key). */
export function valueOriginOf(source: ValueSource | null | undefined): ValueOrigin | null {
  if (!source) return null;
  if (source.origin === null) return { kind: "unreadable" };
  if (source.inherited) return { kind: "inherited", origin: source.origin };
  const provenance = provenanceOf(source.provenance);
  return provenance === null || provenance.kind === "input"
    ? { kind: "direct" }
    : { kind: "derived", provenance };
}
