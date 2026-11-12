/**
 * The provenance of an assignment, as the server stores it (REQ-003-14; ADR 0026,
 * amendment 14): typed by the author of its changeset (`input`), the default of a source of
 * expected values (`source:<id>@<version>`, REQ-003-22), or derived by a module
 * (`module:<module>#<motif>`). A motif the shell knows is said in words — for `kube`,
 * `label:<key>`, the value read from a label of the workload; any other motif is shown as
 * written, named by its module.
 */
export type Provenance =
  | { kind: "input" }
  | { kind: "source"; code: string }
  | { kind: "label"; module: string; key: string }
  | { kind: "module"; module: string; motif: string }
  | { kind: "other"; code: string };

/** The modules whose motif `label:<key>` names a label of the object they read. */
const LABEL_MODULES: ReadonlySet<string> = new Set(["kube"]);
const LABEL = "label:";

/** The provenance `stored` read; `null` when the server gives none. */
export function provenanceOf(stored: string | null | undefined): Provenance | null {
  if (!stored) return null;
  if (stored === "input") return { kind: "input" };
  if (stored.startsWith("source:")) return { kind: "source", code: stored };
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
