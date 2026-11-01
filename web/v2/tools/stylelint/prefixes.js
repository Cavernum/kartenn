// The classes each unit of @kartenn/ui owns (spec 029, task 2): the library ships one
// sheet, loaded on every page, so the rules of a component must match only what that
// component renders. Read by the rule kartenn/component-prefix (plugins.js) and by the
// impact graph of the captures (tools/impact/impact.ts), which treats a change of a
// component's sheet as a change of that component alone, and an exception as everything.
import { readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** The sources of @kartenn/ui, relative to the workspace (web/v2). */
const UI = "ui/src";
const COMPONENTS = `${UI}/components`;

/** `DataGrid` → `data-grid`. */
export const kebab = (name) => name.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();

/**
 * Prefixes a unit owns beyond `k-<its name>`: the names its classes and keyframes took
 * before this rule. Each prefix belongs to one unit only.
 */
export const EXTRA_PREFIXES = {
  [`${COMPONENTS}/CommandPalette`]: ["k-palette"],
  [`${COMPONENTS}/DataGrid`]: ["k-cell-error", "k-row-fresh"],
  [`${COMPONENTS}/DateTimeField`]: ["k-date-field", "k-date-calendar"],
  [`${COMPONENTS}/MatrixGrid`]: ["k-matrix"],
  [`${COMPONENTS}/SidebarNav`]: ["k-sidebar"],
  [`${COMPONENTS}/Toast`]: ["k-toasts"],
};

/**
 * Sheets whose rules match elements of several units (shared rules, the list of options
 * of Select, Combobox and FilterPicker, the chevron of Menu on IconButton): left out of
 * the rule, and a change of one of them reaches every capture.
 */
export const PREFIX_EXCEPTIONS = [
  `${COMPONENTS}/shared.css`,
  `${COMPONENTS}/Select/options.css`,
  `${COMPONENTS}/Menu/Menu.vue`,
];

/**
 * The unit a sheet of @kartenn/ui belongs to: a component's directory, a file of
 * `components/internal/` or of `demo/` (each its own unit); null outside them, or for an
 * exception.
 */
export function unitOf(file) {
  if (PREFIX_EXCEPTIONS.includes(file)) return null;
  const internal = new RegExp(`^${COMPONENTS}/internal/([^/]+)\\.vue$`).exec(file);
  if (internal) return `${COMPONENTS}/internal/${internal[1]}`;
  const demo = new RegExp(`^${UI}/demo/([^/]+)\\.vue$`).exec(file);
  if (demo) return `${UI}/demo/${demo[1]}`;
  const component = new RegExp(`^${COMPONENTS}/([^/]+)/[^/]+\\.(vue|css)$`).exec(file);
  if (component && component[1] !== "internal") return `${COMPONENTS}/${component[1]}`;
  return null;
}

/** The prefixes of a unit. */
function prefixesOf(unit) {
  const name = unit.split("/").at(-1);
  const own = unit.startsWith(`${UI}/demo/`)
    ? `k-demo-${kebab(name.replace(/^Demo/, ""))}`
    : `k-${kebab(name)}`;
  return [own, ...(EXTRA_PREFIXES[unit] ?? [])];
}

/** Prefix → unit, for the units found under `root` (the workspace). */
export function owners(root) {
  const units = [];
  const list = (dir) =>
    readdirSync(fileURLToPath(new URL(`${dir}/`, root)), { withFileTypes: true });
  for (const entry of list(COMPONENTS))
    if (entry.isDirectory() && entry.name !== "internal") units.push(`${COMPONENTS}/${entry.name}`);
  for (const entry of list(`${COMPONENTS}/internal`))
    if (entry.name.endsWith(".vue"))
      units.push(`${COMPONENTS}/internal/${entry.name.slice(0, -4)}`);
  for (const entry of list(`${UI}/demo`))
    if (entry.name.endsWith(".vue")) units.push(`${UI}/demo/${entry.name.slice(0, -4)}`);
  const table = new Map();
  for (const unit of units)
    for (const prefix of prefixesOf(unit)) {
      if (table.has(prefix)) throw new Error(`prefix ${prefix}: ${table.get(prefix)} and ${unit}`);
      table.set(prefix, unit);
    }
  return table;
}

/**
 * The unit owning a class or keyframes name: that of the longest prefix it starts with,
 * followed by its end, `-` or `_` (`k-tree-grid-frame` is TreeGrid's, not Tree's).
 */
export function ownerOf(table, name) {
  let best = null;
  for (const [prefix, unit] of table) {
    const fits = name === prefix || name.startsWith(`${prefix}-`) || name.startsWith(`${prefix}_`);
    if (fits && (best === null || prefix.length > best.prefix.length)) best = { prefix, unit };
  }
  return best?.unit ?? null;
}
