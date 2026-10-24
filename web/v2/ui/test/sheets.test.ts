// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { tokens } from "../src/tokens/index.ts";

// The sheets of the library and of the shell read only custom properties that exist: a
// public token, or a local alias `--k-<component>-…` declared in a sheet of the library
// (REQ-020-4, 13), or a dynamic value set by a script. A misspelt token resolves to
// nothing, silently: this test catches it.

const roots = ["../src", "../../shell/src"].map((p) => fileURLToPath(new URL(p, import.meta.url)));

function sheets(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return sheets(path);
    return /\.(vue|css)$/.test(entry.name) ? [path] : [];
  });
}

const files = roots.flatMap(sheets).map((path) => ({ path, text: readFileSync(path, "utf8") }));
const declared = new Set([
  ...tokens,
  // Declared in a sheet (`--k-x:`), set by a script (`setProperty("--k-x", …)`) or bound
  // as the key of a style object (`{ "--k-x": … }`).
  ...files.flatMap(({ text }) => [...text.matchAll(/(--k-[\w-]+)["']?\s*[:,]/g)].map((m) => m[1]!)),
]);

describe("sheets", () => {
  it("read declared custom properties only", () => {
    const unknown = files.flatMap(({ path, text }) =>
      [...text.matchAll(/var\(\s*(--k-[\w-]+)/g)]
        .map((m) => m[1]!)
        .filter((name) => !declared.has(name))
        .map((name) => `${path.split("/web/v2/")[1]}: ${name}`),
    );
    expect(unknown).toEqual([]);
  });
});
