// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { SDK_VERSION, satisfies } from "@kartenn/sdk/host";
import { describe, expect, it } from "vitest";
import { filesWithoutNotice } from "../../../tools/notices.ts";
import { PLUGIN_BUDGET, SHARED_MODULES, pluginWeight } from "../../../tools/plugin-size.ts";
import manifest from "../public/manifest.json";

// The package of the machines plugin (spec 021, task 18a; REQ-007-38).

const root = fileURLToPath(new URL("..", import.meta.url));
const pkg = JSON.parse(readFileSync(`${root}package.json`, "utf8")) as {
  version: string;
  license: string;
};

describe("@kartenn/plugin-machines", () => {
  it("is under MPL-2.0, with the licence text and the notice in every source file", () => {
    expect(pkg.license).toBe("MPL-2.0");
    expect(readFileSync(`${root}LICENSE`, "utf8")).toMatch(/^Mozilla Public License Version 2\.0/);
    expect(filesWithoutNotice([`${root}src`, `${root}test`])).toEqual([]);
  });

  it("has the version of its package and asks for the SDK of the shell", () => {
    expect(manifest.version).toBe(pkg.version);
    expect(satisfies(SDK_VERSION, manifest.sdk)).toBe(true);
  });

  it("weighs at most 50 Ko gzip, the shared modules imported, never bundled (REQ-021-30)", async () => {
    const weight = await pluginWeight(root);
    expect(weight.bundled).toEqual([]);
    // No sheet of its own (A23).
    expect(weight.sheets).toEqual([]);
    expect(weight.imports.filter((name) => !SHARED_MODULES.includes(name))).toEqual([]);
    expect(weight.bytes).toBeLessThanOrEqual(PLUGIN_BUDGET);
  }, 60_000);
});
