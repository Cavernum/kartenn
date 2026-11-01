import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// `node web/v2/tools/catalogue/build.ts`: the test build of the shell and its catalogue
// (`pnpm -C shell run build:catalogue`, `dist/catalogue`), done again only when its sources
// changed (spec 029, task 1; ADR 0042, decision 4). The webServer of the captures
// (e2e/playwright.config.ts) runs it before `vite preview`.
//
// The fingerprint covers what the build may read, widely: the packages it bundles (shell,
// ui, sdk, the plugins), the manifests, the lockfile and the base configuration of the
// workspace, files tracked and untracked alike (not the ignored ones: node_modules, dist),
// and the version of Node. The stamp beside the output (`dist/catalogue.sources`) also
// holds the digest of the built `index.html`, whose import map carries the digest of every
// module: a build made by hand since, from other sources, is not taken for this one.

export const WORKSPACE = fileURLToPath(new URL("../..", import.meta.url));
export const SOURCES = [
  "shell",
  "ui",
  "sdk",
  "plugins",
  "package.json",
  "pnpm-lock.yaml",
  "pnpm-workspace.yaml",
  "tsconfig.base.json",
  ".npmrc",
];
const OUT = join(WORKSPACE, "dist/catalogue");
const STAMP = join(WORKSPACE, "dist/catalogue.sources");

const sha256 = (body: Buffer | string) => createHash("sha256").update(body).digest("hex");

/** The files of `SOURCES` under `root`, tracked or untracked, without the ignored ones. */
export function sourceFiles(root: string): string[] {
  const listed = execFileSync(
    "git",
    [
      "-c",
      "core.quotePath=false",
      "ls-files",
      "-z",
      "--cached",
      "--others",
      "--exclude-standard",
      "--",
      ...SOURCES,
    ],
    { cwd: root, encoding: "utf8" },
  );
  return [...new Set(listed.split("\0").filter((file) => file.length > 0))].sort();
}

/** The fingerprint of `files` under `root`: paths and contents, a deleted file as such. */
export function fingerprint(root: string, files: string[], node = process.version): string {
  const hash = createHash("sha256").update(`node ${node}\0`);
  for (const file of [...files].sort()) {
    const path = join(root, file);
    hash.update(`${file}\0${existsSync(path) ? sha256(readFileSync(path)) : "deleted"}\0`);
  }
  return hash.digest("hex");
}

interface Stamp {
  sources: string;
  index: string;
}

function readStamp(): Stamp | undefined {
  try {
    return JSON.parse(readFileSync(STAMP, "utf8")) as Stamp;
  } catch {
    return undefined;
  }
}

function main() {
  const sources = fingerprint(WORKSPACE, sourceFiles(WORKSPACE));
  const index = join(OUT, "index.html");
  const stamp = readStamp();
  if (
    stamp?.sources === sources &&
    existsSync(index) &&
    stamp.index === sha256(readFileSync(index))
  ) {
    process.stderr.write("catalogue: sources unchanged, build reused\n");
    return;
  }
  rmSync(STAMP, { force: true });
  const build = spawnSync("pnpm", ["run", "build:catalogue"], {
    cwd: join(WORKSPACE, "shell"),
    stdio: "inherit",
  });
  if (build.status !== 0) process.exit(build.status ?? 1);
  writeFileSync(STAMP, `${JSON.stringify({ sources, index: sha256(readFileSync(index)) })}\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main();
