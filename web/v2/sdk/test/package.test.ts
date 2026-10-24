// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { filesWithoutNotice } from "../../tools/notices.ts";
import { SDK_VERSION } from "../src/index.ts";

// The package @kartenn/sdk 1.0 (spec 021, REQ-021-25; REQ-007-38).

const sdk = fileURLToPath(new URL("..", import.meta.url));
const manifest = JSON.parse(readFileSync(`${sdk}package.json`, "utf8")) as {
  version: string;
  license: string;
  dependencies?: Record<string, string>;
};

describe("@kartenn/sdk", () => {
  it("is under MPL-2.0, with the licence text and the notice in every source file", () => {
    expect(manifest.license).toBe("MPL-2.0");
    expect(readFileSync(`${sdk}LICENSE`, "utf8")).toMatch(/^Mozilla Public License Version 2\.0/);
    expect(filesWithoutNotice([`${sdk}src`, `${sdk}test`, `${sdk}scripts`])).toEqual([]);
  });

  it("announces the version of its manifest, 1.x", () => {
    expect(SDK_VERSION).toBe(manifest.version);
    expect(SDK_VERSION).toMatch(/^1\.\d+\.\d+$/);
  });

  it("depends on no framework (REQ-021-25, REQ-007-12)", () => {
    const names = Object.keys(manifest.dependencies ?? {});
    expect(names.filter((name) => /^(vue|react|@tanstack\/vue-query)$/.test(name))).toEqual([]);
  });
});
