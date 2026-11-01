import ts from "typescript";

// The import graph of the sources of web/v2 (spec 029, task 2; ADR 0042, decision 1): what
// each module imports, symbol by symbol through the re-exports of a barrel, and the
// closure of what a page loads. Pure: the files come from a `Sources`, read from the disk
// by `impact.ts` or written by hand in the tests.

/** What an importer asks of a module. */
export type Demand =
  /** Named bindings (`import { a } from`, `default` for a default import). */
  | { kind: "names"; names: string[] }
  /** The whole namespace (`import * as`, `import()`, a glob). */
  | { kind: "all" }
  /** Its evaluation only (`import "x"`, an import whose bindings are all types). */
  | { kind: "eval" };

/** An import of a module, resolved. */
export interface Dep {
  /** The file imported, relative to the root; null for a package outside the workspace. */
  target: string | null;
  demand: Demand;
  how: "static" | "dynamic" | "glob";
}

export interface Module {
  file: string;
  /** What its body imports (`import` declarations, `import()`, globs, assets). */
  deps: Dep[];
  /** `export { a as b } from "x"`: b → (x, a); `export * as ns from "x"`: ns → (x, "*"). */
  reexports: Map<string, { target: string | null; name: string }>;
  /** `export * from "x"`. */
  stars: (string | null)[];
  /** Names it exports from its own body (values and types). */
  locals: Set<string>;
  /** The imports it could not resolve: whoever loads it may load anything. */
  doubts: string[];
}

/** A package of the workspace, as its `package.json` declares it. */
export interface Package {
  /** Its directory, relative to the root ("" for the root). */
  dir: string;
  name: string;
  /** Subpath ("." or "./x") → file, relative to the package. */
  exports: Map<string, string>;
  /** `sideEffects`: false, a list of globs, or true when absent. */
  sideEffects: boolean | string[];
  /** Its dependencies of every kind. */
  dependencies: string[];
}

/** The files of the workspace, relative to its root, with forward slashes. */
export interface Sources {
  files: string[];
  read(file: string): string;
  packages: Package[];
}

export interface Graph {
  modules: Map<string, Module>;
  /** Whether evaluating the file alone has no effect (its package's `sideEffects`). */
  sideEffectFree(file: string): boolean;
}

/** Extensions of the modules the graph parses. */
export const MODULE_EXTENSIONS = [".ts", ".mts", ".js", ".mjs", ".vue", ".css"];
const RESOLVED_EXTENSIONS = [".ts", ".vue", ".js", ".mjs", ".json"];
const BUILTINS = new Set(["fs", "path", "url", "crypto", "child_process", "os", "util"]);

const dirOf = (file: string) => file.slice(0, Math.max(0, file.lastIndexOf("/")));

/** `a/b/../c/./d` → `a/c/d`; null when it climbs out of the root. */
export function normalize(path: string): string | null {
  const out: string[] = [];
  for (const part of path.split("/")) {
    if (part === "" || part === ".") continue;
    if (part === "..") {
      if (out.length === 0) return null;
      out.pop();
    } else out.push(part);
  }
  return out.join("/");
}

/** A glob (`*`, `**`, `{a,b}`) as a regular expression over paths. */
export function globToRegExp(glob: string): RegExp {
  let re = "";
  for (let i = 0; i < glob.length; i++) {
    const c = glob[i]!;
    if (c === "*" && glob[i + 1] === "*") {
      re += glob[i + 2] === "/" ? "(?:.*/)?" : ".*";
      i += glob[i + 2] === "/" ? 2 : 1;
    } else if (c === "*") re += "[^/]*";
    else if (c === "?") re += "[^/]";
    else if (c === "{") re += "(?:";
    else if (c === "}") re += ")";
    else if (c === ",") re += "|";
    else re += c.replace(/[.+^$()|[\]\\]/g, "\\$&");
  }
  return new RegExp(`^${re}$`);
}

type Resolution = { target: string | null } | { doubt: string };

/** Resolves module specifiers as Vite does for this workspace (no alias). */
export class Resolver {
  private readonly files: Set<string>;
  private readonly byName = new Map<string, Package>();
  private readonly declared = new Set<string>();

  constructor(sources: Sources) {
    this.files = new Set(sources.files);
    for (const pkg of sources.packages) {
      if (pkg.name) this.byName.set(pkg.name, pkg);
      for (const dep of pkg.dependencies) this.declared.add(dep);
    }
  }

  resolve(from: string, specifier: string): Resolution {
    const spec = specifier.replace(/[?#].*$/, "");
    if (spec === "") return { doubt: `empty specifier "${specifier}"` };
    if (spec.startsWith("node:") || BUILTINS.has(spec)) return { target: null };
    if (spec.startsWith("./") || spec.startsWith("../")) {
      const base = normalize(`${dirOf(from)}/${spec}`);
      const found = base === null ? undefined : this.file(base);
      return found ? { target: found } : { doubt: `unresolved "${specifier}"` };
    }
    if (spec.startsWith("/") || specifier.startsWith("#"))
      return { doubt: `unknown alias "${specifier}"` };
    const parts = spec.split("/");
    const name = spec.startsWith("@") ? parts.slice(0, 2).join("/") : parts[0]!;
    const subpath = `.${spec.slice(name.length)}`;
    const pkg = this.byName.get(name);
    if (pkg) {
      const entry = pkg.exports.get(subpath);
      const file = entry === undefined ? null : normalize(`${pkg.dir}/${entry}`);
      return file !== null && this.files.has(file)
        ? { target: file }
        : { doubt: `no export "${subpath}" in ${name}` };
    }
    if (this.declared.has(name)) return { target: null };
    return { doubt: `unknown package or alias "${specifier}"` };
  }

  private file(base: string): string | undefined {
    if (this.files.has(base)) return base;
    for (const ext of RESOLVED_EXTENSIONS) if (this.files.has(base + ext)) return base + ext;
    for (const ext of RESOLVED_EXTENSIONS)
      if (this.files.has(`${base}/index${ext}`)) return `${base}/index${ext}`;
    return undefined;
  }
}

/**
 * The blocks of a single-file component: its scripts, its sheets, and the files a block
 * takes from elsewhere (`src`). Scripts and sheets do not nest; templates do, hence a
 * search for their `src` alone.
 */
function vueParts(source: string): { scripts: string[]; srcs: string[]; styles: string[] } {
  const scripts: string[] = [];
  const styles: string[] = [];
  const srcs: string[] = [];
  const src = (attributes: string) => /\bsrc\s*=\s*["']([^"']+)["']/.exec(attributes)?.[1];
  for (const m of source.matchAll(/<(script|style)\b([^>]*)>([\s\S]*?)<\/\1\s*>/g)) {
    const from = src(m[2]!);
    if (from !== undefined) srcs.push(from);
    else (m[1] === "script" ? scripts : styles).push(m[3]!);
  }
  for (const m of source.matchAll(/<template\b([^>]*)>/g)) {
    const from = src(m[1]!);
    if (from !== undefined) srcs.push(from);
  }
  return { scripts, srcs, styles };
}

/** `@import` and `url()` of a sheet: the files it loads. */
function cssRefs(source: string): string[] {
  const refs: string[] = [];
  const clean = source.replace(/\/\*[\s\S]*?\*\//g, "");
  for (const m of clean.matchAll(/@import\s+(?:url\(\s*)?["']?([^"')\s;]+)/g)) refs.push(m[1]!);
  for (const m of clean.matchAll(/url\(\s*["']?([^"')]+?)["']?\s*\)/g)) refs.push(m[1]!);
  return [...new Set(refs)].filter((ref) => !/^(data:|https?:|#|\/\/)/.test(ref));
}

const isTypeImport = (clause: ts.ImportClause) =>
  clause.phaseModifier === ts.SyntaxKind.TypeKeyword;

function bindingNames(name: ts.BindingName, out: Set<string>): void {
  if (ts.isIdentifier(name)) out.add(name.text);
  else
    for (const element of name.elements)
      if (!ts.isOmittedExpression(element)) bindingNames(element.name, out);
}

/** The string literals of a glob's first argument, or null when it is computed. */
function globPatterns(arg: ts.Expression | undefined): string[] | null {
  if (arg && ts.isStringLiteralLike(arg)) return [arg.text];
  if (arg && ts.isArrayLiteralExpression(arg)) {
    const out: string[] = [];
    for (const element of arg.elements) {
      if (!ts.isStringLiteralLike(element)) return null;
      out.push(element.text);
    }
    return out;
  }
  return null;
}

const isImportMeta = (node: ts.Node) =>
  ts.isMetaProperty(node) && node.keywordToken === ts.SyntaxKind.ImportKeyword;

/** Reads one script (a module, or a script block of a component) into `module`. */
function scanScript(
  module: Module,
  text: string,
  resolve: (spec: string) => Resolution,
  files: string[],
): void {
  const sf = ts.createSourceFile(module.file, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
  const dep = (spec: string, demand: Demand, how: Dep["how"]) => {
    const r = resolve(spec);
    if ("doubt" in r) module.doubts.push(r.doubt);
    else module.deps.push({ target: r.target, demand, how });
  };

  for (const st of sf.statements) {
    if (ts.isImportDeclaration(st)) {
      if (!ts.isStringLiteral(st.moduleSpecifier)) continue;
      const spec = st.moduleSpecifier.text;
      const clause = st.importClause;
      if (clause && isTypeImport(clause)) continue;
      const names: string[] = [];
      let namespace = false;
      if (clause?.name) names.push("default");
      const bindings = clause?.namedBindings;
      if (bindings && ts.isNamespaceImport(bindings)) namespace = true;
      else if (bindings)
        for (const element of bindings.elements)
          if (!element.isTypeOnly) names.push((element.propertyName ?? element.name).text);
      // verbatimModuleSyntax keeps an import whose bindings are all types as `import "x"`.
      const demand: Demand = namespace
        ? { kind: "all" }
        : names.length > 0
          ? { kind: "names", names }
          : { kind: "eval" };
      dep(spec, demand, "static");
    } else if (ts.isExportDeclaration(st)) {
      const clause = st.exportClause;
      if (!st.moduleSpecifier) {
        if (clause && ts.isNamedExports(clause))
          for (const element of clause.elements) module.locals.add(element.name.text);
        continue;
      }
      if (!ts.isStringLiteral(st.moduleSpecifier)) continue;
      const spec = st.moduleSpecifier.text;
      if (st.isTypeOnly) {
        if (clause && ts.isNamedExports(clause))
          for (const element of clause.elements) module.locals.add(element.name.text);
        continue;
      }
      const r = resolve(spec);
      if ("doubt" in r) {
        module.doubts.push(r.doubt);
        continue;
      }
      if (!clause) module.stars.push(r.target);
      else if (ts.isNamespaceExport(clause))
        module.reexports.set(clause.name.text, { target: r.target, name: "*" });
      else
        for (const element of clause.elements) {
          if (element.isTypeOnly) module.locals.add(element.name.text);
          else
            module.reexports.set(element.name.text, {
              target: r.target,
              name: (element.propertyName ?? element.name).text,
            });
        }
    } else if (ts.isExportAssignment(st)) module.locals.add("default");
    else if (ts.canHaveModifiers(st)) {
      const mods = ts.getModifiers(st) ?? [];
      if (!mods.some((m) => m.kind === ts.SyntaxKind.ExportKeyword)) continue;
      if (mods.some((m) => m.kind === ts.SyntaxKind.DefaultKeyword)) module.locals.add("default");
      else if (ts.isVariableStatement(st))
        for (const decl of st.declarationList.declarations) bindingNames(decl.name, module.locals);
      else if ("name" in st && st.name && ts.isIdentifier(st.name as ts.Node))
        module.locals.add((st.name as ts.Identifier).text);
    }
  }

  const visit = (node: ts.Node): void => {
    if (ts.isCallExpression(node)) {
      const callee = node.expression;
      if (callee.kind === ts.SyntaxKind.ImportKeyword) {
        const arg = node.arguments[0];
        if (arg && ts.isStringLiteralLike(arg)) dep(arg.text, { kind: "all" }, "dynamic");
        else module.doubts.push(`computed import(${arg?.getText(sf) ?? ""})`);
      } else if (
        ts.isPropertyAccessExpression(callee) &&
        callee.name.text === "glob" &&
        isImportMeta(callee.expression)
      ) {
        const patterns = globPatterns(node.arguments[0]);
        const options = node.arguments[1];
        const eager =
          options !== undefined &&
          ts.isObjectLiteralExpression(options) &&
          options.properties.some(
            (p) =>
              ts.isPropertyAssignment(p) &&
              p.name.getText(sf) === "eager" &&
              p.initializer.kind === ts.SyntaxKind.TrueKeyword,
          );
        if (patterns === null) module.doubts.push("computed import.meta.glob");
        else
          for (const pattern of patterns) {
            if (pattern.startsWith("!")) continue;
            if (!pattern.startsWith("./") && !pattern.startsWith("../")) {
              module.doubts.push(`glob outside the module's directory "${pattern}"`);
              continue;
            }
            const full = normalize(`${dirOf(module.file)}/${pattern}`);
            if (full === null) {
              module.doubts.push(`glob "${pattern}"`);
              continue;
            }
            const re = globToRegExp(full);
            for (const file of files)
              if (re.test(file))
                module.deps.push({
                  target: file,
                  demand: { kind: "all" },
                  how: eager ? "static" : "glob",
                });
          }
      }
    } else if (
      ts.isNewExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === "URL" &&
      node.arguments?.length === 2 &&
      ts.isPropertyAccessExpression(node.arguments[1]!) &&
      isImportMeta((node.arguments[1] as ts.PropertyAccessExpression).expression)
    ) {
      const arg = node.arguments[0]!;
      if (ts.isStringLiteralLike(arg))
        dep(arg.text.startsWith(".") ? arg.text : `./${arg.text}`, { kind: "all" }, "static");
      else module.doubts.push(`computed new URL(…, import.meta.url)`);
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
}

/** Reads one file into a module. */
export function parseModule(
  file: string,
  text: string,
  resolver: Resolver,
  files: string[],
): Module {
  const module: Module = {
    file,
    deps: [],
    reexports: new Map(),
    stars: [],
    locals: new Set(),
    doubts: [],
  };
  const resolve = (spec: string) => resolver.resolve(file, spec);
  // A sheet loads what it imports and the files its `url()` names: their content counts.
  const sheet = (css: string) => {
    for (const ref of cssRefs(css)) {
      const r = resolve(/^[./]/.test(ref) ? ref : `./${ref}`);
      if ("doubt" in r) module.doubts.push(r.doubt);
      else module.deps.push({ target: r.target, demand: { kind: "all" }, how: "static" });
    }
  };
  if (file.endsWith(".css")) sheet(text);
  else if (file.endsWith(".vue")) {
    const { scripts, srcs, styles } = vueParts(text);
    module.locals.add("default");
    for (const script of scripts) scanScript(module, script, resolve, files);
    for (const style of styles) sheet(style);
    for (const src of srcs) {
      const r = resolve(src.startsWith(".") ? src : `./${src}`);
      if ("doubt" in r) module.doubts.push(r.doubt);
      else module.deps.push({ target: r.target, demand: { kind: "all" }, how: "static" });
    }
  } else scanScript(module, text, resolve, files);
  return module;
}

/** Whether a file matches a `sideEffects` glob of its package, as bundlers read them. */
function matchesSideEffects(relative: string, globs: string[]): boolean {
  return globs.some((glob) => {
    const pattern = glob.replace(/^\.\//, "");
    return globToRegExp(pattern.includes("/") ? pattern : `**/${pattern}`).test(relative);
  });
}

/** The graph of every module of `sources` under `roots` (directories). */
export function buildGraph(sources: Sources, roots: string[]): Graph {
  const resolver = new Resolver(sources);
  const modules = new Map<string, Module>();
  const inRoots = (file: string) => roots.some((root) => file.startsWith(`${root}/`));
  for (const file of sources.files) {
    if (!inRoots(file) || !MODULE_EXTENSIONS.some((ext) => file.endsWith(ext))) continue;
    if (file.endsWith(".d.ts")) continue;
    modules.set(file, parseModule(file, sources.read(file), resolver, sources.files));
  }
  const packages = [...sources.packages].sort((a, b) => b.dir.length - a.dir.length);
  const packageOf = (file: string) =>
    packages.find((pkg) => pkg.dir === "" || file.startsWith(`${pkg.dir}/`));
  return {
    modules,
    sideEffectFree(file) {
      const pkg = packageOf(file);
      if (!pkg || pkg.sideEffects === true) return false;
      if (pkg.sideEffects === false) return true;
      const relative = pkg.dir === "" ? file : file.slice(pkg.dir.length + 1);
      return !matchesSideEffects(relative, pkg.sideEffects);
    },
  };
}

/** A root of a closure: a file and what is asked of it. */
export interface Root {
  file: string;
  demand: Demand;
}

/** An edge left out of a closure: a lazy boundary handled by the caller. */
export type Cut = (from: string, dep: Dep) => boolean;

export interface Closure {
  /** Every file whose content can change what the roots render. */
  files: Set<string>;
  /** The imports it could not follow, as `file: reason`: the closure may hold anything. */
  doubts: string[];
}

/**
 * The files the roots load, following the bindings they ask for: a named import through a
 * barrel reaches the module that defines the name, not its siblings. Evaluating a module
 * its package declares free of side effects (`sideEffects`) loads nothing: its code runs
 * only where a binding of it is used.
 */
export function closure(graph: Graph, roots: Root[], cut: Cut = () => false): Closure {
  const files = new Set<string>();
  const doubts: string[] = [];
  const bodies = new Set<string>();
  const whole = new Set<string>();
  const evaluated = new Set<string>();
  const named = new Map<string, Set<string>>();
  const stack: { file: string; demand: Demand }[] = [...roots];

  /** Whether `file` exports `name`, through its stars too; true where it cannot be read. */
  const provides = (file: string, name: string, seen = new Set<string>()): boolean => {
    const module = graph.modules.get(file);
    if (!module) return true;
    if (seen.has(file)) return false;
    seen.add(file);
    if (module.reexports.has(name) || module.locals.has(name)) return true;
    return module.stars.some((star) => star === null || provides(star, name, seen));
  };

  const body = (module: Module) => {
    if (bodies.has(module.file)) return;
    bodies.add(module.file);
    files.add(module.file);
    for (const doubt of module.doubts) doubts.push(`${module.file}: ${doubt}`);
    for (const dep of module.deps)
      if (dep.target !== null && !cut(module.file, dep))
        stack.push({ file: dep.target, demand: dep.demand });
  };
  const ask = (target: string | null, name: string) => {
    if (target === null) return;
    stack.push({
      file: target,
      demand: name === "*" ? { kind: "all" } : { kind: "names", names: [name] },
    });
  };

  while (stack.length > 0) {
    const { file, demand } = stack.pop()!;
    const module = graph.modules.get(file);
    if (!module) {
      // A leaf: an asset, a JSON file, a module outside the parsed directories.
      if (demand.kind !== "eval" || !graph.sideEffectFree(file)) files.add(file);
      continue;
    }
    if (demand.kind === "eval") {
      if (graph.sideEffectFree(file) || evaluated.has(file)) continue;
      evaluated.add(file);
      body(module);
      for (const re of module.reexports.values())
        if (re.target !== null) stack.push({ file: re.target, demand });
      for (const star of module.stars) if (star !== null) stack.push({ file: star, demand });
      continue;
    }
    if (demand.kind === "all") {
      body(module);
      if (whole.has(file)) continue;
      whole.add(file);
      for (const re of module.reexports.values()) ask(re.target, re.name);
      for (const star of module.stars) if (star !== null) stack.push({ file: star, demand });
      continue;
    }
    const done = named.get(file) ?? new Set<string>();
    named.set(file, done);
    for (const name of demand.names) {
      if (done.has(name)) continue;
      done.add(name);
      const re = module.reexports.get(name);
      if (re) {
        body(module);
        ask(re.target, re.name);
      } else if (module.locals.has(name)) body(module);
      else if (module.stars.some((star) => star !== null && provides(star, name))) {
        body(module);
        for (const star of module.stars) if (star !== null && provides(star, name)) ask(star, name);
      } else if (!module.stars.includes(null)) {
        // A name the module does not export, as far as it can be read: all of it.
        doubts.push(`${file}: no export "${name}"`);
        stack.push({ file, demand: { kind: "all" } });
      } else body(module);
    }
  }
  return { files, doubts };
}
