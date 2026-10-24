// @kartenn/ui as the shell shares it with its plugins (ADR 0038; spec 021, REQ-021-26): an
// entry of the build, named by the import map, whose sheet is emitted apart and linked by
// `index.html` with its digest.
import "@kartenn/ui/style.css";

export * from "@kartenn/ui";
