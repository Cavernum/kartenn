import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { buildGraph, closure, type Package, type Sources } from "../impact/graph.ts";
import {
  CATALOGUE_PAGES,
  CATALOGUE_SPEC,
  ECRANS_SPEC,
  WORKSPACE,
  impact,
  loadProject,
  screens,
  specPages,
  type Project,
} from "../impact/impact.ts";

// The impact graph of the captures (spec 029, task 2; ADR 0042, decision 1): every
// component of the catalogue and every mock-up has its targets; a component reaches the
// mock-ups that import it, a symbol of the entry of @kartenn/ui only its real importers;
// transverse files reach everything; in doubt, the selection widens.

/** In-memory sources: `files` path → text, packages declared by their package.json. */
function sources(files: Record<string, string>): Sources {
  const packages: Package[] = Object.entries(files)
    .filter(([file]) => file.endsWith("package.json"))
    .map(([file, text]) => {
      const json = JSON.parse(text) as {
        name: string;
        exports?: Record<string, string>;
        sideEffects?: boolean | string[];
        dependencies?: Record<string, string>;
      };
      return {
        dir: file.slice(0, -"/package.json".length),
        name: json.name,
        exports: new Map(Object.entries(json.exports ?? {})),
        sideEffects: json.sideEffects ?? true,
        dependencies: Object.keys(json.dependencies ?? {}),
      };
    });
  return { files: Object.keys(files).sort(), read: (file) => files[file]!, packages };
}

const lib = JSON.stringify({
  name: "lib",
  exports: { ".": "./index.ts" },
  sideEffects: ["*.css"],
  dependencies: { vue: "*" },
});
const app = JSON.stringify({ name: "app", dependencies: { lib: "*", vue: "*" } });

const loaded = (files: Record<string, string>, page = "app/page.ts") => {
  const graph = buildGraph(sources(files), ["lib", "app"]);
  return closure(graph, [{ file: page, demand: { kind: "all" } }]);
};

describe("import graph", () => {
  it("follows a named import through a barrel to the module that defines it", () => {
    const c = loaded({
      "lib/package.json": lib,
      "lib/index.ts": 'export { default as A } from "./A.vue";\nexport { b } from "./b.ts";\n',
      "lib/A.vue":
        '<script setup lang="ts">\nimport { ref } from "vue";\nimport "./a.css";\n</script>\n',
      "lib/a.css": ".k-a { color: red; }\n",
      "lib/b.ts": "export const b = 1;\n",
      "app/package.json": app,
      "app/page.ts": 'import { A } from "lib";\nexport default A;\n',
    });
    expect([...c.files].sort()).toEqual(["app/page.ts", "lib/A.vue", "lib/a.css", "lib/index.ts"]);
    expect(c.doubts).toEqual([]);
  });

  it("resolves a name through `export *`, the other branches left out", () => {
    const c = loaded({
      "lib/package.json": lib,
      "lib/index.ts": 'export * from "./a.ts";\nexport * from "./b.ts";\n',
      "lib/a.ts": "export const a = 1;\n",
      "lib/b.ts": "export function b() {}\n",
      "app/package.json": app,
      "app/page.ts": 'import { b } from "lib";\nb();\n',
    });
    expect([...c.files].sort()).toEqual(["app/page.ts", "lib/b.ts", "lib/index.ts"]);
  });

  it("skips type-only imports, keeps one whose bindings are all types as an evaluation", () => {
    const c = loaded({
      "lib/package.json": lib,
      "lib/index.ts": 'export { T } from "./t.ts";\nexport { u } from "./u.ts";\n',
      "lib/t.ts": "export type T = number;\n",
      "lib/u.ts": "export const u = 1;\n",
      "app/package.json": app,
      "app/types.ts": "export type X = string;\n",
      "app/side.ts": "export type Y = string;\nconsole.log('side effect');\n",
      "app/page.ts":
        'import type { T } from "lib";\nimport type { X } from "./types.ts";\n' +
        'import { type Y } from "./side.ts";\nexport const v: T | X | Y = 1;\n',
    });
    // `import { type Y }` stays `import "./side.ts"` (verbatimModuleSyntax): side.ts runs.
    expect([...c.files].sort()).toEqual(["app/page.ts", "app/side.ts"]);
  });

  it("evaluates a module free of side effects for nothing, another for its body", () => {
    const files = {
      "lib/package.json": lib,
      "lib/index.ts": 'import "./base.css";\nexport { a } from "./a.ts";\n',
      "lib/base.css": ":root { --x: 1; }\n",
      "lib/a.ts": "export const a = 1;\n",
      "app/package.json": app,
      "app/shared.ts": 'export * from "lib";\n',
      "app/page.ts": 'import "./shared.ts";\n',
    };
    // The app evaluates shared.ts, which re-exports lib: lib's modules are free of side
    // effects (sideEffects), so nothing of lib is loaded but where a binding is used.
    expect([...loaded(files).files].sort()).toEqual(["app/page.ts", "app/shared.ts"]);
  });

  it("follows dynamic imports, globs and sheets; a cut edge stops there", () => {
    const files = {
      "app/package.json": app,
      "app/page.ts":
        'const lazy = () => import("./lazy.ts");\nconst all = import.meta.glob("./parts/*.ts");\n',
      "app/lazy.ts": 'import "./lazy.css";\nexport default 1;\n',
      "app/lazy.css": '@import "./more.css";\n.a { background: url("./img.svg"); }\n',
      "app/more.css": ".b {}\n",
      "app/img.svg": "<svg/>",
      "app/parts/one.ts": "export default 1;\n",
      "app/parts/two.ts": "export default 2;\n",
    };
    const graph = buildGraph(sources(files), ["app"]);
    const root = [{ file: "app/page.ts", demand: { kind: "all" } as const }];
    expect([...closure(graph, root).files].sort()).toEqual([
      "app/img.svg",
      "app/lazy.css",
      "app/lazy.ts",
      "app/more.css",
      "app/page.ts",
      "app/parts/one.ts",
      "app/parts/two.ts",
    ]);
    const cut = closure(graph, root, (_, dep) => dep.how === "glob");
    expect([...cut.files]).not.toContain("app/parts/one.ts");
  });

  it("widens on what it cannot resolve: a computed import, an unknown alias, a missing file", () => {
    const c = loaded({
      "app/package.json": app,
      "app/page.ts":
        'import { x } from "@/x";\nimport "./gone.ts";\n' +
        "const name = 'a';\nexport const m = () => import(`./${name}.ts`);\n",
    });
    expect(c.doubts).toEqual([
      'app/page.ts: unknown package or alias "@/x"',
      'app/page.ts: unresolved "./gone.ts"',
      "app/page.ts: computed import(`./${name}.ts`)",
    ]);
  });

  it("takes a whole module for a name it does not find", () => {
    const c = loaded({
      "lib/package.json": lib,
      "lib/index.ts": 'export { a } from "./a.ts";\nexport { b } from "./b.ts";\n',
      "lib/a.ts": "export const a = 1;\n",
      "lib/b.ts": "export const b = 1;\n",
      "app/package.json": app,
      "app/page.ts": 'import { missing } from "lib";\n',
    });
    expect(c.doubts).toEqual(['lib/index.ts: no export "missing"']);
    expect([...c.files].sort()).toEqual(["app/page.ts", "lib/a.ts", "lib/b.ts", "lib/index.ts"]);
    // The same through `export *`: no branch has it, every branch is taken.
    const star = loaded({
      "lib/package.json": lib,
      "lib/index.ts": 'export * from "./a.ts";\nexport * from "./b.ts";\n',
      "lib/a.ts": "export const a = 1;\n",
      "lib/b.ts": "export const b = 1;\n",
      "app/package.json": app,
      "app/page.ts": 'import { missing } from "lib";\n',
    });
    expect(star.doubts).toEqual(['lib/index.ts: no export "missing"']);
    expect([...star.files].sort()).toEqual(["app/page.ts", "lib/a.ts", "lib/b.ts", "lib/index.ts"]);
  });
});

describe("pages a spec opens", () => {
  const components = ["Button", "DataGrid", "Menu"];
  const mockups = ["E1", "E10"];
  const pages = (text: string) => [...specPages(text, components, mockups)].sort();

  it("reads literal addresses, names given to helpers and lists looped over", () => {
    expect(
      pages(
        'await openState(page, "Button", "rest");\nawait page.goto("/_ui/ecrans/E10?vue=x");\n' +
          'for (const c of ["DataGrid"]) await open(page, `/_ui/c/${c}`);\n' +
          'for (const url of ["/_ui/jetons?mode=dark"]) await open(page, url);\n',
      ),
    ).toEqual(["catalogue:Button", "catalogue:DataGrid", "catalogue:jetons", "ecran:E10"]);
  });

  it("reads MOCKUPS as every mock-up, the components' directory as every component", () => {
    expect(
      pages('import { MOCKUPS } from "./registry.ts";\nawait page.goto("/_ui/c/Menu");\n'),
    ).toEqual(["catalogue:Menu", "ecran:E1", "ecran:E10"]);
    expect(pages('readdirSync(new URL("../../ui/src/components", import.meta.url));\n')).toContain(
      "catalogue:Button",
    );
  });

  it("widens to every page on an address it cannot read", () => {
    const every = [
      "catalogue:Button",
      "catalogue:DataGrid",
      "catalogue:Menu",
      "catalogue:icones",
      "catalogue:index",
      "catalogue:jetons",
      "ecran:E1",
      "ecran:E10",
      "revue",
    ];
    expect(
      pages('await page.goto("/_ui/c/Menu");\nawait open(page, `${path}${query}`);\n'),
    ).toEqual(every);
    expect(pages("async function go(page, url) { await open(page, url); }\n")).toEqual(every);
    expect(pages("// opens nothing it names\n")).toEqual(every);
  });
});

describe("impact on the workspace", () => {
  let project: Project;
  const v2 = (file: string) => `${project.prefix}/${file}`;
  const run = (...files: string[]) => impact(files.map(v2), project);
  const names = (...files: string[]) =>
    run(...files).targets.map((target) => `${target.kind}:${target.name}`);

  beforeAll(async () => {
    project = await loadProject();
  });

  it("reads every page without doubt", () => {
    expect(project.prefix).toBe("web/v2");
    for (const page of project.pages) expect(page.closure.doubts, page.key).toEqual([]);
  });

  it("gives every component of the catalogue its page", () => {
    const declared = readdirSync(join(WORKSPACE, "ui/src/components"), { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
      .filter((name) =>
        existsSync(join(WORKSPACE, `ui/src/components/${name}/${name}.catalogue.ts`)),
      )
      .sort();
    expect(project.components).toEqual(declared);
    for (const name of declared) {
      const own = [`ui/src/components/${name}/${name}.catalogue.ts`];
      if (existsSync(join(WORKSPACE, `ui/src/components/${name}/${name}.vue`)))
        own.push(`ui/src/components/${name}/${name}.vue`);
      for (const file of own) {
        const result = run(file);
        expect(result.all, file).toBe(false);
        expect(names(file), file).toContain(`catalogue:${name}`);
      }
    }
  });

  it("gives every mock-up its captures, from its screen alone", () => {
    const screenOf = screens(
      readFileSync(join(WORKSPACE, "shell/src/_ui/ecrans/EcranPage.vue"), "utf8"),
    );
    expect(project.mockups.length).toBeGreaterThan(0);
    for (const id of project.mockups) {
      const screen = screenOf.get(id);
      expect(screen, id).toBeDefined();
      const reached = names(screen!).filter((name) => name.startsWith("ecran:"));
      expect(reached, id).toEqual([`ecran:${id}`]);
    }
  });

  it("reaches a mock-up from each component of @kartenn/ui it imports", () => {
    const screenOf = screens(
      readFileSync(join(WORKSPACE, "shell/src/_ui/ecrans/EcranPage.vue"), "utf8"),
    );
    let checked = 0;
    for (const id of project.mockups) {
      const source = readFileSync(join(WORKSPACE, screenOf.get(id)!), "utf8");
      const imported = [...source.matchAll(/import\s*\{([^}]*)\}\s*from\s*"@kartenn\/ui"/g)]
        .flatMap((m) => m[1]!.split(","))
        .map((name) => name.trim())
        .filter((name) => /^[A-Z]\w*$/.test(name));
      for (const name of imported) {
        const file = `ui/src/components/${name}/${name}.vue`;
        if (!existsSync(join(WORKSPACE, file))) continue;
        expect(names(file), `${name} → ${id}`).toContain(`ecran:${id}`);
        checked++;
      }
    }
    expect(checked).toBeGreaterThan(50);
  });

  it("reaches through the entry of @kartenn/ui only the real importers of a symbol", () => {
    const e3 = readFileSync(join(WORKSPACE, "shell/src/_ui/ecrans/E3Adresses.vue"), "utf8");
    expect(e3).toContain('from "@kartenn/ui"');
    expect(e3).not.toMatch(/\bMatrixGrid\b/);
    const reached = names("ui/src/components/MatrixGrid/MatrixGrid.vue");
    expect(reached).toContain("catalogue:MatrixGrid");
    expect(reached).toContain("ecran:E10");
    expect(reached).not.toContain("ecran:E3");
    expect(reached).not.toContain("catalogue:Button");
    // Tree: its page and its script, no mock-up (they use TreeGrid).
    expect(names("ui/src/components/Tree/Tree.vue").filter((n) => n.startsWith("ecran:"))).toEqual(
      [],
    );
  });

  it("reaches the keyboard script of a component, and the transverse specs that open it", () => {
    for (const file of readdirSync(join(WORKSPACE, "e2e/components")).filter((f) =>
      f.endsWith(".spec.ts"),
    )) {
      const name = file
        .replace(/\.spec\.ts$/, "")
        .replace(/(^|-)([a-z])/g, (_, __, c: string) => c.toUpperCase());
      const component = project.components.includes(name) ? name : "EnvChip";
      expect(names(`ui/src/components/${component}/${component}.vue`), file).toContain(
        `spec:components/${file}`,
      );
    }
    const reached = names("ui/src/components/SidePanel/SidePanel.vue");
    for (const spec of ["clavier", "layout", "touch", "csp"])
      expect(reached).toContain(`spec:visual/${spec}.spec.ts`);
    // A component no transverse spec names, in no mock-up: csp alone (it opens every page).
    const tree = names("ui/src/components/Tree/Tree.vue");
    expect(tree).toContain("spec:visual/csp.spec.ts");
    expect(tree).not.toContain("spec:visual/clavier.spec.ts");
  });

  it("reaches everything from a transverse file", () => {
    for (const file of [
      "ui/src/tokens/tokens.css",
      "ui/src/tokens/index.ts",
      "ui/src/style.css",
      "ui/src/styles/layers.css",
      "ui/src/components/shared.css",
      "ui/src/fonts/geist-latin-wght-normal.woff2",
      "e2e/fixtures/index.ts",
      "e2e/playwright.config.ts",
      "package.json",
      "ui/package.json",
      "pnpm-lock.yaml",
      "shell/vite.config.ts",
      "shell/index.html",
      "tsconfig.base.json",
      "tools/impact/graph.ts",
    ])
      expect(run(file).all, file).toBe(true);
    expect(impact(["devenv.nix"], project).all).toBe(true);
    // A file no module points to may be read by what the graph cannot see.
    expect(run("ui/src/components/Button/unknown.svg").all).toBe(true);
    // What every page loads (the entry of the shell, the strings of @kartenn/ui): every page.
    for (const file of ["shell/src/main.ts", "ui/src/i18n.ts"]) {
      const reached = names(file);
      expect(reached.filter((name) => !name.startsWith("spec:"))).toHaveLength(
        CATALOGUE_PAGES.length + project.components.length + project.mockups.length,
      );
    }
  });

  it("reaches nothing from a file no capture reads", () => {
    for (const file of [
      "ui/test/package.test.ts",
      "ui/src/components/Button/button.test.ts",
      "e2e/journeys/shell.spec.ts",
      "e2e/journeys.config.ts",
      "shell/scripts/size.ts",
      "eslint.config.js",
      "tools/test/eslint.test.ts",
      "shell/src/views/ListView.vue",
    ]) {
      const result = run(file);
      expect(result, file).toEqual({ all: false, reasons: [], targets: [] });
    }
    expect(
      impact(
        ["crates/kartenn-rest/src/web.rs", "docs/adr/0042-verifications-web-ciblees.md"],
        project,
      ).targets,
    ).toEqual([]);
  });

  it("widens a page whose imports are not all read to any change", () => {
    const pages = project.pages.map((page) =>
      page.key === "ecran:E3"
        ? { ...page, closure: { ...page.closure, doubts: ["E3Adresses.vue: computed import(x)"] } }
        : page,
    );
    const result = impact([v2("shell/src/views/ListView.vue")], { ...project, pages });
    expect(result.targets.map((t) => `${t.kind}:${t.name}`)).toContain("ecran:E3");
    expect(result.reasons).toEqual(["ecran:E3 widened: E3Adresses.vue: computed import(x)"]);
    expect(impact([v2("ui/README.md")], { ...project, pages }).targets).toEqual([]);
  });

  it("reaches a spec from itself, the page of a reference from its name", () => {
    expect(names("e2e/components/button.spec.ts")).toEqual(["spec:components/button.spec.ts"]);
    expect(names(`e2e/${CATALOGUE_SPEC}`)).toEqual([`spec:${CATALOGUE_SPEC}`]);
    expect(names("e2e/visual/__screenshots__/catalogue.spec.ts/TreeGrid-rest-dark.png")).toEqual([
      "catalogue:TreeGrid",
    ]);
    expect(names("e2e/visual/__screenshots__/ecrans.spec.ts/E10-matrice-1280-dark.png")).toEqual([
      "ecran:E10",
    ]);
    expect(names("e2e/visual/__screenshots__/catalogue.spec.ts/jetons-light-compacte.png")).toEqual(
      ["catalogue:jetons"],
    );
    expect(names("e2e/components/__screenshots__/forced-colors.spec.ts/skeleton.png")).toEqual([
      "spec:components/forced-colors.spec.ts",
    ]);
    // A new catalogue file is listed by the index.
    expect(names("ui/src/components/Nouveau/Nouveau.catalogue.ts")).toContain("catalogue:index");
  });
});

describe("selection by Playwright", () => {
  let project: Project;
  /** Each test of the project `visual`: its file and the title Playwright greps. */
  let tests: { file: string; title: string }[];

  const list = (...args: string[]) => {
    const out = execFileSync(
      process.execPath,
      [
        join(WORKSPACE, "e2e/node_modules/@playwright/test/cli.js"),
        "test",
        "--list",
        "--project",
        "visual",
        "--reporter=json",
        ...args,
      ],
      {
        cwd: join(WORKSPACE, "e2e"),
        encoding: "utf8",
        maxBuffer: 64 * 1024 * 1024,
        env: {
          ...process.env,
          PLAYWRIGHT_KARTENN_FONTCONFIG: process.env.PLAYWRIGHT_KARTENN_FONTCONFIG ?? "unset",
        },
      },
    );
    interface Suite {
      title: string;
      suites?: Suite[];
      specs?: { title: string }[];
    }
    const found: { file: string; title: string }[] = [];
    const walk = (suite: Suite, path: string[], file: string) => {
      for (const spec of suite.specs ?? [])
        found.push({ file, title: ["", "visual", ...path, spec.title].join(" ") });
      for (const child of suite.suites ?? []) walk(child, [...path, child.title], file);
    };
    for (const suite of (JSON.parse(out) as { suites: Suite[] }).suites)
      walk(suite, [suite.title], suite.title);
    return found;
  };

  beforeAll(async () => {
    project = await loadProject();
    tests = list();
  }, 120_000);

  it("selects each test of the catalogue and the mock-ups by exactly one page", () => {
    // The entry of the shell is loaded by every page: all of them, page by page.
    const { targets } = impact([`${project.prefix}/shell/src/main.ts`], project);
    const pages = targets.filter((t) => t.kind !== "spec");
    expect(pages.length).toBe(
      CATALOGUE_PAGES.length + project.components.length + project.mockups.length,
    );
    for (const file of [CATALOGUE_SPEC, ECRANS_SPEC]) {
      const own = tests.filter((test) => test.file === file);
      expect(own.length, file).toBeGreaterThan(0);
      for (const test of own) {
        const by = pages.filter((t) => new RegExp(t.grep).test(test.title)).map((t) => t.name);
        expect(by, test.title).toHaveLength(1);
      }
    }
    for (const target of targets)
      expect(
        tests.some((test) => new RegExp(target.grep).test(test.title)),
        target.grep,
      ).toBe(true);
  });

  it("builds the titles as Playwright greps them", () => {
    const greps = [
      ...impact([`${project.prefix}/ui/src/components/MatrixGrid/MatrixGrid.vue`], project).targets,
    ].map((target) => target.grep);
    const combined = greps.join("|");
    const ours = tests.filter((test) => new RegExp(combined).test(test.title)).map((t) => t.title);
    const theirs = list("--grep", combined).map((t) => t.title);
    expect(theirs.length).toBeGreaterThan(0);
    expect(theirs.sort()).toEqual(ours.sort());
  });
});
