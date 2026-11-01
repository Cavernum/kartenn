import { readFileSync } from "node:fs";
import { relative, resolve } from "node:path";
import { formatLines, impact, loadProject } from "./impact.ts";

// `node tools/impact/cli.ts [--json] [file…]`: the captures a change reaches (spec 029,
// task 2). Files as arguments, else one per line on the standard input, relative to the
// current directory or absolute (`git diff --name-only` from the repository's root).
// Prints one target per line, `<spec>\t<grep>` (spec relative to e2e/, a regular
// expression for Playwright's `--grep`), or `all`; `--json` prints the whole result. The
// doubts that widened the selection go to the standard error.

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

const project = await loadProject();
const changed = given.map((file) => relative(project.repository, resolve(file)));
const result = impact(changed, project);
for (const reason of result.reasons) process.stderr.write(`impact: ${reason}\n`);
process.stdout.write(json ? `${JSON.stringify(result, null, 2)}\n` : formatLines(result));
