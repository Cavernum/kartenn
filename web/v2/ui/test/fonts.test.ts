// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { describe, expect, it } from "vitest";

// The fonts of kartenn (spec 020, task 6; REQ-020-10, 30): Geist and Geist Mono, variable
// woff2, Latin and Latin Extended, shipped with their licence, within the weight budget.

const fonts = new URL("../src/fonts/", import.meta.url);
const sheet = readFileSync(new URL("../src/tokens/fonts.css", import.meta.url), "utf8");
const files = readdirSync(fonts);
const woff2 = files.filter((file) => file.endsWith(".woff2")).sort();

describe("fonts", () => {
  it("are the four variable files, Latin and Latin Extended, upright", () => {
    expect(woff2).toEqual([
      "geist-latin-ext-wght-normal.woff2",
      "geist-latin-wght-normal.woff2",
      "geist-mono-latin-ext-wght-normal.woff2",
      "geist-mono-latin-wght-normal.woff2",
    ]);
    for (const file of woff2) {
      // The signature of WOFF 2.0 (W3C WOFF2, § 3 « WOFF2 Header »).
      expect(readFileSync(new URL(file, fonts)).subarray(0, 4).toString("latin1"), file).toBe(
        "wOF2",
      );
    }
  });

  it("weigh at most 120 KB together (design « Risques »)", () => {
    const total = woff2.reduce((sum, file) => sum + statSync(new URL(file, fonts)).size, 0);
    expect(total).toBeLessThanOrEqual(120_000);
  });

  it("are each declared once, swapped, from the origin only", () => {
    for (const file of woff2) expect(sheet.split(`../fonts/${file}`).length - 1, file).toBe(1);
    expect(sheet.match(/font-display: swap;/g)).toHaveLength(4);
    expect(sheet.replace(/\/\*[\s\S]*?\*\//g, "")).not.toMatch(/https?:|\/\//);
  });

  it("ship the SIL Open Font License 1.1 with their copyright", () => {
    for (const licence of ["OFL-Geist.txt", "OFL-GeistMono.txt"]) {
      const text = readFileSync(new URL(licence, fonts), "utf8");
      expect(text, licence).toMatch(/^Copyright 2024 The Geist Project Authors/);
      expect(text, licence).toContain("SIL OPEN FONT LICENSE Version 1.1");
    }
  });

  it("name the families of the tokens, with their metric fallbacks", () => {
    const tokens = readFileSync(new URL("../src/tokens/tokens.css", import.meta.url), "utf8");
    expect(tokens).toContain('--k-font-sans: "Geist", "Geist Fallback", system-ui, sans-serif;');
    expect(sheet).toContain('font-family: "Geist Fallback";');
    expect(sheet).toContain('font-family: "Geist Mono Fallback";');
  });
});
