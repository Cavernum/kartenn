// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { SDK_VERSION, satisfies } from "@kartenn/sdk/host";
import { describe, expect, it } from "vitest";
import { filesWithoutNotice } from "../../../tools/notices.ts";
import manifest from "../public/manifest.json";

// The package of the example plugin (spec 021, task 25; REQ-007-38).

const root = fileURLToPath(new URL("..", import.meta.url));
const pkg = JSON.parse(readFileSync(`${root}package.json`, "utf8")) as {
  version: string;
  license: string;
};

describe("@kartenn/plugin-example", () => {
  it("is under MPL-2.0, with the licence text and the notice in every source file", () => {
    expect(pkg.license).toBe("MPL-2.0");
    expect(readFileSync(`${root}LICENSE`, "utf8")).toMatch(/^Mozilla Public License Version 2\.0/);
    expect(filesWithoutNotice([`${root}src`, `${root}test`])).toEqual([]);
  });

  it("has the version of its package and asks for the SDK of the shell", () => {
    expect(manifest.version).toBe(pkg.version);
    expect(satisfies(SDK_VERSION, manifest.sdk)).toBe(true);
  });
});
