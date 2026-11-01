import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { fingerprint, sourceFiles, WORKSPACE } from "../catalogue/build.ts";

// The build of the catalogue reused while its sources stay the same (spec 029, task 1;
// ADR 0042, decision 4): the fingerprint follows every file the build may read, and only
// those.

const roots: string[] = [];
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

/** A git repository holding `files` (path → text), the given ones committed. */
function repository(files: Record<string, string>, committed: string[] = []): string {
  const root = mkdtempSync(join(tmpdir(), "kartenn-catalogue-"));
  roots.push(root);
  for (const [file, text] of Object.entries(files)) {
    mkdirSync(dirname(join(root, file)), { recursive: true });
    writeFileSync(join(root, file), text);
  }
  const git = (...args: string[]) => execFileSync("git", args, { cwd: root, stdio: "pipe" });
  git("init", "-q");
  if (committed.length > 0) {
    git("add", "--", ...committed);
    git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "t");
  }
  return root;
}

describe("sources of the build", () => {
  it("lists the packages and manifests, tracked or not, without ignored files nor e2e", () => {
    const root = repository(
      {
        ".gitignore": "dist/\nnode_modules/\n",
        "package.json": "{}",
        "pnpm-lock.yaml": "",
        "shell/vite.config.ts": "",
        "shell/src/new.ts": "",
        "ui/src/index.ts": "",
        "ui/node_modules/x/index.js": "",
        "dist/catalogue/index.html": "",
        "e2e/visual/catalogue.spec.ts": "",
        "tools/impact/cli.ts": "",
      },
      [".gitignore", "package.json", "pnpm-lock.yaml", "shell/vite.config.ts", "ui/src/index.ts"],
    );
    expect(sourceFiles(root)).toEqual([
      "package.json",
      "pnpm-lock.yaml",
      "shell/src/new.ts",
      "shell/vite.config.ts",
      "ui/src/index.ts",
    ]);
  });

  it("covers the shell, @kartenn/ui and the lockfile of the workspace", () => {
    const files = sourceFiles(WORKSPACE);
    expect(files).toEqual(
      expect.arrayContaining(["shell/vite.config.ts", "ui/src/index.ts", "pnpm-lock.yaml"]),
    );
    expect(files.filter((file) => /^(e2e|tools|dist)\/|node_modules\//.test(file))).toEqual([]);
  });
});

describe("fingerprint", () => {
  const files = ["shell/a.ts", "ui/b.vue"];
  const root = () => repository({ "shell/a.ts": "a", "ui/b.vue": "b" });

  it("stays the same for the same sources, in any order", () => {
    const at = root();
    expect(fingerprint(at, files)).toBe(fingerprint(at, [...files].reverse()));
  });

  it("changes with a content, a deleted file, an added one or the version of Node", () => {
    const at = root();
    const before = fingerprint(at, files);
    writeFileSync(join(at, "ui/b.vue"), "b2");
    const edited = fingerprint(at, files);
    expect(edited).not.toBe(before);
    rmSync(join(at, "ui/b.vue"));
    expect(fingerprint(at, files)).not.toBe(edited);
    writeFileSync(join(at, "ui/b.vue"), "b");
    expect(fingerprint(at, files)).toBe(before);
    writeFileSync(join(at, "shell/c.ts"), "");
    expect(fingerprint(at, [...files, "shell/c.ts"])).not.toBe(before);
    expect(fingerprint(at, files, "v0.0.0")).not.toBe(before);
  });
});
