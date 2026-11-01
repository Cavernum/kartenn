import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { isAbsolute, join, relative } from "node:path";
import { formatLines, impact, loadProject, WORKSPACE } from "./impact.ts";

// `node web/v2/tools/impact/cli.ts [--json] [file…]`: the captures a change reaches (spec
// 029, task 2). Files as arguments, else one per line on the standard input: paths of the
// repository (`git diff --name-only`), read from its root whatever the current directory,
// or absolute. Prints one target per line, `<spec>\t<grep>` (spec relative to e2e/, a
// regular expression for Playwright's `--grep`), or `all`; `--json` prints the whole
// result. The doubts that widened the selection, the deleted specs, and the paths that
// neither exist nor were deleted, go to the standard error (`impact: …`); the reason of
// `all` as `impact: all: …`.

const git = (...args: string[]) =>
  execFileSync("git", args, { cwd: WORKSPACE, encoding: "utf8" }).trim();

const args = process.argv.slice(2);
const json = args.includes("--json");
const named = args.filter((arg) => arg !== "--json");
const given =
  named.length > 0
    ? named
    : readFileSync(0, "utf8")
        .split("\n")
        .map((line) => line.trim())
        .filter((line) => line.length > 0);

const repository = git("rev-parse", "--show-toplevel");
const project = await loadProject();
const changed = given.map((file) => (isAbsolute(file) ? relative(repository, file) : file));
for (const path of changed) {
  if (existsSync(join(repository, path))) continue;
  // Deleted: from the working tree (still in the index) or by a commit.
  const known =
    git("ls-files", "--", path) !== "" ||
    git("log", "--all", "--diff-filter=D", "--format=%h", "-1", "--", path) !== "";
  if (!known) process.stderr.write(`impact: ${path}: no such file, nor a deleted one\n`);
}
const result = impact(changed, project);
// The reason of « everything » under its own tag, apart from the other notices.
const tag = result.all ? "impact: all: " : "impact: ";
for (const reason of result.reasons) process.stderr.write(`${tag}${reason}\n`);
process.stdout.write(json ? `${JSON.stringify(result, null, 2)}\n` : formatLines(result));
