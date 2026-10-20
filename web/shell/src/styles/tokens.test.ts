import { readFileSync } from "node:fs";
import { tokens } from "@kartenn/sdk";
import { describe, expect, it } from "vitest";

// Contrasts of the design tokens in both modes (REQ-007-30, 33; WCAG 2.2, 1.4.3 and
// 1.4.11): 4.5:1 for text, 3:1 for borders and the focus ring.

const css = readFileSync(new URL("./tokens.css", import.meta.url), "utf8");

function block(selector: string): Record<string, string> {
  const start = css.indexOf(`${selector} {`);
  expect(start, selector).toBeGreaterThanOrEqual(0);
  const body = css.slice(start, css.indexOf("}", start));
  return Object.fromEntries(
    [...body.matchAll(/(--k-[\w-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2]!.trim()]),
  );
}

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  }) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x) as [number, number];
  return (hi + 0.05) / (lo + 0.05);
}

const light = block(":root");
const dark = block(':root[data-theme="dark"]');
const backgrounds = ["--k-color-bg", "--k-color-surface"];
const texts = [
  "--k-color-text",
  "--k-color-muted",
  "--k-color-accent",
  "--k-color-danger",
  "--k-color-warning",
];
const components = ["--k-color-border", "--k-color-focus"];

describe("design tokens", () => {
  it("are all defined, those of the SDK included", () => {
    for (const token of tokens) expect(light, token).toHaveProperty(token);
  });

  it("the dark mode of the system is the dark mode chosen", () => {
    expect(block(':root:not([data-theme="light"])')).toEqual(dark);
  });

  for (const [mode, palette] of [
    ["light", light],
    ["dark", dark],
  ] as const) {
    it(`contrast enough in ${mode} mode`, () => {
      for (const bg of backgrounds) {
        for (const fg of texts) {
          expect(contrast(palette[fg]!, palette[bg]!), `${fg} on ${bg}`).toBeGreaterThanOrEqual(
            4.5,
          );
        }
        for (const fg of components) {
          expect(contrast(palette[fg]!, palette[bg]!), `${fg} on ${bg}`).toBeGreaterThanOrEqual(3);
        }
      }
    });
  }

  it("the ratio follows WCAG", () => {
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21, 5);
    expect(contrast("#777777", "#ffffff")).toBeCloseTo(4.48, 2);
  });
});
