// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { describe, expect, it } from "vitest";
import { compare, parseVersion, satisfies } from "../src/semver.ts";

describe("sdk range of a manifest (W7)", () => {
  it.each([
    // Caret: the first non-zero number is fixed.
    ["1.2.3", "^1.2", true],
    ["1.9.0", "^1.2", true],
    ["1.1.9", "^1.2", false],
    ["2.0.0", "^1.2", false],
    ["1.0.0", "^1", true],
    ["0.2.5", "^0.2", true],
    ["0.3.0", "^0.2", false],
    ["0.0.3", "^0.0.3", true],
    ["0.0.4", "^0.0.3", false],
    ["0.1.0", "^0", true],
    // Tilde: the minor is fixed.
    ["1.2.9", "~1.2", true],
    ["1.3.0", "~1.2", false],
    ["1.2.2", "~1.2.3", false],
    ["1.2.3", "~1", false],
    // Exact.
    ["1.2.3", "1.2.3", true],
    ["1.2.3+build.5", "1.2.3", true],
    ["1.2.4", "1.2.3", false],
    ["1.2.3", "1.2", false],
    // Prereleases: never chosen by a range of releases.
    ["1.3.0-rc.1", "^1.2", false],
    ["2.0.0-rc.1", "^1.2", false],
    ["1.2.3-beta.2", "^1.2.3-beta.1", true],
    ["1.2.3-alpha", "^1.2.3-beta.1", false],
    ["1.2.4-beta.1", "^1.2.3-beta.1", false],
    ["1.2.3", "^1.2.3-beta.1", true],
    ["1.2.3-beta.1", "1.2.3-beta.1", true],
    // Malformed.
    ["1.2", "^1.2", false],
    ["1.2.3", ">=1.2", false],
    ["1.2.3", "^01.2", false],
  ])("%s satisfies %s: %s", (version, range, expected) => {
    expect(satisfies(version, range)).toBe(expected);
  });

  it("orders prereleases as Semantic Versioning 2.0.0 §11", () => {
    const order = [
      "1.0.0-alpha",
      "1.0.0-alpha.1",
      "1.0.0-alpha.beta",
      "1.0.0-beta",
      "1.0.0-beta.2",
      "1.0.0-beta.11",
      "1.0.0-rc.1",
      "1.0.0",
    ].map((v) => parseVersion(v)!);
    for (let i = 1; i < order.length; i++) {
      expect(compare(order[i - 1]!, order[i]!)).toBeLessThan(0);
    }
  });
});
