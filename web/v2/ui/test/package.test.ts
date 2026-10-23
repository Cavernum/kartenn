// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { filesWithoutNotice } from "../../tools/notices.ts";

// The package @kartenn/ui (spec 020, task 4; REQ-020-30, 33).

const ui = fileURLToPath(new URL("..", import.meta.url));
const manifest = JSON.parse(readFileSync(`${ui}package.json`, "utf8")) as {
  license: string;
  dependencies: Record<string, string>;
};

describe("@kartenn/ui", () => {
  it("is under MPL-2.0, with the licence text and the notice in every source file", () => {
    expect(manifest.license).toBe("MPL-2.0");
    expect(readFileSync(`${ui}LICENSE`, "utf8")).toMatch(/^Mozilla Public License Version 2\.0/);
    expect(filesWithoutNotice([`${ui}src`, `${ui}test`])).toEqual([]);
  });

  it("pins Reka UI to an exact version (REQ-020-33)", () => {
    expect(manifest.dependencies["reka-ui"]).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it("declares the order of the layers before anything else (REQ-020-12)", () => {
    const sheet = readFileSync(`${ui}src/style.css`, "utf8");
    const imports = [...sheet.matchAll(/@import "([^"]+)"/g)].map((m) => m[1]);
    expect(imports[0]).toBe("./styles/layers.css");
    expect(readFileSync(`${ui}src/styles/layers.css`, "utf8")).toContain(
      "@layer reset, tokens, base, components, plugins, utilities;",
    );
    expect(existsSync(`${ui}src/index.ts`)).toBe(true);
  });
});
