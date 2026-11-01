import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { beforeAll, describe, expect, it } from "vitest";
import { buildGraph, closure, type Package, type Sources } from "../impact/graph.ts";
import { PREFIX_EXCEPTIONS } from "../stylelint/prefixes.js";
import {
  CATALOGUE_PAGES,
  CATALOGUE_SPEC,
  ECRANS_SPEC,
  WORKSPACE,
  catalogueFiles,
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

  it("reads literal addresses, names given to helpers and lists of literals looped over", () => {
    expect(
      pages(
        'await openState(page, "Button", "rest");\nawait page.goto("/_ui/ecrans/E10?vue=x");\n' +
          'await open(page, "/_ui/c/DataGrid");\n' +
          'for (const url of ["/_ui/jetons?mode=dark"]) await open(page, url);\n',
      ),
    ).toEqual(["catalogue:Button", "catalogue:DataGrid", "catalogue:jetons", "ecran:E10"]);
  });

  it("reads a computed address as any page of its kind, whatever literals stand beside it", () => {
    expect(pages('for (const c of ["DataGrid"]) await open(page, `/_ui/c/${c}`);\n')).toEqual([
      "catalogue:Button",
      "catalogue:DataGrid",
      "catalogue:Menu",
    ]);
    expect(
      pages(
        'const state = (page, name) => page.goto(`/_ui/c/${name}?etat=x`);\nawait state(page, "Menu");\n',
      ),
    ).toEqual(["catalogue:Button", "catalogue:DataGrid", "catalogue:Menu"]);
    expect(pages('const id = pick("E1");\nconst url = `/_ui/ecrans/${id}`;\n')).toEqual([
      "ecran:E1",
      "ecran:E10",
    ]);
    // The view of a named mock-up computed: that mock-up alone.
    expect(pages("await page.goto(`/_ui/ecrans/E10?vue=${view}`);\n")).toEqual(["ecran:E10"]);
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
        // A sheet left out of kartenn/component-prefix: everything.
        if (PREFIX_EXCEPTIONS.includes(file)) {
          expect(result.all, file).toBe(true);
          continue;
        }
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
    for (const id of project.mockups) {
      const source = readFileSync(join(WORKSPACE, screenOf.get(id)!), "utf8");
      const imported = [...source.matchAll(/import\s*\{([^}]*)\}\s*from\s*"@kartenn\/ui"/g)]
        .flatMap((m) => m[1]!.split(","))
        .map((name) => name.trim())
        .filter((name) => /^[A-Z]\w*$/.test(name));
      for (const name of imported) {
        const file = `ui/src/components/${name}/${name}.vue`;
        if (existsSync(join(WORKSPACE, file)) && !PREFIX_EXCEPTIONS.includes(file))
          expect(names(file), `${name} → ${id}`).toContain(`ecran:${id}`);
      }
    }
  });

  /** The single-file component of each component of its name, its sheet's exceptions aside. */
  const componentFiles = () =>
    project.components
      .map((name) => `ui/src/components/${name}/${name}.vue`)
      .filter((file) => existsSync(join(WORKSPACE, file)) && !PREFIX_EXCEPTIONS.includes(file));

  it("reaches through the entry of @kartenn/ui only the real importers of a symbol", () => {
    // A page is reached by a component exactly when its closure loads the component.
    let pruned = 0;
    for (const file of componentFiles()) {
      const reached = new Set(names(file));
      for (const page of project.pages) {
        if (page.key === "revue") continue;
        expect(reached.has(page.key), `${file} → ${page.key}`).toBe(page.closure.files.has(file));
        // Pages that import from the entry without loading this component: pruned by symbol.
        if (page.closure.files.has("ui/src/index.ts") && !page.closure.files.has(file)) pruned++;
      }
    }
    expect(pruned).toBeGreaterThan(0);
  });

  it("reaches each spec from the components of the pages it opens", () => {
    for (const file of componentFiles()) {
      const reached = new Set(names(file));
      for (const [spec, pages] of project.specs) {
        const opens = [...pages].some((key) =>
          project.pages.find((page) => page.key === key)?.closure.files.has(file),
        );
        expect(reached.has(`spec:${spec}`), `${file} → ${spec}`).toBe(opens);
      }
    }
    // Every keyboard script opens a page of the catalogue, hence is reached by its component.
    for (const [spec, pages] of project.specs)
      if (spec.startsWith("components/"))
        expect(
          [...pages].some((key) => key.startsWith("catalogue:")),
          spec,
        ).toBe(true);
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
      "tools/stylelint/prefixes.js",
      // Files of configuration, read by their name.
      "eslint.config.js",
      "stylelint.config.js",
      "vitest.config.ts",
      "e2e/journeys.config.ts",
      "postcss.config.js",
      // The sheets left out of kartenn/component-prefix.
      ...PREFIX_EXCEPTIONS,
    ])
      expect(run(file).all, file).toBe(true);
    expect(impact(["devenv.nix"], project).all).toBe(true);
    // A file no module points to may be read by what the graph cannot see.
    expect(run("ui/src/components/Button/unknown.svg").all).toBe(true);
    // What every page loads (the strings of @kartenn/ui): every page, one by one.
    expect(names("ui/src/i18n.ts").filter((name) => !name.startsWith("spec:"))).toHaveLength(
      CATALOGUE_PAGES.length + project.components.length + project.mockups.length,
    );
  });

  it("reaches everything from what the entry of the shell loads statically", () => {
    // Its modules run and its sheets join the entry's on every page, route components
    // imported statically included (ShellFrame.vue, from router.ts).
    expect(project.entry.has("shell/src/frame/ShellFrame.vue")).toBe(true);
    for (const file of project.entry) expect(run(file).all, file).toBe(true);
    for (const file of [
      "shell/src/main.ts",
      "shell/src/frame/ShellFrame.vue",
      "shell/src/router.ts",
    ])
      expect(project.entry.has(file), file).toBe(true);
    // Not a lazy route component, nor a component of @kartenn/ui the frame renders (its JS
    // is free of side effects, its sheet kept to it by kartenn/component-prefix).
    expect(project.entry.has("shell/src/views/ListView.vue")).toBe(false);
    expect(run("ui/src/components/SidebarNav/SidebarNav.vue").all).toBe(false);
  });

  it("reaches nothing from a file no capture reads", () => {
    for (const file of [
      "ui/test/package.test.ts",
      "ui/src/components/Button/button.test.ts",
      "e2e/journeys/shell.spec.ts",
      "shell/scripts/size.ts",
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
    // A new catalogue file is listed by the index, wherever it lies under components/*/.
    expect(names("ui/src/components/Nouveau/Nouveau.catalogue.ts")).toContain("catalogue:index");
    expect(names("ui/src/components/X/Y.catalogue.ts")).toContain("catalogue:index");
  });

  it("reads the catalogue files as catalogue.ts globs them", () => {
    const glob = /import\.meta\.glob<[^>]*>\("([^"]+)"\)/.exec(
      readFileSync(join(WORKSPACE, "ui/src/catalogue.ts"), "utf8"),
    )?.[1];
    expect(glob).toBe("./components/*/*.catalogue.ts");
    expect(
      catalogueFiles([
        "ui/src/components/X/Y.catalogue.ts",
        "ui/src/components/Z/Z.catalogue.ts",
        "ui/src/components/Z/deep/W.catalogue.ts",
        "ui/src/Q.catalogue.ts",
      ]),
    ).toEqual(
      new Map([
        ["Y", ["ui/src/components/X/Y.catalogue.ts"]],
        ["Z", ["ui/src/components/Z/Z.catalogue.ts"]],
      ]),
    );
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
    // The strings of @kartenn/ui are loaded by every page: all of them, page by page.
    const { targets } = impact([`${project.prefix}/ui/src/i18n.ts`], project);
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
    const { targets } = impact([`${project.prefix}/ui/src/i18n.ts`], project);
    const transverse = [...project.specs.keys()].filter((spec) => spec.startsWith("visual/"));
    const script = [...project.specs.keys()].find((spec) => spec.startsWith("components/"))!;
    expect(transverse.length).toBeGreaterThan(0);
    // Each transverse spec, a keyboard script, a page of the catalogue and a mock-up, alone
    // then together: the same tests on both sides.
    const chosen = [
      ...[...transverse, script].map((spec) => targets.find((t) => t.spec === spec)!),
      targets.find((t) => t.kind === "catalogue" && !CATALOGUE_PAGES.includes(t.name))!,
      targets.find((t) => t.kind === "ecran")!,
    ];
    const ours = (grep: string) =>
      tests.filter((test) => new RegExp(grep).test(test.title)).map((t) => t.title);
    const theirs = (grep: string) => list("--grep", grep).map((t) => t.title);
    for (const target of chosen) {
      const mine = ours(target.grep);
      expect(mine.length, target.grep).toBeGreaterThan(0);
      expect(
        mine.every((title) => title.includes(` ${target.spec} `)),
        target.grep,
      ).toBe(true);
    }
    const combined = chosen.map((target) => target.grep).join("|");
    expect(theirs(combined).sort()).toEqual(ours(combined).sort());
  });
});
