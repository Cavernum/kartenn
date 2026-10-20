// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, describe, expect, it } from "vitest";
import { filesWithoutNotice, hasMplNotice, MPL_NOTICE } from "./notices.ts";

const web = fileURLToPath(new URL("../..", import.meta.url));

const LINE_NOTICE = [
  "// This Source Code Form is subject to the terms of the Mozilla Public",
  "// License, v. 2.0. If a copy of the MPL was not distributed with this",
  "// file, You can obtain one at https://mozilla.org/MPL/2.0/.",
].join("\n");

describe("MPL notices (REQ-007-38)", () => {
  it("every source file of web/sdk and web/plugins carries the notice", () => {
    expect(filesWithoutNotice([join(web, "sdk"), join(web, "plugins")])).toEqual([]);
  });

  it("the notice is the one of the Rust crates under MPL", () => {
    const rust = readFileSync(join(web, "../crates/syslog-sender/src/lib.rs"), "utf8");
    expect(hasMplNotice(rust)).toBe(true);
  });

  it("accepts line, block and HTML comments", () => {
    expect(hasMplNotice(`${LINE_NOTICE}\n\nexport {};\n`)).toBe(true);
    expect(hasMplNotice(`/* ${MPL_NOTICE.replace(/\. /g, ".\n * ")} */\n:root {}\n`)).toBe(true);
    expect(hasMplNotice(`<!-- ${MPL_NOTICE} -->\n<template />\n`)).toBe(true);
  });

  it("refuses a missing or altered notice", () => {
    expect(hasMplNotice("export const x = 1;\n")).toBe(false);
    expect(hasMplNotice(LINE_NOTICE.replace("v. 2.0", "v. 1.1"))).toBe(false);
    expect(hasMplNotice(`\n\n\n\n\n\n\n\n${LINE_NOTICE}`)).toBe(false);
  });

  describe("on a tree", () => {
    let root: string | undefined;
    afterEach(() => {
      if (root) rmSync(root, { recursive: true, force: true });
    });

    it("names a file of the SDK without the notice, and skips dependencies", () => {
      root = mkdtempSync(join(tmpdir(), "kartenn-notices-"));
      mkdirSync(join(root, "src"));
      mkdirSync(join(root, "node_modules"));
      writeFileSync(join(root, "src", "ok.ts"), `${LINE_NOTICE}\nexport {};\n`);
      writeFileSync(join(root, "src", "bare.ts"), "export {};\n");
      writeFileSync(join(root, "src", "notes.md"), "no notice needed\n");
      writeFileSync(join(root, "node_modules", "dep.js"), "export {};\n");
      expect(filesWithoutNotice([root])).toEqual([join(root, "src", "bare.ts")]);
    });
  });
});
