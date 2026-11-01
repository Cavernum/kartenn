import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import ts from "typescript";
import { PREFIX_EXCEPTIONS } from "../stylelint/prefixes.js";
import {
  buildGraph,
  closure,
  MODULE_EXTENSIONS,
  type Closure,
  type Cut,
  type Graph,
  type Package,
  type Root,
  type Sources,
} from "./graph.ts";

// The targets of the Playwright project `visual` a change reaches (spec 029, task 2; ADR
// 0042, decision 1): the pages of the catalogue (index, tokens, icons, one per component),
// the mock-ups (one per `MOCKUPS` identifier), the keyboard scripts and the transverse
// specs; or everything, for a transverse file. In doubt, the selection widens.

/** The workspace of the new shell: the parent of `tools/`. */
export const WORKSPACE = fileURLToPath(new URL("../..", import.meta.url)).replace(/\/$/, "");

/** A spec, or a part of one, to run: `grep` selects its tests (Playwright `--grep`). */
export interface Target {
  /** `catalogue` (a page of catalogue.spec.ts), `ecran` (a mock-up), `spec` (a whole file). */
  kind: "catalogue" | "ecran" | "spec";
  /** Page (`index`, `jetons`, `icones`, a component), mock-up identifier, or spec path. */
  name: string;
  /** The spec file, relative to `e2e/`. */
  spec: string;
  /** A regular expression over the title Playwright greps (` <project> <file> <titles>`). */
  grep: string;
}

export interface Impact {
  /** Everything: a transverse file, or a file the graph cannot place. */
  all: boolean;
  /** Why everything, and the doubts that widened the selection. */
  reasons: string[];
  /** Sorted by spec, then name; empty when `all`. */
  targets: Target[];
}

export const CATALOGUE_SPEC = "visual/catalogue.spec.ts";
export const ECRANS_SPEC = "visual/ecrans.spec.ts";
const UI_PAGES = "shell/src/_ui";
const MAIN = "shell/src/main.ts";
const ECRAN_PAGE = `${UI_PAGES}/ecrans/EcranPage.vue`;
const CATALOGUE_MODULE = "ui/src/catalogue.ts";
const COMPONENTS = "ui/src/components";
/**
 * A catalogue file, as `catalogue.ts` globs them (`./components/*\/*.catalogue.ts`): the
 * component is named by the file, whatever its directory.
 */
export const CATALOGUE_FILE = /^ui\/src\/components\/[^/]+\/([^/]+)\.catalogue\.ts$/;
const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
/** The pages of the catalogue that are not a component's. */
export const CATALOGUE_PAGES = ["index", "jetons", "icones"];
/** The directories whose modules make the graph. */
const GRAPH_ROOTS = ["ui", "shell", "sdk", "plugins"];
const SKIPPED = new Set([
  "node_modules",
  "dist",
  ".git",
  "test-results",
  "playwright-report",
  "__screenshots__",
]);

/**
 * Files whose change reaches every capture (ADR 0042, decision 1): manifests and lockfile,
 * compiler and bundler settings, every file of configuration read by its name, the
 * fixtures of Playwright, the tokens, the base sheet and its layers, the fonts, the page
 * and public files of the shell, the sheets left out of kartenn/component-prefix, this
 * selection itself and the prefixes it counts on. Paths relative to the workspace.
 */
const TRANSVERSE: [RegExp, string][] = [
  [/(^|\/)package\.json$/, "a package manifest"],
  [/^pnpm-(lock|workspace)\.yaml$/, "the lockfile or the workspace"],
  [/(^|\/)\.npmrc$/, "the settings of pnpm"],
  [/(^|\/)tsconfig[^/]*\.json$/, "TypeScript settings"],
  [/(^|\/)[^/]+\.config\.[cm]?[jt]s$/, "a configuration (Vite, Playwright, lint, Vitest…)"],
  [/(^|\/)\.(postcssrc|browserslistrc)[^/]*$/, "a configuration read by its name"],
  [/^e2e\/fixtures\//, "the fixtures of Playwright"],
  [/^ui\/src\/style\.css$/, "the sheet of @kartenn/ui"],
  [/^ui\/src\/styles\//, "the base sheet and its layers"],
  [/^ui\/src\/tokens\//, "the tokens"],
  [/^ui\/src\/fonts\//, "the fonts"],
  [/^shell\/index\.html$/, "the page of the shell"],
  [/^shell\/public\//, "the public files of the shell"],
  [/^tools\/impact\//, "the selection of the captures itself"],
  [/^tools\/stylelint\/prefixes\.js$/, "the prefixes of the components' sheets"],
  ...PREFIX_EXCEPTIONS.map((file): [RegExp, string] => [
    new RegExp(`^${escape(file)}$`),
    "a sheet that matches beyond its component (kartenn/component-prefix exception)",
  ]),
];
/** Outside the workspace (paths relative to the repository): Nix, the capture script. */
const TRANSVERSE_OUTSIDE: [RegExp, string][] = [
  [/^devenv\.(nix|lock|yaml)$/, "the environment and the capture script (devenv)"],
];
/**
 * Files no capture reads: documentation, lint and unit tests, the other Playwright
 * projects, the public files of a plugin (no plugin is in the catalogue build).
 */
const INERT = [
  /\.(md|txt)$/i,
  /(^|\/)LICENSE[^/]*$/,
  /^tools\//,
  /(^|\/)test\//,
  /\.test\.ts$/,
  /(^|\/)__snapshots__\//,
  /^(sdk|shell)\/scripts\//,
  /^plugins\/[^/]+\/public\//,
  /^e2e\/(journeys|bench|plugins)\//,
  /^e2e\/(serve\.sh|kartenn\.toml)$/,
];
const isScript = (file: string) =>
  MODULE_EXTENSIONS.some((ext) => file.endsWith(ext)) && !file.endsWith(".css");

/** A page of the catalogue build that a capture or a check opens, and what it loads. */
interface Page {
  key: string;
  closure: Closure;
  /** Paths whose mere presence the page lists (the index lists the catalogue files). */
  listed?: RegExp;
}

/** The workspace read once: its graph, its pages and their closures, its specs. */
export interface Project {
  repository: string;
  /** The workspace, relative to the repository (`web/v2`). */
  prefix: string;
  graph: Graph;
  components: string[];
  mockups: string[];
  pages: Page[];
  /** The sheets of the base sheet (`@import` of `ui/src/style.css`). */
  sheet: Set<string>;
  /**
   * What the entry of the shell loads statically, lazy edges aside: its modules run and
   * their sheets join the entry's on every page. Those of @kartenn/ui aside (free of side
   * effects, their sheets kept to their component by kartenn/component-prefix).
   */
  entry: Set<string>;
  /** Every file a module of the graph points to. */
  referenced: Set<string>;
  /** Spec (relative to `e2e/`) → the pages it opens, catalogue and mock-ups aside. */
  specs: Map<string, Set<string>>;
}

function walk(root: string): string[] {
  const out: string[] = [];
  const visit = (dir: string, rel: string) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (SKIPPED.has(entry.name)) continue;
      const path = rel ? `${rel}/${entry.name}` : entry.name;
      if (entry.isDirectory()) visit(join(dir, entry.name), path);
      else if (entry.isFile()) out.push(path);
    }
  };
  visit(root, "");
  return out.sort();
}

/** The file of an entry of `exports`: a path, or its `import` or `default` condition. */
function exportTarget(value: unknown): string | undefined {
  if (typeof value === "string") return value;
  if (value && typeof value === "object") {
    const conditions = value as { import?: unknown; default?: unknown };
    return exportTarget(conditions.import ?? conditions.default);
  }
  return undefined;
}

function readPackage(dir: string, text: string): Package {
  const json = JSON.parse(text) as {
    name?: string;
    exports?: unknown;
    main?: string;
    sideEffects?: boolean | string[];
    dependencies?: Record<string, string>;
    devDependencies?: Record<string, string>;
    peerDependencies?: Record<string, string>;
    optionalDependencies?: Record<string, string>;
  };
  const exports = new Map<string, string>();
  const raw = json.exports;
  const keys = raw && typeof raw === "object" ? Object.keys(raw) : [];
  if (keys.length > 0 && keys.every((key) => key.startsWith(".")))
    for (const key of keys) {
      const target = exportTarget((raw as Record<string, unknown>)[key]);
      if (target) exports.set(key, target);
    }
  else {
    const target = exportTarget(raw) ?? json.main;
    if (target) exports.set(".", target);
  }
  return {
    dir,
    name: json.name ?? "",
    exports,
    sideEffects: json.sideEffects ?? true,
    dependencies: [
      json.dependencies,
      json.devDependencies,
      json.peerDependencies,
      json.optionalDependencies,
    ].flatMap((deps) => Object.keys(deps ?? {})),
  };
}

/** The sources of the workspace on disk. */
export function diskSources(root = WORKSPACE): Sources {
  const files = walk(root);
  const read = (file: string) => readFileSync(join(root, file), "utf8");
  const packages = files
    .filter((file) => file === "package.json" || file.endsWith("/package.json"))
    .map((file) => readPackage(file === "package.json" ? "" : dirname(file), read(file)));
  return { files, read, packages };
}

/**
 * The boundaries the pages are cut at, each page rooted on its own side: the route
 * components of a route table (rendered on their route only; the captured ones are the
 * pages below), the screens of `EcranPage.vue` (one per mock-up), the catalogue files
 * `catalogue.ts` loads by name (one per component). A route component imported
 * statically still loads with the entry: its module runs and its sheet joins the entry's
 * on every page (`Project.entry`, everything).
 */
const cut: Cut = (from, dep) => {
  if (/^shell\/src\/(router|(.*\/)?routes)\.ts$/.test(from) && dep.target?.endsWith(".vue"))
    return true;
  if (from === ECRAN_PAGE && dep.how === "dynamic") return true;
  return from === CATALOGUE_MODULE && dep.how === "glob";
};
/** The static load of the entry: lazy edges only are left out. */
const lazy: Cut = (_, dep) => dep.how !== "static";
/** For a mock-up whose screen is not found: every screen of EcranPage.vue. */
const cutButScreens: Cut = (from, dep) => from !== ECRAN_PAGE && cut(from, dep);

/** Identifier → screen of the mock-ups (`SCREENS` of `EcranPage.vue`). */
export function screens(text: string): Map<string, string> {
  const out = new Map<string, string>();
  const re = /\b(E\d+)\s*:\s*defineAsyncComponent\(\s*\(\)\s*=>\s*import\(\s*["']\.\/([^"']+)["']/g;
  for (const m of text.matchAll(re)) out.set(m[1]!, `${UI_PAGES}/ecrans/${m[2]!}`);
  return out;
}

/** The identifiers of `MOCKUPS` (registry.ts with registry017.ts), as the captures read it. */
async function mockupIds(root: string): Promise<string[]> {
  const url = pathToFileURL(join(root, UI_PAGES, "ecrans/registry.ts")).href;
  const { MOCKUPS } = (await import(url)) as { MOCKUPS: { id: string }[] };
  return MOCKUPS.map((mockup) => mockup.id);
}

const isLiteral = (node: ts.Node) => ts.isStringLiteralLike(node) || ts.isTemplateExpression(node);

/**
 * Whether every declaration of `name` in the file gives it literal values: a constant
 * (`const url = "…"`) or the variable of a loop over literals (`for (const url of ["…"])`).
 * A parameter, or any other declaration, could hold anything.
 */
function literalVariable(sf: ts.SourceFile, name: string): boolean {
  let found = false;
  let other = false;
  const visit = (node: ts.Node): void => {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.name.text === name) {
      const list = node.parent;
      const loop = ts.isVariableDeclarationList(list) ? list.parent : undefined;
      if (node.initializer && isLiteral(node.initializer)) found = true;
      else if (
        loop &&
        ts.isForOfStatement(loop) &&
        ts.isArrayLiteralExpression(loop.expression) &&
        loop.expression.elements.every(isLiteral)
      )
        found = true;
      else other = true;
    } else if (
      (ts.isParameter(node) || ts.isBindingElement(node)) &&
      ts.isIdentifier(node.name) &&
      node.name.text === name
    )
      other = true;
    ts.forEachChild(node, visit);
  };
  visit(sf);
  return found && !other;
}

/**
 * The pages a spec opens, read from its literals: `/_ui/c/<Nom>`, `/_ui/ecrans/<Id>`, a
 * component name or a mock-up identifier alone (a helper called with it, a list looped
 * over), `MOCKUPS` (every mock-up), the directory of the components (every component).
 * A computed address of a kind of page (`/_ui/c/${x}`, `/_ui/ecrans/${y}`) may open any
 * page of its kind, whatever literals stand beside it. A navigation to an address it
 * cannot read at all, or no page found, reaches every page.
 */
export function specPages(text: string, components: string[], mockups: string[]): Set<string> {
  const sf = ts.createSourceFile("spec.ts", text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  const known = new Set(components);
  const ids = new Set(mockups);
  const pages = new Set<string>();
  let anyComponent = false;
  let anyMockup = false;
  let everyMockup = false;
  let anything = false;

  const literal = (value: string) => {
    if (known.has(value)) pages.add(`catalogue:${value}`);
    if (ids.has(value)) pages.add(`ecran:${value}`);
    for (const m of value.matchAll(/\/_ui\/c\/([A-Za-z]+)/g))
      if (known.has(m[1]!)) pages.add(`catalogue:${m[1]!}`);
      else anyComponent = true;
    for (const m of value.matchAll(/\/_ui\/ecrans\/(E\d+)/g))
      if (ids.has(m[1]!)) pages.add(`ecran:${m[1]!}`);
      else anyMockup = true;
    if (/\/_ui\/jetons/.test(value)) pages.add("catalogue:jetons");
    if (/\/_ui\/icones/.test(value)) pages.add("catalogue:icones");
    if (/\/_ui\/revue/.test(value)) pages.add("revue");
    if (/^\/_ui\/?([?#]|$)/.test(value)) pages.add("catalogue:index");
    if (value.includes(COMPONENTS)) anyComponent = true;
  };
  /**
   * A navigation's address: a literal; a template whose head says the kind of page; a
   * constant, or the variable of a loop over a list of literals, all read as literals.
   */
  const address = (arg: ts.Expression | undefined) => {
    if (!arg || ts.isStringLiteralLike(arg)) return;
    if (ts.isTemplateExpression(arg)) {
      const head = arg.head.text;
      if (head.endsWith("/_ui/c/")) anyComponent = true;
      else if (head.endsWith("/_ui/ecrans/")) anyMockup = true;
      else if (!/\/_ui\/(c|ecrans)\/|\/_ui\/(jetons|icones|revue)\b|^\/_ui([?#]|$)/.test(head))
        anything = true;
      return;
    }
    if (!(ts.isIdentifier(arg) && literalVariable(sf, arg.text))) anything = true;
  };
  const visit = (node: ts.Node): void => {
    if (ts.isStringLiteralLike(node)) literal(node.text);
    else if (ts.isTemplateExpression(node)) {
      literal(node.head.text);
      for (const span of node.templateSpans) literal(span.literal.text);
      // A page computed anywhere (a helper, a variable): any of its kind.
      if (node.head.text.endsWith("/_ui/c/")) anyComponent = true;
      if (node.head.text.endsWith("/_ui/ecrans/")) anyMockup = true;
    } else if (ts.isIdentifier(node) && node.text === "MOCKUPS") everyMockup = true;
    if (ts.isCallExpression(node)) {
      const callee = node.expression;
      const name = ts.isIdentifier(callee)
        ? callee.text
        : ts.isPropertyAccessExpression(callee)
          ? callee.name.text
          : "";
      if (name === "goto") address(node.arguments[0]);
      else if (name === "open") address(node.arguments[1]);
      else if (name === "openState") {
        const arg = node.arguments[1];
        if (!arg || !ts.isStringLiteralLike(arg)) anyComponent = true;
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);

  const add = (keys: string[]) => keys.forEach((key) => pages.add(key));
  const everyComponentPage = components.map((c) => `catalogue:${c}`);
  const everyMockupPage = mockups.map((m) => `ecran:${m}`);
  if (anything || pages.size === 0) {
    add(everyComponentPage);
    add(everyMockupPage);
    add([...CATALOGUE_PAGES.map((page) => `catalogue:${page}`), "revue"]);
  }
  if (anyComponent) add(everyComponentPage);
  if (everyMockup || anyMockup) add(everyMockupPage);
  return pages;
}

/** Component → its catalogue files (`CATALOGUE_FILE`; several only by mistake, all kept). */
export function catalogueFiles(files: string[]): Map<string, string[]> {
  const out = new Map<string, string[]>();
  for (const file of files) {
    const name = CATALOGUE_FILE.exec(file)?.[1];
    if (name !== undefined) out.set(name, [...(out.get(name) ?? []), file]);
  }
  return out;
}

/** Reads the workspace: graph, pages and their closures, specs and the pages they open. */
export async function loadProject(root = WORKSPACE, sources = diskSources(root)): Promise<Project> {
  const graph = buildGraph(sources, GRAPH_ROOTS);
  const has = new Set(sources.files);
  const catalogues = catalogueFiles(sources.files);
  const components = [...catalogues.keys()].sort();
  const mockups = await mockupIds(root);
  const screenOf = screens(sources.read(ECRAN_PAGE));

  const all = (file: string): Root => ({ file, demand: { kind: "all" } });
  // What every page loads: the entry of the shell, evaluated.
  const common: Root[] = [{ file: MAIN, demand: { kind: "eval" } }];
  const layout = [...common, all(`${UI_PAGES}/CatalogueLayout.vue`)];
  const page = (key: string, roots: Root[], how = cut, listed?: RegExp): Page => ({
    key,
    closure: closure(graph, roots, how),
    ...(listed ? { listed } : {}),
  });
  const pages: Page[] = [
    page("catalogue:index", [...layout, all(`${UI_PAGES}/IndexPage.vue`)], cut, CATALOGUE_FILE),
    page("catalogue:jetons", [...layout, all(`${UI_PAGES}/TokensPage.vue`)]),
    page("catalogue:icones", [...layout, all(`${UI_PAGES}/IconsPage.vue`)]),
    page("revue", [...layout, all(`${UI_PAGES}/RevuePage.vue`)]),
    ...components.map((name) =>
      page(`catalogue:${name}`, [
        ...layout,
        all(`${UI_PAGES}/ComponentPage.vue`),
        ...catalogues.get(name)!.map(all),
      ]),
    ),
    ...mockups.map((id) => {
      const screen = screenOf.get(id);
      return screen !== undefined && has.has(screen)
        ? page(`ecran:${id}`, [...common, all(ECRAN_PAGE), all(screen)])
        : page(`ecran:${id}`, [...common, all(ECRAN_PAGE)], cutButScreens);
    }),
  ];

  const referenced = new Set<string>();
  for (const module of graph.modules.values()) {
    for (const dep of module.deps) if (dep.target !== null) referenced.add(dep.target);
    for (const re of module.reexports.values()) if (re.target !== null) referenced.add(re.target);
    for (const star of module.stars) if (star !== null) referenced.add(star);
  }

  const specs = new Map<string, Set<string>>();
  for (const file of sources.files) {
    const spec = /^e2e\/((?:visual|components)\/.+\.spec\.ts)$/.exec(file)?.[1];
    if (spec === undefined || spec === CATALOGUE_SPEC || spec === ECRANS_SPEC) continue;
    specs.set(spec, specPages(sources.read(file), components, mockups));
  }

  const repository = repositoryOf(root);
  return {
    repository,
    prefix: root === repository ? "" : root.slice(repository.length + 1),
    graph,
    components,
    mockups,
    pages,
    sheet: closure(graph, [all("ui/src/style.css")]).files,
    entry: new Set([...closure(graph, common, lazy).files].filter((f) => !f.startsWith("ui/"))),
    referenced,
    specs,
  };
}

/** The repository of the workspace (`.git`: a directory, or the file of a worktree). */
function repositoryOf(root: string): string {
  for (let dir = root; dir !== dirname(dir); dir = dirname(dir))
    if (existsSync(join(dir, ".git"))) return dir;
  return root;
}

/** The target of a page of the catalogue or of a mock-up; null for a page no capture opens. */
function pageTarget(key: string): Target | null {
  const [kind, name = ""] = key.split(":");
  if (kind === "ecran")
    return {
      kind: "ecran",
      name,
      spec: ECRANS_SPEC,
      grep: ` ${escape(ECRANS_SPEC)} \\d+ px ${escape(name)} `,
    };
  if (kind !== "catalogue") return null;
  // catalogue.spec.ts: « mode <m> › index | tokens, <d> | icons », then « <Nom> › … ».
  const titles: Record<string, string> = {
    index: "mode \\w+ index( |$)",
    jetons: "mode \\w+ tokens, ",
    icones: "mode \\w+ icons( |$)",
  };
  return {
    kind: "catalogue",
    name,
    spec: CATALOGUE_SPEC,
    grep: ` ${escape(CATALOGUE_SPEC)} ${titles[name] ?? `${escape(name)} `}`,
  };
}

const specTarget = (spec: string): Target => ({
  kind: "spec",
  name: spec,
  spec,
  grep: ` ${escape(spec)} `,
});

/** The page of a reference of the catalogue or the mock-ups, from its name; else null. */
function referencePage(project: Project, spec: string, name: string): string | null {
  if (spec === ECRANS_SPEC) {
    const id = /^(E\d+)-/.exec(name)?.[1];
    return id !== undefined && project.mockups.includes(id) ? `ecran:${id}` : null;
  }
  if (spec !== CATALOGUE_SPEC) return null;
  const page = CATALOGUE_PAGES.find((p) => name.startsWith(`${p}-`));
  if (page) return `catalogue:${page}`;
  const component = project.components
    .filter((c) => name.startsWith(`${c}-`))
    .sort((a, b) => b.length - a.length)[0];
  return component === undefined ? null : `catalogue:${component}`;
}

/**
 * The targets `changed` reaches: paths relative to the repository, as `git diff
 * --name-only` gives them, deleted files included.
 */
export function impact(changed: string[], project: Project): Impact {
  const reasons: string[] = [];
  /** Pages whose content changed: their captures and the specs that open them. */
  const touched = new Set<string>();
  /** Pages whose references changed: their captures alone. */
  const recaptured = new Set<string>();
  const whole = new Set<string>();
  const everything = (reason: string): Impact => ({ all: true, reasons: [reason], targets: [] });
  const prefix = project.prefix ? `${project.prefix}/` : "";

  for (const path of changed) {
    if (!path.startsWith(prefix)) {
      const outside = TRANSVERSE_OUTSIDE.find(([re]) => re.test(path));
      if (outside) return everything(`${path}: ${outside[1]}`);
      continue;
    }
    const file = path.slice(prefix.length);
    const transverse = TRANSVERSE.find(([re]) => re.test(file));
    if (transverse) return everything(`${path}: ${transverse[1]}`);
    if (project.sheet.has(file)) return everything(`${path}: a sheet of @kartenn/ui's base`);
    if (project.entry.has(file))
      return everything(`${path}: loaded by the entry of the shell, on every page`);

    if (file.startsWith("e2e/")) {
      // A reference: its page for the catalogue and the mock-ups, else its whole spec.
      const reference =
        /^e2e\/((?:visual|components)\/(?:.+\/)?)__screenshots__\/([^/]+)\/(.+)$/.exec(file);
      const spec = /^e2e\/((?:visual|components)\/.+\.spec\.ts)$/.exec(file)?.[1];
      if (reference) {
        const owner = `${reference[1]!}${reference[2]!}`;
        const page = referencePage(project, owner, reference[3]!);
        if (page !== null) recaptured.add(page);
        else whole.add(owner);
      } else if (spec !== undefined) whole.add(spec);
      else if (!INERT.some((re) => re.test(file)))
        return everything(`${path}: a file of the Playwright project of the captures`);
      continue;
    }

    const inert = INERT.some((re) => re.test(file));
    let reached = false;
    for (const { key, closure: c, listed } of project.pages) {
      if (c.files.has(file) || listed?.test(file)) touched.add(key);
      else if (c.doubts.length > 0 && !inert) {
        // A page whose imports are not all read may load this file too.
        touched.add(key);
        for (const doubt of c.doubts) reasons.push(`${key} widened: ${doubt}`);
      } else continue;
      reached = true;
    }
    if (reached || inert) continue;
    // A script no captured page loads (a view of the shell, a helper of the unit tests, a
    // deleted file) changes no capture; a sheet or an asset no module points to may be
    // loaded by what the graph does not read.
    if (isScript(file) || project.referenced.has(file)) continue;
    return everything(`${path}: a file the import graph does not read`);
  }

  const targets = new Map<string, Target>();
  for (const key of [...touched, ...recaptured]) {
    const target = pageTarget(key);
    if (target) targets.set(`${target.spec} ${target.name}`, target);
  }
  for (const [spec, pages] of project.specs)
    if ([...pages].some((page) => touched.has(page))) whole.add(spec);
  for (const spec of whole) {
    for (const [key, target] of targets) if (target.spec === spec) targets.delete(key);
    targets.set(`${spec} `, specTarget(spec));
  }
  return {
    all: false,
    reasons: [...new Set(reasons)],
    targets: [...targets.keys()].sort().map((key) => targets.get(key)!),
  };
}

/** One target per line, `<spec>\t<grep>`; `all` alone for everything. */
export function formatLines(result: Impact): string {
  if (result.all) return "all\n";
  return result.targets.map((target) => `${target.spec}\t${target.grep}\n`).join("");
}
