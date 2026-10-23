// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readdirSync, readFileSync } from "node:fs";
import { extname, join } from "node:path";

/** Exhibit A of the MPL 2.0, as in `crates/syslog-sender/src/lib.rs` (REQ-007-38). */
export const MPL_NOTICE =
  "This Source Code Form is subject to the terms of the Mozilla Public " +
  "License, v. 2.0. If a copy of the MPL was not distributed with this " +
  "file, You can obtain one at https://mozilla.org/MPL/2.0/.";

/** Source files that must carry the notice. */
export const SOURCE_EXTENSIONS = new Set([".ts", ".js", ".mjs", ".vue", ".css"]);

const SKIPPED_DIRS = new Set(["node_modules", "dist"]);

/** Whether the head of a file carries the notice, whatever its comment syntax. */
export function hasMplNotice(source: string): boolean {
  const head = source
    .split("\n")
    .slice(0, 8)
    .map((line) =>
      line
        .replace(/^\s*(\/\/|\/\*+|\*\/|\*|<!--|-->)\s?/, "")
        .replace(/\s*(\*\/|-->)\s*$/, "")
        .trim(),
    )
    .filter((line) => line.length > 0)
    .join(" ");
  return head.includes(MPL_NOTICE);
}

/** Source files under `root`, recursively, outside dependencies and builds. */
export function sourceFiles(root: string): string[] {
  const found: string[] = [];
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    if (SKIPPED_DIRS.has(entry.name)) continue;
    const path = join(root, entry.name);
    if (entry.isDirectory()) found.push(...sourceFiles(path));
    else if (SOURCE_EXTENSIONS.has(extname(entry.name))) found.push(path);
  }
  return found;
}

/** Files under `roots` whose head lacks the notice. */
export function filesWithoutNotice(roots: string[]): string[] {
  return roots
    .flatMap(sourceFiles)
    .filter((path) => !hasMplNotice(readFileSync(path, "utf8")))
    .sort();
}
