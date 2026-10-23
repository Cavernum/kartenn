// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

import { readFileSync } from "node:fs";
import { createSSRApp, h } from "vue";
import { renderToString } from "vue/server-renderer";
import { describe, expect, it } from "vitest";
import Icon from "../src/components/Icon/Icon.vue";
import type { IconProps } from "../src/components/Icon/icon.ts";
import { icons, lucideIcons, metierIcons } from "../src/icons/index.ts";

// The icons (spec 020, task 7; REQ-020-15).

const render = (props: IconProps) => renderToString(createSSRApp({ render: () => h(Icon, props) }));

describe("icons", () => {
  it("a decorative icon is hidden from assistive technologies", async () => {
    const svg = await render({ name: "prefix" });
    expect(svg).toContain('aria-hidden="true"');
    expect(svg).not.toContain("role=");
    // The SSR compiler writes `viewbox`; the HTML parser restores the case of SVG attributes.
    expect(svg).toMatch(/viewBox="0 0 24 24"/i);
    expect(svg).toContain('data-size="16"');
  });

  it("an icon that carries a meaning has a name and the role img", async () => {
    const svg = await render({
      name: "circle-check",
      size: 20,
      meaningful: true,
      label: "Conforme",
    });
    expect(svg).toContain('role="img"');
    expect(svg).toContain('aria-label="Conforme"');
    expect(svg).not.toContain("aria-hidden");
    expect(svg).toContain('data-size="20"');
  });

  it("a meaningful icon without a name does not compile", () => {
    const named: IconProps = { name: "flow", meaningful: true, label: "Flux" };
    // @ts-expect-error: meaningful without label (REQ-020-15)
    const unnamed: IconProps = { name: "flow", meaningful: true };
    // @ts-expect-error: the same through the props of the component
    const props: InstanceType<typeof Icon>["$props"] = { name: "flow", meaningful: true };
    // @ts-expect-error: an icon outside the closed list
    const unknown: IconProps = { name: "rocket" };
    expect([named, unnamed, props, unknown]).toHaveLength(4);
  });

  it("business icons follow the grid of Lucide: 24 units, 2 of padding, no fill", () => {
    for (const [name, node] of Object.entries(metierIcons)) {
      const numbers = node
        .flatMap(([, attrs]) => Object.entries(attrs))
        .filter(([key]) => ["x", "y", "cx", "cy"].includes(key))
        .map(([, value]) => Number(value));
      for (const n of numbers) expect(n, name).toBeGreaterThanOrEqual(2);
      for (const n of numbers) expect(n, name).toBeLessThanOrEqual(22);
      for (const [tag, attrs] of node) {
        expect(["path", "rect", "circle", "line"], name).toContain(tag);
        expect(attrs, name).not.toHaveProperty("fill");
        expect(attrs, name).not.toHaveProperty("stroke-width");
      }
    }
  });

  it("names are unique across Lucide and business icons", () => {
    const names = [...Object.keys(lucideIcons), ...Object.keys(metierIcons)];
    expect(new Set(names).size).toBe(names.length);
    expect(Object.keys(icons)).toHaveLength(names.length);
  });

  it("ships the licence of Lucide, ISC and MIT (REQ-020-30)", () => {
    const licence = readFileSync(
      new URL("../src/icons/LICENSE-lucide.txt", import.meta.url),
      "utf8",
    );
    expect(licence).toMatch(/^ISC License/);
    expect(licence).toContain("The MIT License");
    const manifest = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
    expect(manifest.dependencies.lucide).toMatch(/^\d+\.\d+\.\d+$/);
  });
});
