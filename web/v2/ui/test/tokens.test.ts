// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { ENV_HUES, STATES, tokens } from "../src/tokens/index.ts";

// The tokens of kartenn (spec 020, task 5; REQ-020-2 to 11): contrasts of every declared
// pair in both modes (WCAG 2.2, 1.4.3 and 1.4.11), densities, reduced motion, environment
// hues distinct from the states, and the public list equal to the file.

const css = readFileSync(new URL("../src/tokens/tokens.css", import.meta.url), "utf8");

type Palette = Record<string, string>;

/** The declarations of the block `selector {`, the `nth` one in the file. */
function block(selector: string, nth = 0): Palette {
  let start = -1;
  for (let i = 0; i <= nth; i++) start = css.indexOf(`${selector} {`, start + 1);
  expect(start, selector).toBeGreaterThanOrEqual(0);
  const body = css.slice(start + selector.length + 2, css.indexOf("}", start));
  return Object.fromEntries(
    [...body.matchAll(/(--k-[\w-]+):\s*([^;]+);/g)].map((m) => [
      m[1]!,
      m[2]!.replace(/\s+/g, " ").trim(),
    ]),
  );
}

const light = block(":root");
const dark = { ...light, ...block(':root[data-theme="dark"]') };
const comfortable = { ...light, ...block(':root[data-density="comfortable"]') };
const reduced = { ...light, ...block(":root", 1) };

// Colours: sRGB, WCAG relative luminance, and color-mix in Oklab (CSS Color 5).
type Rgb = [number, number, number];

function hex(value: string): Rgb {
  expect(value, "a 6-digit hex colour").toMatch(/^#[0-9a-f]{6}$/);
  return [1, 3, 5].map((i) => parseInt(value.slice(i, i + 2), 16) / 255) as Rgb;
}

const toLinear = (c: number) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
const toGamma = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);

function toOklab([r, g, b]: Rgb): Rgb {
  const [lr, lg, lb] = [r, g, b].map(toLinear) as Rgb;
  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ];
}

function fromOklab([L, a, b]: Rgb): Rgb {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ].map((c) => Math.min(1, Math.max(0, toGamma(c)))) as Rgb;
}

/** The colour a token resolves to in a palette: hex, `var()` or `color-mix` in Oklab. */
function color(palette: Palette, value: string): Rgb {
  const reference = /^var\((--k-[\w-]+)\)$/.exec(value);
  if (reference) return color(palette, palette[reference[1]!]!);
  const mix = /^color-mix\( ?in oklab, var\((--k-[\w-]+)\) (\d+)%, var\((--k-[\w-]+)\) ?\)$/.exec(
    value,
  );
  if (mix) {
    const p = Number(mix[2]) / 100;
    const [a, b] = [
      toOklab(color(palette, `var(${mix[1]})`)),
      toOklab(color(palette, `var(${mix[3]})`)),
    ];
    return fromOklab(a.map((x, i) => x * p + b[i]! * (1 - p)) as Rgb);
  }
  return hex(value);
}

function luminance(rgb: Rgb): number {
  const [r, g, b] = rgb.map(toLinear) as Rgb;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function contrast(a: Rgb, b: Rgb): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

const pair = (palette: Palette, fg: string, bg: string) =>
  contrast(color(palette, `var(${fg})`), color(palette, `var(${bg})`));

/** Text on its backgrounds: 4.5:1 (WCAG 1.4.3). */
const TEXT: [string, string][] = [
  ...["--k-bg", "--k-surface", "--k-surface-sunken", "--k-accent-soft"].flatMap(
    (bg): [string, string][] => [
      ["--k-text", bg],
      ["--k-text-muted", bg],
      ["--k-accent", bg],
    ],
  ),
  ["--k-accent-fg", "--k-accent"],
  ["--k-accent-fg", "--k-accent-hover"],
  ["--k-danger-fg", "--k-danger"],
  ["--k-danger-fg", "--k-danger-hover"],
  ["--k-text", "--k-hover"],
  ["--k-text", "--k-active"],
  ["--k-text-muted", "--k-hover"],
  ["--k-band-fg", "--k-band-bg"],
  ["--k-sidebar-fg", "--k-sidebar-bg"],
  ["--k-sidebar-fg", "--k-sidebar-active-bg"],
  ["--k-sidebar-active-fg", "--k-sidebar-active-bg"],
  ...STATES.flatMap((s): [string, string][] => [
    [`--k-state-${s}-fg`, "--k-surface"],
    [`--k-state-${s}-fg`, `--k-state-${s}-bg`],
  ]),
];

/** Components, focus ring, meaningful icons and environment dots: 3:1 (WCAG 1.4.11). */
const NON_TEXT: [string, string][] = [
  ...["--k-bg", "--k-surface", "--k-surface-sunken"].flatMap((bg): [string, string][] => [
    ["--k-border-control", bg],
    ["--k-focus-ring", bg],
  ]),
  ["--k-focus-ring", "--k-accent-soft"],
  ["--k-sidebar-active-icon", "--k-sidebar-active-bg"],
  ...Array.from({ length: ENV_HUES }, (_, i): [string, string] => [
    `--k-env-${i + 1}`,
    "--k-surface",
  ]),
];

describe("tokens", () => {
  it("the public list equals the file", () => {
    const declared = new Set([...css.matchAll(/(--k-[\w-]+):/g)].map((m) => m[1]!));
    expect([...declared].sort()).toEqual([...tokens].sort());
  });

  it("the dark mode of the system is the dark mode chosen", () => {
    expect(block(':root:not([data-theme="light"])')).toEqual(block(':root[data-theme="dark"]'));
  });

  it("dark mode and density redefine semantic tokens only, already declared", () => {
    for (const name of Object.keys({
      ...block(':root[data-theme="dark"]'),
      ...block(':root[data-density="comfortable"]'),
    })) {
      expect(light, name).toHaveProperty(name);
    }
  });

  for (const [mode, palette] of [
    ["light", light],
    ["dark", dark],
  ] as const) {
    it(`text contrasts at least 4.5:1 in ${mode} mode`, () => {
      const low = TEXT.map(([fg, bg]) => [`${fg} on ${bg}`, pair(palette, fg, bg)] as const)
        .filter(([, ratio]) => ratio < 4.5)
        .map(([name, ratio]) => `${name}: ${ratio.toFixed(2)}`);
      expect(low).toEqual([]);
    });

    it(`components contrast at least 3:1 in ${mode} mode`, () => {
      const low = NON_TEXT.map(([fg, bg]) => [`${fg} on ${bg}`, pair(palette, fg, bg)] as const)
        .filter(([, ratio]) => ratio < 3)
        .map(([name, ratio]) => `${name}: ${ratio.toFixed(2)}`);
      expect(low).toEqual([]);
    });

    // A skeleton is decorative (no 3:1 due), but must read as a bar on its surface, still
    // under reduced motion, and its sheen must stand out of it (review of 2026-10-25).
    it(`skeleton bars and their sheen stand out in ${mode} mode`, () => {
      for (const bg of ["--k-surface", "--k-surface-sunken", "--k-hover"]) {
        expect(pair(palette, "--k-skeleton", bg), bg).toBeGreaterThanOrEqual(1.2);
      }
      expect(pair(palette, "--k-skeleton-sheen", "--k-skeleton")).toBeGreaterThanOrEqual(1.25);
    });

    // The four states of REQ-020-7 (`info` is the accent, which the first hue shares).
    it(`environment hues differ from the states in ${mode} mode`, () => {
      const states = (["ok", "warn", "ko", "review"] as const).map((s) =>
        color(palette, `var(--k-state-${s}-fg)`).join(),
      );
      const hues = Array.from({ length: ENV_HUES }, (_, i) =>
        color(palette, `var(--k-env-${i + 1})`).join(),
      );
      expect(new Set(hues).size, "six distinct hues").toBe(ENV_HUES);
      for (const hue of hues) expect(states).not.toContain(hue);
    });
  }

  it("keeps the departures from « Ardoise » (design « Couleurs »)", () => {
    expect(pair(light, "--k-text-muted", "--k-accent-soft")).toBeCloseTo(4.64, 1);
    expect(pair(light, "--k-border-control", "--k-surface")).toBeCloseTo(3.5, 1);
  });

  it("has two densities, 28 and 36 px rows, controls of at least 24 px", () => {
    expect(light["--k-row-height"]).toBe("28px");
    expect(comfortable["--k-row-height"]).toBe("36px");
    for (const palette of [light, comfortable]) {
      for (const size of ["sm", "md", "lg"]) {
        expect(parseFloat(palette[`--k-control-height-${size}`]!)).toBeGreaterThanOrEqual(24);
      }
    }
  });

  // A colour fade is not motion (WCAG 2.3.3): `fade` and `slow` (the highlight of a row)
  // keep 80 ms; every other duration, which moves or scales, is zero.
  for (const [source, palette] of [
    ["the system", reduced],
    ["the user's setting", { ...light, ...block(':root[data-motion="reduce"]') }],
  ] as const) {
    it(`zeroes every duration of a movement under reduced motion (${source})`, () => {
      for (const [name, value] of Object.entries(palette)) {
        if (!name.startsWith("--k-duration-")) continue;
        const fade = name === "--k-duration-fade" || name === "--k-duration-slow";
        expect(value, name).toBe(fade ? "80ms" : "0ms");
      }
      expect(palette["--k-duration-instant"]).toBe("0ms");
      for (const size of ["sm", "md", "lg"]) {
        expect(palette[`--k-motion-shift-${size}`], size).toBe("0px");
      }
      expect(palette["--k-motion-scale"]).toBe("1");
    });
  }

  it("the reduced motion of the system is the one chosen", () => {
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\) \{\s*:root \{/);
    expect(block(":root", 1)).toEqual(block(':root[data-motion="reduce"]'));
    expect(light["--k-duration-medium"]).toBe("200ms");
    expect(light["--k-duration-slow"]).toBe("300ms");
  });

  it("the ratio follows WCAG", () => {
    expect(contrast(hex("#000000"), hex("#ffffff"))).toBeCloseTo(21, 5);
    expect(contrast(hex("#777777"), hex("#ffffff"))).toBeCloseTo(4.48, 2);
  });
});
